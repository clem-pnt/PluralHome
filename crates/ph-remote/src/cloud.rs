//! Synchronisation chiffrée entre appareils via le serveur PluralHome.
//!
//! L'accès est authentifié par la session Aragonite ; le contenu est chiffré ici,
//! avant l'envoi, par une clé de synchro que le serveur ne voit jamais. Il ne
//! reçoit que des identifiants opaques (HMAC de la table et de la clé primaire)
//! et des blocs scellés. Un nouvel appareil obtient la clé de synchro par une clé
//! de secours (affichée une fois à l'activation) ou par appairage avec un
//! appareil déjà synchronisé (échange X25519, vérifié par une empreinte).

use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use ph_core::cloudkey::{self, PairingSecret, RecoveryKey, SyncKey};
use ph_core::replica::{self, Change};
use ph_core::rusqlite::Connection;
use ph_core::{CoreError, kv};
use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::aragonite::{self, Sso};
use crate::db::Db;
use crate::{RemoteError, Result, USER_AGENT};

/// Adresse du service, remplaçable par `PLURALHOME_SYNC_URL`.
pub const DEFAULT_BASE: &str = "https://api.aragonite.fr/pluralhome-sync";

const K_KEY: &str = "sync.key";
const K_KEYSET: &str = "sync.keyset_id";
const K_DEVICE: &str = "sync.device";
const K_SEQ: &str = "sync.seq";
const K_LAST_AT: &str = "sync.last_at";
const K_LAST_ERROR: &str = "sync.last_error";
const K_PAIRING: &str = "sync.pairing";

const PAGE: usize = 500;
/// Taille maximale d'un envoi (le serveur refuse au-delà de 8 Mo).
const MAX_BATCH_BYTES: usize = 6 * 1024 * 1024;
const MAX_ROUNDS: usize = 200;

#[must_use]
pub fn base_url() -> String {
    std::env::var("PLURALHOME_SYNC_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_BASE.to_owned())
}

// ---------- API ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keyset {
    pub id: String,
    /// Clé de synchro chiffrée par la clé de secours (base64).
    pub recovery: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteState {
    pub keyset: Option<Keyset>,
    pub seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub k: String,
    pub v: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Page {
    pub items: Vec<Item>,
    pub seq: i64,
    pub more: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairRequest {
    pub id: String,
    #[serde(rename = "pub")]
    pub public: String,
    pub expires_at: i64,
}

/// Opérations du serveur, appelées avec un jeton d'accès Aragonite.
pub trait CloudApi {
    fn state(&self, token: &str) -> Result<RemoteState>;
    /// `Conflict` si un jeu de clés existe déjà pour ce compte.
    fn put_keyset(&self, token: &str, keyset: &Keyset) -> Result<()>;
    fn changes(&self, token: &str, since: i64, limit: usize) -> Result<Page>;
    fn push(&self, token: &str, items: &[Item]) -> Result<i64>;
    fn delete_account(&self, token: &str) -> Result<()>;
    fn pair_create(&self, token: &str, public: &str) -> Result<String>;
    fn pair_list(&self, token: &str) -> Result<Vec<PairRequest>>;
    fn pair_seal(&self, token: &str, id: &str, sealed: &str) -> Result<()>;
    /// `NotFound` si la demande a expiré ou a été annulée.
    fn pair_take(&self, token: &str, id: &str) -> Result<Option<String>>;
    fn pair_cancel(&self, token: &str, id: &str) -> Result<()>;
}

pub struct HttpCloud {
    http: Client,
    base: String,
}

impl HttpCloud {
    pub fn new() -> Result<Self> {
        Self::with_base(&base_url())
    }

    pub fn with_base(base: &str) -> Result<Self> {
        let local = base.starts_with("http://127.0.0.1") || base.starts_with("http://localhost");
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(60))
            .https_only(!local)
            .build()?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
        })
    }

    fn call<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        token: &str,
        body: Option<&Value>,
    ) -> Result<T> {
        let mut req = self
            .http
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(token);
        if let Some(b) = body {
            req = req.json(b);
        }
        let res = req.send()?;
        let status = res.status();
        if status.is_success() {
            return Ok(res.json()?);
        }
        let code = res
            .json::<Value>()
            .ok()
            .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_default();
        Err(match (status, code.as_str()) {
            (StatusCode::UNAUTHORIZED, _) => RemoteError::Unauthorized,
            (StatusCode::TOO_MANY_REQUESTS, _) => RemoteError::RateLimited,
            (StatusCode::CONFLICT, "KEYSET_EXISTS") => RemoteError::Conflict,
            (StatusCode::CONFLICT, "NO_KEYSET") => RemoteError::NotLinked,
            (StatusCode::NOT_FOUND, "PAIR_NOT_FOUND") => RemoteError::Core(CoreError::NotFound),
            (StatusCode::PAYLOAD_TOO_LARGE, "QUOTA") => RemoteError::Quota,
            _ => RemoteError::Http {
                status: status.as_u16(),
                message: code.chars().take(40).collect(),
            },
        })
    }
}

