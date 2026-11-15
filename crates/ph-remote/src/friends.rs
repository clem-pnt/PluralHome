//! Amis entre systèmes, via le serveur PluralHome.
//!
//! Le serveur relie des comptes Aragonite mais n'expose jamais leur identifiant :
//! un ami est désigné par l'identifiant de l'amitié. Tout ce qui circule (fiches,
//! invitations, messages) est scellé ici, pour le seul destinataire ; le serveur ne
//! voit que des clés publiques et des blocs illisibles.
//!
//! Une invitation est un lien `pluralhome://ami/<id>/<secret>` (ou son QR code) :
//! l'identifiant désigne le blob côté serveur, le secret (jamais envoyé au serveur)
//! permet de l'ouvrir. Qui a le lien peut demander à devenir ami ; la demande doit
//! encore être acceptée.

use std::collections::BTreeMap;

use ph_core::cloudkey::InviteSecret;
use ph_core::friends::{self as core, InviteCard, RemoteFriend};
use ph_core::rusqlite::Connection;
use ph_core::{CoreError, kv, outbox};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::cloud::{Ack, Auth, HttpCloud, Id, authed, hex, path_id, unhex32};
use crate::db::Db;
use crate::{RemoteError, Result};

/// Clé publique enregistrée auprès du serveur (locale à l'appareil).
const K_REGISTERED: &str = "friends.registered";
/// Dernier courrier lu (local).
const K_MAIL_SEQ: &str = "friends.mailseq";
/// Empreinte de la dernière liste d'amis reçue (locale).
const K_LIST_HASH: &str = "friends.listhash";
const K_INVITE: &str = "friends.invite.";
const K_OUTGOING: &str = "friends.outgoing.";
const K_BLOCKED: &str = "friends.blocked.";

const LINK_PREFIX: &str = "pluralhome://ami/";
const MIN_TTL: i64 = 10 * 60_000;
const MAX_TTL: i64 = 90 * 24 * 3_600_000;
const MAIL_BATCH: usize = 50;
const MAIL_BATCH_BYTES: usize = 6 * 1024 * 1024;
const MAX_ROUNDS: usize = 50;