#[derive(Deserialize)]
struct Seq {
    seq: i64,
}
#[derive(Deserialize)]
struct Id {
    id: String,
}
#[derive(Deserialize)]
struct Requests {
    requests: Vec<PairRequest>,
}
#[derive(Deserialize)]
struct Sealed {
    sealed: Option<String>,
}
#[derive(Deserialize)]
struct Ack {}

impl CloudApi for HttpCloud {
    fn state(&self, token: &str) -> Result<RemoteState> {
        self.call(reqwest::Method::GET, "/v1/state", token, None)
    }
    fn put_keyset(&self, token: &str, keyset: &Keyset) -> Result<()> {
        let body = serde_json::to_value(keyset).map_err(|_| RemoteError::Decode)?;
        self.call::<Ack>(reqwest::Method::PUT, "/v1/keyset", token, Some(&body))
            .map(|_| ())
    }
    fn changes(&self, token: &str, since: i64, limit: usize) -> Result<Page> {
        let path = format!("/v1/changes?since={since}&limit={limit}");
        self.call(reqwest::Method::GET, &path, token, None)
    }
    fn push(&self, token: &str, items: &[Item]) -> Result<i64> {
        let body = json!({ "items": items });
        self.call::<Seq>(reqwest::Method::POST, "/v1/changes", token, Some(&body))
            .map(|s| s.seq)
    }
    fn delete_account(&self, token: &str) -> Result<()> {
        self.call::<Ack>(reqwest::Method::DELETE, "/v1/account", token, None)
            .map(|_| ())
    }
    fn pair_create(&self, token: &str, public: &str) -> Result<String> {
        let body = json!({ "pub": public });
        self.call::<Id>(reqwest::Method::POST, "/v1/pair", token, Some(&body))
            .map(|r| r.id)
    }
    fn pair_list(&self, token: &str) -> Result<Vec<PairRequest>> {
        self.call::<Requests>(reqwest::Method::GET, "/v1/pair", token, None)
            .map(|r| r.requests)
    }
    fn pair_seal(&self, token: &str, id: &str, sealed: &str) -> Result<()> {
        let body = json!({ "sealed": sealed });
        let path = format!("/v1/pair/{}", path_id(id)?);
        self.call::<Ack>(reqwest::Method::PUT, &path, token, Some(&body))
            .map(|_| ())
    }
    fn pair_take(&self, token: &str, id: &str) -> Result<Option<String>> {
        let path = format!("/v1/pair/{}", path_id(id)?);
        self.call::<Sealed>(reqwest::Method::GET, &path, token, None)
            .map(|r| r.sealed)
    }
    fn pair_cancel(&self, token: &str, id: &str) -> Result<()> {
        let path = format!("/v1/pair/{}", path_id(id)?);
        self.call::<Ack>(reqwest::Method::DELETE, &path, token, None)
            .map(|_| ())
    }
}