// ---------- API ----------

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteIdentity {
    #[serde(rename = "pub")]
    pub public: Option<String>,
    pub open: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInvite {
    pub id: String,
    pub once: bool,
    pub expires_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteIncoming {
    pub id: String,
    #[serde(rename = "pub")]
    pub public: String,
    pub card: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteOutgoing {
    pub id: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RemoteRequests {
    pub incoming: Vec<RemoteIncoming>,
    pub outgoing: Vec<RemoteOutgoing>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireFriend {
    pub id: String,
    #[serde(rename = "pub")]
    pub public: String,
    pub card: Option<String>,
    pub card_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBlock {
    pub id: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MailItem {
    pub s: i64,
    pub friend: String,
    pub v: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MailPage {
    pub items: Vec<MailItem>,
    pub more: bool,
}

/// Opérations du service d'amis, appelées avec un jeton d'accès Aragonite.
pub trait FriendsApi {
    fn identity(&self, t: &str) -> Result<RemoteIdentity>;
    fn put_identity(&self, t: &str, public: &str, replace: bool) -> Result<()>;
    fn set_open(&self, t: &str, open: bool) -> Result<()>;
    fn invites(&self, t: &str) -> Result<Vec<RemoteInvite>>;
    fn invite_create(&self, t: &str, card: &str, ttl: Option<i64>, once: bool) -> Result<String>;
    fn invite_read(&self, t: &str, id: &str) -> Result<String>;
    fn invite_revoke(&self, t: &str, id: &str) -> Result<()>;
    fn requests(&self, t: &str) -> Result<RemoteRequests>;
    fn request_send(&self, t: &str, invite: &str, card: &str) -> Result<String>;
    fn request_accept(&self, t: &str, id: &str, card: &str) -> Result<String>;
    fn request_block(&self, t: &str, id: &str) -> Result<String>;
    fn request_drop(&self, t: &str, id: &str) -> Result<()>;
    fn friends(&self, t: &str) -> Result<Vec<WireFriend>>;
    fn put_card(&self, t: &str, id: &str, card: &str) -> Result<()>;
    fn mail_send(&self, t: &str, id: &str, items: &[String]) -> Result<()>;
    fn block_friend(&self, t: &str, id: &str) -> Result<String>;
    fn unfriend(&self, t: &str, id: &str) -> Result<()>;
    fn blocks(&self, t: &str) -> Result<Vec<RemoteBlock>>;
    fn unblock(&self, t: &str, id: &str) -> Result<()>;
    fn mail(&self, t: &str, after: i64) -> Result<MailPage>;
    fn mail_ack(&self, t: &str, up_to: i64) -> Result<()>;
}

fn body<T: Serialize>(v: &T) -> Result<Value> {
    serde_json::to_value(v).map_err(|_| RemoteError::Decode)
}

#[derive(Deserialize)]
struct Invites {
    invites: Vec<RemoteInvite>,
}
#[derive(Deserialize)]
struct CardBody {
    card: String,
}
#[derive(Deserialize)]
struct Friends {
    friends: Vec<WireFriend>,
}
#[derive(Deserialize)]
struct Blocks {
    blocks: Vec<RemoteBlock>,
}

impl FriendsApi for HttpCloud {
    fn identity(&self, t: &str) -> Result<RemoteIdentity> {
        self.call(Method::GET, "/v1/friends/identity", t, None)
    }
    fn put_identity(&self, t: &str, public: &str, replace: bool) -> Result<()> {
        let b = json!({ "pub": public, "replace": replace });
        self.call::<Ack>(Method::PUT, "/v1/friends/identity", t, Some(&b))
            .map(|_| ())
    }
    fn set_open(&self, t: &str, open: bool) -> Result<()> {
        let b = json!({ "open": open });
        self.call::<Ack>(Method::PUT, "/v1/friends/settings", t, Some(&b))
            .map(|_| ())
    }
    fn invites(&self, t: &str) -> Result<Vec<RemoteInvite>> {
        self.call::<Invites>(Method::GET, "/v1/friends/invites", t, None)
            .map(|r| r.invites)
    }
    fn invite_create(&self, t: &str, card: &str, ttl: Option<i64>, once: bool) -> Result<String> {
        let b = json!({ "card": card, "ttl": ttl, "once": once });
        self.call::<Id>(Method::POST, "/v1/friends/invites", t, Some(&b))
            .map(|r| r.id)
    }
    fn invite_read(&self, t: &str, id: &str) -> Result<String> {
        let path = format!("/v1/friends/invites/{}", path_id(id)?);
        self.call::<CardBody>(Method::GET, &path, t, None)
            .map(|r| r.card)
    }
    fn invite_revoke(&self, t: &str, id: &str) -> Result<()> {
        let path = format!("/v1/friends/invites/{}", path_id(id)?);
        self.call::<Ack>(Method::DELETE, &path, t, None).map(|_| ())
    }
    fn requests(&self, t: &str) -> Result<RemoteRequests> {
        self.call(Method::GET, "/v1/friends/requests", t, None)
    }
    fn request_send(&self, t: &str, invite: &str, card: &str) -> Result<String> {
        let b = json!({ "invite": path_id(invite)?, "card": card });
        self.call::<Id>(Method::POST, "/v1/friends/requests", t, Some(&b))
            .map(|r| r.id)
    }
    fn request_accept(&self, t: &str, id: &str, card: &str) -> Result<String> {
        let path = format!("/v1/friends/requests/{}/accept", path_id(id)?);
        let b = json!({ "card": card });
        self.call::<Id>(Method::POST, &path, t, Some(&b))
            .map(|r| r.id)
    }
    fn request_block(&self, t: &str, id: &str) -> Result<String> {
        let path = format!("/v1/friends/requests/{}/block", path_id(id)?);
        self.call::<Id>(Method::POST, &path, t, None).map(|r| r.id)
    }
    fn request_drop(&self, t: &str, id: &str) -> Result<()> {
        let path = format!("/v1/friends/requests/{}", path_id(id)?);
        self.call::<Ack>(Method::DELETE, &path, t, None).map(|_| ())
    }
    fn friends(&self, t: &str) -> Result<Vec<WireFriend>> {
        self.call::<Friends>(Method::GET, "/v1/friends", t, None)
            .map(|r| r.friends)
    }
    fn put_card(&self, t: &str, id: &str, card: &str) -> Result<()> {
        let path = format!("/v1/friends/{}/card", path_id(id)?);
        let b = json!({ "card": card });
        self.call::<Ack>(Method::PUT, &path, t, Some(&b))
            .map(|_| ())
    }
    fn mail_send(&self, t: &str, id: &str, items: &[String]) -> Result<()> {
        let path = format!("/v1/friends/{}/mail", path_id(id)?);
        let b = body(&json!({ "items": items }))?;
        self.call::<Ack>(Method::POST, &path, t, Some(&b))
            .map(|_| ())
    }
    fn block_friend(&self, t: &str, id: &str) -> Result<String> {
        let path = format!("/v1/friends/{}/block", path_id(id)?);
        self.call::<Id>(Method::POST, &path, t, None).map(|r| r.id)
    }
    fn unfriend(&self, t: &str, id: &str) -> Result<()> {
        let path = format!("/v1/friends/{}", path_id(id)?);
        self.call::<Ack>(Method::DELETE, &path, t, None).map(|_| ())
    }
    fn blocks(&self, t: &str) -> Result<Vec<RemoteBlock>> {
        self.call::<Blocks>(Method::GET, "/v1/friends/blocks", t, None)
            .map(|r| r.blocks)
    }
    fn unblock(&self, t: &str, id: &str) -> Result<()> {
        let path = format!("/v1/friends/blocks/{}", path_id(id)?);
        self.call::<Ack>(Method::DELETE, &path, t, None).map(|_| ())
    }
    fn mail(&self, t: &str, after: i64) -> Result<MailPage> {
        self.call(Method::GET, &format!("/v1/mail?after={after}"), t, None)
    }
    fn mail_ack(&self, t: &str, up_to: i64) -> Result<()> {
        let b = json!({ "upTo": up_to });
        self.call::<Ack>(Method::POST, "/v1/mail/ack", t, Some(&b))
            .map(|_| ())
    }
}

// ---------- types exposés ----------

/// Invitation prête à partager.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Invite {
    pub id: String,
    /// `None` si le lien a été créé sur un autre appareil (le secret n'y est pas).
    pub link: Option<String>,
    pub once: bool,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub expires_at: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
}

/// Aperçu d'une invitation reçue, avant d'envoyer la demande.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct InvitePreview {
    pub name: Option<String>,
    pub avatar: Option<String>,
    /// Code de vérification, identique chez l'autre système.
    pub code: String,
    /// Le lien vient de notre propre système.
    pub own: bool,
    /// Déjà ami avec ce système.
    pub already: bool,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct IncomingRequest {
    pub id: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
    /// Membres montrés par la fiche ; `None` si elle est illisible.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub members: Option<usize>,
    pub code: String,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct OutgoingRequest {
    pub id: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub code: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Requests {
    pub incoming: Vec<IncomingRequest>,
    pub outgoing: Vec<OutgoingRequest>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Blocked {
    pub id: String,
    /// Nom connu au moment du blocage, s'il a été bloqué depuis cet appareil.
    pub name: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Accepte les nouvelles demandes (sinon, les liens existants ne marchent plus).
    pub open: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct FriendsReport {
    /// La liste ou les fiches des amis ont changé.
    pub changed: bool,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub sent: usize,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub received: usize,
}

#[derive(Serialize, Deserialize)]
struct SavedOutgoing {
    name: Option<String>,
    avatar: Option<String>,
    #[serde(rename = "pub")]
    public: String,
}

// ---------- liens ----------

#[must_use]
pub fn link(id: &str, secret: &InviteSecret) -> String {
    format!("{LINK_PREFIX}{id}/{}", secret.to_hex())
}

/// Lit un lien d'invitation, collé tel quel ou extrait d'un QR code. Tolère les
/// espaces et un texte autour.
pub fn parse_link(text: &str) -> Result<(String, InviteSecret)> {
    let bad = || RemoteError::Core(CoreError::Invalid("link"));
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let rest = compact
        .find(LINK_PREFIX)
        .map_or(compact.as_str(), |i| &compact[i + LINK_PREFIX.len()..]);
    let (id, secret) = rest.split_once('/').ok_or_else(bad)?;
    let secret: String = secret.chars().take_while(char::is_ascii_hexdigit).collect();
    if secret.len() != 32 {
        return Err(bad());
    }
    let id = path_id(id).map_err(|_| bad())?;
    let secret = InviteSecret::from_hex(&secret).map_err(|_| bad())?;
    Ok((id, secret))
}

// ---------- identité ----------

/// Enregistre notre clé publique auprès du serveur si besoin.
/// `friend_identity_conflict` si le compte en a déjà une autre : un autre appareil
/// a créé l'identité et la synchro ne l'a pas encore apportée ici, ou les
/// données ont été perdues (voir [`take_over_identity`]).
pub fn ensure_identity(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth) -> Result<()> {
    let (public, registered) = db.with(|c| {
        let p = hex(&core::identity(c)?.public());
        let r = kv::get(c, K_REGISTERED)?;
        Ok((p, r))
    })?;
    if registered.as_deref() == Some(public.as_str()) {
        return Ok(());
    }
    let remote = authed(auth, |t| api.identity(t))?;
    match remote.public {
        Some(p) if p == public => {}
        Some(_) => return Err(RemoteError::Friend("friend_identity_conflict")),
        None => authed(auth, |t| api.put_identity(t, &public, false))?,
    }
    db.with(|c| kv::set(c, K_REGISTERED, &public))?;
    Ok(())
}

/// Impose l'identité de cet appareil au compte. Toutes les amitiés sont perdues.
pub fn take_over_identity(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth) -> Result<()> {
    let public = db.with(|c| Ok(hex(&core::identity(c)?.public())))?;
    authed(auth, |t| api.put_identity(t, &public, true))?;
    db.with(|c| {
        kv::set(c, K_REGISTERED, &public)?;
        core::reconcile(c, &[])?;
        clear_prefix(c, K_OUTGOING)?;
        clear_prefix(c, K_INVITE)?;
        clear_prefix(c, K_BLOCKED)
    })?;
    Ok(())
}

fn clear_prefix(c: &Connection, prefix: &str) -> ph_core::Result<()> {
    for (k, _) in kv::list_prefix(c, prefix)? {
        kv::delete(c, &k)?;
    }
    Ok(())
}

pub fn settings(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth) -> Result<Settings> {
    ensure_identity(db, api, auth)?;
    let r = authed(auth, |t| api.identity(t))?;
    Ok(Settings { open: r.open })
}

pub fn set_open(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth, open: bool) -> Result<()> {
    ensure_identity(db, api, auth)?;
    authed(auth, |t| api.set_open(t, open))
}

// ---------- invitations ----------

/// Nouvelle invitation. `ttl` en millisecondes ; `None` : sans limite de durée.
pub fn create_invite(
    db: &impl Db,
    api: &impl FriendsApi,
    auth: &impl Auth,
    ttl: Option<i64>,
    once: bool,
) -> Result<Invite> {
    if ttl.is_some_and(|t| !(MIN_TTL..=MAX_TTL).contains(&t)) {
        return Err(RemoteError::Core(CoreError::Invalid("ttl")));
    }
    ensure_identity(db, api, auth)?;
    let secret = InviteSecret::generate()?;
    let card = db.with(|c| core::invite_card(c, &secret))?;
    let id = authed(auth, |t| api.invite_create(t, &card, ttl, once))?;
    db.with(|c| kv::set(c, &format!("{K_INVITE}{id}"), &secret.to_hex()))?;
    let now = crate::cloud::now_ms();
    Ok(Invite {
        link: Some(link(&id, &secret)),
        id,
        once,
        expires_at: ttl.map(|t| now + t),
        created_at: now,
    })
}

pub fn invites(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth) -> Result<Vec<Invite>> {
    let list = authed(auth, |t| api.invites(t))?;
    db.with(|c| {
        let saved = kv::list_prefix(c, K_INVITE)?;
        let mut out = Vec::new();
        for i in list {
            let secret = saved
                .iter()
                .find(|(k, _)| k[K_INVITE.len()..] == i.id)
                .and_then(|(_, v)| InviteSecret::from_hex(v).ok());
            out.push(Invite {
                link: secret.map(|s| link(&i.id, &s)),
                id: i.id,
                once: i.once,
                expires_at: i.expires_at,
                created_at: i.created_at,
            });
        }
        for (k, _) in saved {
            if !out.iter().any(|i| k[K_INVITE.len()..] == i.id) {
                kv::delete(c, &k)?;
            }
        }
        Ok(out)
    })
    .map_err(Into::into)
}

pub fn revoke_invite(
    db: &impl Db,
    api: &impl FriendsApi,
    auth: &impl Auth,
    id: &str,
) -> Result<()> {
    authed(auth, |t| api.invite_revoke(t, id))?;
    db.with(|c| kv::delete(c, &format!("{K_INVITE}{id}")))?;
    Ok(())
}

fn read_invite(
    api: &impl FriendsApi,
    auth: &impl Auth,
    text: &str,
) -> Result<(String, InviteCard, [u8; 32])> {
    let (id, secret) = parse_link(text)?;
    let sealed = authed(auth, |t| api.invite_read(t, &id))?;
    let (card, public) = core::open_invite(&secret, &sealed)
        .map_err(|_| RemoteError::Friend("friend_invite_invalid"))?;
    Ok((id, card, public))
}

pub fn preview_invite(
    db: &impl Db,
    api: &impl FriendsApi,
    auth: &impl Auth,
    text: &str,
) -> Result<InvitePreview> {
    let (_, card, public) = read_invite(api, auth, text)?;
    db.with(|c| {
        let own = core::identity(c)?.public() == public;
        let code = if own {
            String::new()
        } else {
            core::code_with(c, &public)?
        };
        let already = !own && core::list(c)?.iter().any(|f| f.code == code);
        Ok(InvitePreview {
            name: card.name,
            avatar: card.avatar,
            code,
            own,
            already,
        })
    })
    .map_err(Into::into)
}

/// Envoie une demande d'ami au système qui a créé le lien.
pub fn send_request(
    db: &impl Db,
    api: &impl FriendsApi,
    auth: &impl Auth,
    text: &str,
) -> Result<String> {
    ensure_identity(db, api, auth)?;
    let (invite, card, public) = read_invite(api, auth, text)?;
    let mine = db.with(|c| Ok(core::identity(c)?.public()))?;
    if mine == public {
        return Err(RemoteError::Friend("friend_self"));
    }
    let sealed = db.with(|c| core::sealed_card_for(c, &public))?;
    let id = authed(auth, |t| api.request_send(t, &invite, &sealed))?;
    let saved = SavedOutgoing {
        name: card.name,
        avatar: card.avatar,
        public: hex(&public),
    };
    let raw = serde_json::to_string(&saved).map_err(|_| RemoteError::Decode)?;
    db.with(|c| kv::set(c, &format!("{K_OUTGOING}{id}"), &raw))?;
    Ok(id)
}

// ---------- demandes ----------

pub fn requests(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth) -> Result<Requests> {
    let remote = authed(auth, |t| api.requests(t))?;
    db.with(|c| {
        let mut out = Requests::default();
        for r in remote.incoming {
            let Some(public) = unhex32(&r.public) else {
                continue;
            };
            let card = core::open_card(c, &public, &r.card).ok();
            out.incoming.push(IncomingRequest {
                id: r.id,
                name: card.as_ref().and_then(|k| k.name.clone()),
                avatar: card.as_ref().and_then(|k| k.avatar.clone()),
                members: card.as_ref().map(|k| k.members.len()),
                code: core::code_with(c, &public)?,
                created_at: r.created_at,
            });
        }
        let saved = kv::list_prefix(c, K_OUTGOING)?;
        for r in remote.outgoing {
            let info = saved
                .iter()
                .find(|(k, _)| k[K_OUTGOING.len()..] == r.id)
                .and_then(|(_, v)| serde_json::from_str::<SavedOutgoing>(v).ok());
            let code = match info.as_ref().and_then(|i| unhex32(&i.public)) {
                Some(p) => Some(core::code_with(c, &p)?),
                None => None,
            };
            out.outgoing.push(OutgoingRequest {
                name: info.as_ref().and_then(|i| i.name.clone()),
                avatar: info.and_then(|i| i.avatar),
                code,
                id: r.id,
                created_at: r.created_at,
            });
        }
        for (k, _) in saved {
            if !out.outgoing.iter().any(|o| k[K_OUTGOING.len()..] == o.id) {
                kv::delete(c, &k)?;
            }
        }
        Ok(out)
    })
    .map_err(Into::into)
}

/// Accepte une demande : notre fiche part vers le demandeur, l'amitié est créée.
pub fn accept(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth, id: &str) -> Result<String> {
    ensure_identity(db, api, auth)?;
    let remote = authed(auth, |t| api.requests(t))?;
    let req = remote
        .incoming
        .into_iter()
        .find(|r| r.id == id)
        .ok_or(RemoteError::Friend("friend_request_gone"))?;
    let public = unhex32(&req.public).ok_or(RemoteError::Decode)?;
    let sealed = db.with(|c| core::sealed_card_for(c, &public))?;
    let friend = authed(auth, |t| api.request_accept(t, id, &sealed))?;
    refresh(db, api, auth)?;
    Ok(friend)
}

/// Refuse une demande reçue ou annule une demande envoyée.
pub fn drop_request(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth, id: &str) -> Result<()> {
    authed(auth, |t| api.request_drop(t, id))?;
    db.with(|c| kv::delete(c, &format!("{K_OUTGOING}{id}")))?;
    Ok(())
}

pub fn block_request(
    db: &impl Db,
    api: &impl FriendsApi,
    auth: &impl Auth,
    id: &str,
    name: Option<&str>,
) -> Result<()> {
    let block = authed(auth, |t| api.request_block(t, id))?;
    remember_block(db, &block, name)
}

fn remember_block(db: &impl Db, block: &str, name: Option<&str>) -> Result<()> {
    let name = name.unwrap_or_default();
    db.with(|c| kv::set(c, &format!("{K_BLOCKED}{block}"), name))?;
    Ok(())
}

// ---------- amis ----------

pub fn unfriend(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth, id: &str) -> Result<()> {
    authed(auth, |t| api.unfriend(t, id))?;
    db.with(|c| core::forget(c, id))?;
    Ok(())
}

pub fn block_friend(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth, id: &str) -> Result<()> {
    let name = db.with(|c| Ok(core::get(c, id).ok().and_then(|f| f.nickname.or(f.name))))?;
    let block = authed(auth, |t| api.block_friend(t, id))?;
    db.with(|c| core::forget(c, id))?;
    remember_block(db, &block, name.as_deref())
}

pub fn blocks(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth) -> Result<Vec<Blocked>> {
    let list = authed(auth, |t| api.blocks(t))?;
    db.with(|c| {
        let saved = kv::list_prefix(c, K_BLOCKED)?;
        let out: Vec<Blocked> = list
            .into_iter()
            .map(|b| Blocked {
                name: saved
                    .iter()
                    .find(|(k, _)| k[K_BLOCKED.len()..] == b.id)
                    .map(|(_, v)| v.clone())
                    .filter(|v| !v.is_empty()),
                id: b.id,
                created_at: b.created_at,
            })
            .collect();
        for (k, _) in saved {
            if !out.iter().any(|b| k[K_BLOCKED.len()..] == b.id) {
                kv::delete(c, &k)?;
            }
        }
        Ok(out)
    })
    .map_err(Into::into)
}

pub fn unblock(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth, id: &str) -> Result<()> {
    authed(auth, |t| api.unblock(t, id))?;
    db.with(|c| kv::delete(c, &format!("{K_BLOCKED}{id}")))?;
    Ok(())
}

// ---------- synchro ----------

/// Met à jour la liste des amis et publie nos fiches. `true` si la liste a changé.
fn refresh(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth) -> Result<bool> {
    let wire = authed(auth, |t| api.friends(t))?;
    let mut h = Sha256::new();
    let mut remote = Vec::with_capacity(wire.len());
    for f in wire {
        let Some(public) = unhex32(&f.public) else {
            continue;
        };
        h.update(f.id.as_bytes());
        h.update(public);
        h.update(f.card_at.unwrap_or_default().to_le_bytes());
        remote.push(RemoteFriend {
            id: f.id,
            public,
            card: f.card,
            card_at: f.card_at,
            created_at: f.created_at,
        });
    }
    let hash = hex(&h.finalize());
    let (changed, cards) = db.with(|c| {
        let changed = kv::get(c, K_LIST_HASH)?.as_deref() != Some(hash.as_str());
        if changed {
            core::reconcile(c, &remote)?;
            kv::set(c, K_LIST_HASH, &hash)?;
        }
        Ok((changed, core::cards_to_publish(c)?))
    })?;
    for (id, sealed, hash) in cards {
        match authed(auth, |t| api.put_card(t, &id, &sealed)) {
            Ok(()) => db.with(|c| core::card_published(c, &id, &hash))?,
            // L'amitié vient de disparaître : la prochaine liste la retirera.
            Err(RemoteError::Friend("friend_gone")) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(changed)
}

/// Envoie le courrier en attente, ami par ami. Renvoie le nombre d'envois.
fn flush(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth) -> Result<usize> {
    let mut sent = 0;
    for _ in 0..MAX_ROUNDS {
        let pending = db.with(|c| core::outgoing(c, 200))?;
        if pending.is_empty() {
            break;
        }
        let mut by_friend: BTreeMap<String, Vec<(i64, String)>> = BTreeMap::new();
        for o in pending {
            by_friend
                .entry(o.friend)
                .or_default()
                .push((o.outbox_id, o.item));
        }
        let mut progressed = false;
        for (friend, items) in by_friend {
            for batch in batches(items) {
                let payload: Vec<String> = batch.iter().map(|(_, v)| v.clone()).collect();
                match authed(auth, |t| api.mail_send(t, &friend, &payload)) {
                    Ok(()) | Err(RemoteError::Friend("friend_gone")) => {
                        db.with(|c| {
                            for (id, _) in &batch {
                                outbox::done(c, *id)?;
                            }
                            Ok(())
                        })?;
                        sent += batch.len();
                        progressed = true;
                    }
                    // Boîte de l'ami pleine : on garde pour plus tard.
                    Err(RemoteError::Friend("friend_mailbox_full")) => {
                        db.with(|c| {
                            for (id, _) in &batch {
                                outbox::failed(c, *id, "mailbox_full")?;
                            }
                            Ok(())
                        })?;
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        if !progressed {
            break;
        }
    }
    Ok(sent)
}

fn batches(items: Vec<(i64, String)>) -> Vec<Vec<(i64, String)>> {
    let mut out: Vec<Vec<(i64, String)>> = Vec::new();
    let mut size = 0;
    for it in items {
        let fits = out
            .last()
            .is_some_and(|b| b.len() < MAIL_BATCH && size + it.1.len() <= MAIL_BATCH_BYTES);
        if !fits {
            out.push(Vec::new());
            size = 0;
        }
        size += it.1.len();
        if let Some(b) = out.last_mut() {
            b.push(it);
        }
    }
    out
}

/// Relève le courrier. Renvoie le nombre de courriers appliqués.
fn collect(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth) -> Result<usize> {
    let mut after = db.with(|c| {
        Ok(kv::get(c, K_MAIL_SEQ)?
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0))
    })?;
    let mut received = 0;
    for _ in 0..MAX_ROUNDS {
        let page = authed(auth, |t| api.mail(t, after))?;
        let Some(last) = page.items.last().map(|i| i.s) else {
            break;
        };
        received += db.with(|c| {
            let mut n = 0;
            for item in &page.items {
                if core::receive(c, &item.friend, &item.v)? {
                    n += 1;
                }
            }
            kv::set(c, K_MAIL_SEQ, &last.to_string())?;
            Ok(n)
        })?;
        after = last;
        authed(auth, |t| api.mail_ack(t, last))?;
        if !page.more {
            break;
        }
    }
    Ok(received)
}

/// Cycle complet : amis et fiches, envoi puis relève du courrier.
pub fn sync(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth) -> Result<FriendsReport> {
    ensure_identity(db, api, auth)?;
    let changed = refresh(db, api, auth)?;
    let sent = flush(db, api, auth)?;
    let received = collect(db, api, auth)?;
    Ok(FriendsReport {
        changed,
        sent,
        received,
    })
}

/// Fiche à jour pour tous les amis (après un changement de réglages ou de membres).
pub fn publish(db: &impl Db, api: &impl FriendsApi, auth: &impl Auth) -> Result<()> {
    refresh(db, api, auth).map(|_| ())
}