fn path_id(id: &str) -> Result<String> {
    Uuid::parse_str(id)
        .map(|u| u.hyphenated().to_string())
        .map_err(|_| RemoteError::Decode)
}

// ---------- session ----------

/// Fournit un jeton d'accès ; `invalidate` est appelé quand le serveur le refuse.
pub trait Auth {
    fn token(&self) -> Result<Zeroizing<String>>;
    fn invalidate(&self);
}

/// Jeton issu de la session Aragonite enregistrée dans le coffre.
pub struct SsoAuth<'a, D: Db> {
    pub db: &'a D,
    pub sso: &'a Sso,
}

impl<D: Db> Auth for SsoAuth<'_, D> {
    fn token(&self) -> Result<Zeroizing<String>> {
        aragonite::access_token(self.db, self.sso)
    }
    fn invalidate(&self) {
        aragonite::invalidate();
    }
}

/// Exécute `op` avec un jeton ; un refus déclenche un renouvellement et un seul nouvel essai.
fn authed<T>(auth: &impl Auth, op: impl Fn(&str) -> Result<T>) -> Result<T> {
    let token = auth.token()?;
    match op(&token) {
        Err(RemoteError::Unauthorized) => {
            auth.invalidate();
            op(&auth.token()?)
        }
        r => r,
    }
}

// ---------- état local ----------

fn load_key(conn: &Connection) -> ph_core::Result<Option<(SyncKey, String)>> {
    let (Some(key), Some(id)) = (kv::get(conn, K_KEY)?, kv::get(conn, K_KEYSET)?) else {
        return Ok(None);
    };
    let key = Zeroizing::new(key);
    Ok(Some((SyncKey::from_hex(&key)?, id)))
}

fn device(conn: &Connection) -> ph_core::Result<String> {
    if let Some(d) = kv::get(conn, K_DEVICE)? {
        return Ok(d);
    }
    let d = Uuid::now_v7().to_string();
    kv::set(conn, K_DEVICE, &d)?;
    Ok(d)
}

/// Enregistre la clé de synchro et prépare l'envoi de tout le contenu local.
fn install(conn: &mut Connection, key: &SyncKey, keyset_id: &str) -> ph_core::Result<()> {
    let tx = conn.transaction()?;
    kv::set(&tx, K_KEY, &key.to_hex())?;
    kv::set(&tx, K_KEYSET, keyset_id)?;
    kv::set(&tx, K_SEQ, "0")?;
    kv::delete(&tx, K_LAST_ERROR)?;
    kv::delete(&tx, K_PAIRING)?;
    device(&tx)?;
    replica::seed(&tx)?;
    tx.commit()?;
    Ok(())
}

fn clear(conn: &Connection) -> ph_core::Result<()> {
    for k in [K_KEY, K_KEYSET, K_SEQ, K_LAST_AT, K_LAST_ERROR, K_PAIRING] {
        kv::delete(conn, k)?;
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Pairing {
    id: String,
    secret: Zeroizing<String>,
}

fn load_pairing(conn: &Connection) -> ph_core::Result<Option<(String, PairingSecret)>> {
    let Some(raw) = kv::get(conn, K_PAIRING)?.map(Zeroizing::new) else {
        return Ok(None);
    };
    let Ok(p) = serde_json::from_str::<Pairing>(&raw) else {
        kv::delete(conn, K_PAIRING)?;
        return Ok(None);
    };
    Ok(Some((p.id, PairingSecret::from_hex(&p.secret)?)))
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CloudStatus {
    pub enabled: bool,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub pending: i64,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
    /// Empreinte de la demande d'appairage en cours sur cet appareil.
    pub pairing: Option<String>,
}

/// État local, sans réseau.
pub fn status(db: &impl Db) -> Result<CloudStatus> {
    Ok(db.with(|c| {
        let enabled = kv::get(c, K_KEY)?.is_some();
        let pairing = load_pairing(c)?.map(|(_, s)| cloudkey::fingerprint(&s.public()));
        Ok(CloudStatus {
            enabled,
            pending: if enabled {
                replica::pending_count(c)?
            } else {
                0
            },
            last_sync: kv::get(c, K_LAST_AT)?.and_then(|s| s.parse().ok()),
            last_error: kv::get(c, K_LAST_ERROR)?,
            pairing,
        })
    })?)
}

/// Ce que le serveur sait de ce compte, pour choisir entre activer et rejoindre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Remote {
    /// Aucune synchro sur ce compte.
    Empty,
    /// Une synchro existe : un appareil doit la rejoindre.
    Exists,
}

pub fn probe(api: &impl CloudApi, auth: &impl Auth) -> Result<Remote> {
    let st = authed(auth, |t| api.state(t))?;
    Ok(if st.keyset.is_some() {
        Remote::Exists
    } else {
        Remote::Empty
    })
}

// ---------- activation ----------

/// Active la synchro pour ce compte. Renvoie la clé de secours, à montrer une seule fois.
/// `Conflict` si le compte a déjà une synchro (il faut alors la rejoindre).
pub fn enable(db: &impl Db, api: &impl CloudApi, auth: &impl Auth) -> Result<Zeroizing<String>> {
    let key = SyncKey::generate()?;
    let recovery = RecoveryKey::generate()?;
    let id = Uuid::now_v7().to_string();
    let wrapped = key.wrap_for_recovery(&recovery, &id)?;
    let keyset = Keyset {
        id: id.clone(),
        recovery: B64.encode(wrapped),
    };
    authed(auth, |t| api.put_keyset(t, &keyset))?;
    db.with(|c| install(c, &key, &id))?;
    Ok(recovery.display())
}

/// Rejoint la synchro existante avec la clé de secours.
pub fn join_with_recovery(
    db: &impl Db,
    api: &impl CloudApi,
    auth: &impl Auth,
    recovery: &str,
) -> Result<()> {
    let recovery = RecoveryKey::parse(recovery)?;
    let st = authed(auth, |t| api.state(t))?;
    let ks = st.keyset.ok_or(RemoteError::NotLinked)?;
    let wrapped = B64.decode(&ks.recovery).map_err(|_| RemoteError::Decode)?;
    let key = SyncKey::unwrap_recovery(&wrapped, &recovery, &ks.id)?;
    db.with(|c| install(c, &key, &ks.id))?;
    Ok(())
}

/// Désactive la synchro sur cet appareil ; les données locales et distantes restent.
pub fn disable(db: &impl Db) -> Result<()> {
    Ok(db.with(|c| clear(c))?)
}

/// Efface toutes les données synchronisées du serveur, puis désactive ici.
pub fn delete_remote(db: &impl Db, api: &impl CloudApi, auth: &impl Auth) -> Result<()> {
    authed(auth, |t| api.delete_account(t))?;
    disable(db)
}

// ---------- appairage ----------

/// Demande la clé à un appareil déjà synchronisé. Renvoie l'empreinte à comparer.
pub fn pair_request(db: &impl Db, api: &impl CloudApi, auth: &impl Auth) -> Result<String> {
    if let Some((id, _)) = db.with(|c| load_pairing(c))? {
        let _ = authed(auth, |t| api.pair_cancel(t, &id));
    }
    let secret = PairingSecret::generate()?;
    let public = secret.public();
    let id = authed(auth, |t| api.pair_create(t, &hex(&public)))?;
    let p = Pairing {
        id,
        secret: secret.to_hex(),
    };
    let raw = Zeroizing::new(serde_json::to_string(&p).map_err(|_| RemoteError::Decode)?);
    db.with(|c| kv::set(c, K_PAIRING, &raw))?;
    Ok(cloudkey::fingerprint(&public))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PairPoll {
    /// Pas encore approuvée.
    Waiting,
    /// Clé reçue : la synchro est active.
    Joined,
    /// Expirée ou refusée.
    Gone,
}

/// Vérifie si la demande d'appairage a été approuvée.
pub fn pair_poll(db: &impl Db, api: &impl CloudApi, auth: &impl Auth) -> Result<PairPoll> {
    let Some((id, secret)) = db.with(|c| load_pairing(c))? else {
        return Ok(PairPoll::Gone);
    };
    let sealed = match authed(auth, |t| api.pair_take(t, &id)) {
        Ok(Some(s)) => s,
        Ok(None) => return Ok(PairPoll::Waiting),
        Err(RemoteError::Core(CoreError::NotFound)) => {
            db.with(|c| kv::delete(c, K_PAIRING))?;
            return Ok(PairPoll::Gone);
        }
        Err(e) => return Err(e),
    };
    let st = authed(auth, |t| api.state(t))?;
    let ks = st.keyset.ok_or(RemoteError::NotLinked)?;
    let sealed = B64.decode(sealed).map_err(|_| RemoteError::Decode)?;
    let key = match secret.open(&sealed, &ks.id) {
        Ok(k) => k,
        Err(e) => {
            db.with(|c| kv::delete(c, K_PAIRING))?;
            return Err(e.into());
        }
    };
    db.with(|c| install(c, &key, &ks.id))?;
    Ok(PairPoll::Joined)
}

pub fn pair_cancel(db: &impl Db, api: &impl CloudApi, auth: &impl Auth) -> Result<()> {
    if let Some((id, _)) = db.with(|c| load_pairing(c))? {
        db.with(|c| kv::delete(c, K_PAIRING))?;
        authed(auth, |t| api.pair_cancel(t, &id))?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PendingDevice {
    pub id: String,
    pub fingerprint: String,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub expires_at: i64,
}

/// Demandes d'appairage en attente (sur un appareil déjà synchronisé).
pub fn pair_requests(
    db: &impl Db,
    api: &impl CloudApi,
    auth: &impl Auth,
) -> Result<Vec<PendingDevice>> {
    let own = db.with(|c| load_pairing(c))?.map(|(id, _)| id);
    let list = authed(auth, |t| api.pair_list(t))?;
    Ok(list
        .into_iter()
        .filter(|r| Some(&r.id) != own.as_ref())
        .filter_map(|r| {
            let public = unhex32(&r.public)?;
            Some(PendingDevice {
                id: r.id,
                fingerprint: cloudkey::fingerprint(&public),
                expires_at: r.expires_at,
            })
        })
        .collect())
}

/// Confie la clé de synchro à l'appareil demandeur (après comparaison de l'empreinte).
pub fn pair_approve(db: &impl Db, api: &impl CloudApi, auth: &impl Auth, id: &str) -> Result<()> {
    let (key, keyset) = db.with(|c| load_key(c))?.ok_or(RemoteError::NotLinked)?;
    let list = authed(auth, |t| api.pair_list(t))?;
    let req = list
        .into_iter()
        .find(|r| r.id == id)
        .ok_or(RemoteError::Core(CoreError::NotFound))?;
    let public = unhex32(&req.public).ok_or(RemoteError::Decode)?;
    let sealed = B64.encode(key.seal_for_device(&public, &keyset)?);
    authed(auth, |t| api.pair_seal(t, id, &sealed))
}

/// Refuse une demande d'appairage.
pub fn pair_reject(api: &impl CloudApi, auth: &impl Auth, id: &str) -> Result<()> {
    authed(auth, |t| api.pair_cancel(t, id))
}

// ---------- synchro ----------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    pub pulled: usize,
    pub pushed: usize,
}

/// Un cycle complet : réception puis envoi. Sans effet si la synchro est inactive.
pub fn sync(db: &impl Db, api: &impl CloudApi, auth: &impl Auth) -> Result<Report> {
    let result = run(db, api, auth);
    let now = now_ms().to_string();
    db.with(|c| match &result {
        Ok(_) => {
            kv::set(c, K_LAST_AT, &now)?;
            kv::delete(c, K_LAST_ERROR)
        }
        Err(e) if kv::get(c, K_KEY)?.is_some() => kv::set(c, K_LAST_ERROR, e.code()),
        Err(_) => Ok(()),
    })?;
    result
}

fn run(db: &impl Db, api: &impl CloudApi, auth: &impl Auth) -> Result<Report> {
    let Some((key, keyset)) = db.with(|c| load_key(c))? else {
        return Err(RemoteError::NotLinked);
    };
    let me = db.with(|c| device(c))?;
    let st = authed(auth, |t| api.state(t))?;
    match st.keyset {
        Some(ks) if ks.id == keyset => {}
        // Synchro supprimée ou recréée depuis un autre appareil : la clé locale ne vaut plus.
        _ => {
            db.with(|c| clear(c))?;
            return Err(RemoteError::NotLinked);
        }
    }

    let mut report = Report::default();
    let mut since: i64 = db
        .with(|c| kv::get(c, K_SEQ))?
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    // Le serveur a été vidé puis recréé avec le même jeu de clés : tout relire.
    if since > st.seq {
        since = 0;
    }
    for _ in 0..MAX_ROUNDS {
        let page = authed(auth, |t| api.changes(t, since, PAGE))?;
        let mut changes = Vec::with_capacity(page.items.len());
        for item in &page.items {
            if let Some(c) = decode(&key, item) {
                changes.push(c);
            } else {
                tracing::warn!("élément de synchro illisible, ignoré");
            }
        }
        let seq = page.seq;
        db.with(|c| {
            let applied = replica::apply(c, &changes, &me)?;
            kv::set(c, K_SEQ, &seq.to_string())?;
            Ok(applied)
        })?;
        report.pulled += changes.len();
        since = seq;
        if !page.more {
            break;
        }
    }

    for _ in 0..MAX_ROUNDS {
        let pending = db.with(|c| replica::pending(c, &me, PAGE))?;
        if pending.is_empty() {
            break;
        }
        let mut batch: Vec<Change> = Vec::new();
        let mut items: Vec<Item> = Vec::new();
        let mut size = 0;
        for change in pending {
            let item = encode(&key, &change)?;
            let len = item.v.len() + item.k.len() + 16;
            if !items.is_empty() && size + len > MAX_BATCH_BYTES {
                flush(db, api, auth, &me, &mut batch, &mut items, &mut report)?;
                size = 0;
            }
            size += len;
            items.push(item);
            batch.push(change);
        }
        flush(db, api, auth, &me, &mut batch, &mut items, &mut report)?;
    }
    Ok(report)
}

fn flush(
    db: &impl Db,
    api: &impl CloudApi,
    auth: &impl Auth,
    me: &str,
    batch: &mut Vec<Change>,
    items: &mut Vec<Item>,
    report: &mut Report,
) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let seq = authed(auth, |t| api.push(t, items))?;
    db.with(|c| {
        replica::mark_pushed(c, batch, me)?;
        // Nos propres envois n'ont pas à être relus, sauf si un autre appareil
        // a écrit entre-temps (sa version arrivera au prochain cycle).
        let known: i64 = kv::get(c, K_SEQ)?.and_then(|s| s.parse().ok()).unwrap_or(0);
        if known + i64::try_from(items.len()).unwrap_or(i64::MAX) == seq {
            kv::set(c, K_SEQ, &seq.to_string())?;
        }
        Ok(())
    })?;
    report.pushed += items.len();
    batch.clear();
    items.clear();
    Ok(())
}

fn encode(key: &SyncKey, change: &Change) -> Result<Item> {
    let k = key.item_id(&change.t, &change.p);
    let plain = Zeroizing::new(serde_json::to_vec(change).map_err(|_| RemoteError::Decode)?);
    let v = B64.encode(key.seal(&k, &plain)?);
    Ok(Item { k, v })
}

fn decode(key: &SyncKey, item: &Item) -> Option<Change> {
    let sealed = B64.decode(&item.v).ok()?;
    let plain = key.open(&item.k, &sealed).ok()?;
    let change: Change = serde_json::from_slice(&plain).ok()?;
    // L'identifiant doit correspondre au contenu : un bloc déplacé est refusé.
    (key.item_id(&change.t, &change.p) == item.k).then_some(change)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

fn unhex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}
