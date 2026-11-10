//! Compte Aragonite (SSO), en client public OAuth 2 : code d'autorisation + PKCE S256,
//! redirection vers une adresse de bouclage (RFC 8252).
//!
//! Le compte ne sert qu'à l'identité (nom affiché, Discord rattaché). Aucune donnée
//! du coffre ne part vers le SSO. Seul le jeton de renouvellement est gardé, dans le
//! coffre chiffré ; le jeton d'accès vit en mémoire le temps d'un appel. Le SSO fait
//! tourner les jetons de renouvellement et ferme la session si un ancien est rejoué :
//! le nouveau est donc enregistré avant toute autre chose.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ph_core::kv;
use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;
use zeroize::Zeroizing;

use crate::db::Db;
use crate::{RemoteError, Result, USER_AGENT};

pub const SSO_BASE: &str = "https://sso.aragonite.fr";
pub const CLIENT_ID: &str = "pluralhome";

const K_REFRESH: &str = "aragonite.refresh_token";
const K_ACCOUNT: &str = "aragonite.account";

/// Compte tel qu'affiché dans l'application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub display_name: String,
    pub discord: Option<DiscordIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DiscordIdentity {
    pub id: String,
    pub username: String,
    pub display_name: String,
}

// ---------- PKCE ----------

fn random_b64(bytes: usize) -> Result<Zeroizing<String>> {
    let mut buf = Zeroizing::new(vec![0u8; bytes]);
    getrandom::fill(&mut buf).map_err(|_| RemoteError::Core(ph_core::CoreError::Crypto))?;
    Ok(Zeroizing::new(URL_SAFE_NO_PAD.encode(&*buf)))
}

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Comparaison en temps constant (le `state` est un secret de courte durée).
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

// ---------- client HTTP ----------

pub struct Sso {
    http: Client,
    base: Url,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Zeroizing<String>,
    refresh_token: Zeroizing<String>,
}

#[derive(Deserialize)]
struct UserInfo {
    user: SsoUser,
}

#[derive(Deserialize)]
struct SsoUser {
    profile: SsoProfile,
    #[serde(default)]
    identities: Vec<SsoIdentity>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SsoProfile {
    display_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SsoIdentity {
    provider: String,
    provider_user_id: String,
    username: Option<String>,
    display_name: Option<String>,
    #[serde(default)]
    primary: bool,
}

impl SsoUser {
    fn into_account(self) -> Account {
        let mut discord: Vec<SsoIdentity> = self
            .identities
            .into_iter()
            .filter(|i| i.provider == "discord")
            .collect();
        discord.sort_by_key(|i| !i.primary);
        let discord = discord.into_iter().next().map(|i| {
            let username = i.username.unwrap_or_default();
            DiscordIdentity {
                id: i.provider_user_id,
                display_name: i.display_name.unwrap_or_else(|| username.clone()),
                username,
            }
        });
        let display_name = self
            .profile
            .display_name
            .filter(|n| !n.trim().is_empty())
            .or_else(|| discord.as_ref().map(|d| d.display_name.clone()))
            .unwrap_or_default();
        Account {
            display_name,
            discord,
        }
    }
}

impl Sso {
    pub fn new() -> Result<Self> {
        Self::with_base(SSO_BASE)
    }

    pub fn with_base(base: &str) -> Result<Self> {
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(20))
            .https_only(!base.starts_with("http://127.0.0.1"))
            .build()?;
        let base = Url::parse(base).map_err(|_| RemoteError::Decode)?;
        Ok(Self { http, base })
    }

    fn url(&self, path: &str) -> Url {
        self.base.join(path).unwrap_or_else(|_| self.base.clone())
    }

    fn token(&self, form: &[(&str, &str)]) -> Result<TokenResponse> {
        let res = self.http.post(self.url("/oauth/token")).form(form).send()?;
        match res.status() {
            s if s.is_success() => Ok(res.json()?),
            StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED => Err(RemoteError::Unauthorized),
            StatusCode::TOO_MANY_REQUESTS => Err(RemoteError::RateLimited),
            s => Err(RemoteError::Http {
                status: s.as_u16(),
                message: String::new(),
            }),
        }
    }

    fn userinfo(&self, access: &str) -> Result<Account> {
        let res = self
            .http
            .post(self.url("/oauth/userinfo"))
            .bearer_auth(access)
            .form(&[("client_id", CLIENT_ID)])
            .send()?;
        match res.status() {
            s if s.is_success() => Ok(res.json::<UserInfo>()?.user.into_account()),
            StatusCode::UNAUTHORIZED => Err(RemoteError::Unauthorized),
            StatusCode::TOO_MANY_REQUESTS => Err(RemoteError::RateLimited),
            s => Err(RemoteError::Http {
                status: s.as_u16(),
                message: String::new(),
            }),
        }
    }

    fn revoke(&self, token: &str) -> Result<()> {
        self.http
            .post(self.url("/oauth/revoke"))
            .form(&[("client_id", CLIENT_ID), ("token", token)])
            .send()?
            .error_for_status()?;
        Ok(())
    }
}

// ---------- connexion par le navigateur ----------

/// Connexion en cours : le navigateur doit ouvrir `authorize_url`, puis
/// [`Login::finish`] attend son retour sur l'adresse de bouclage.
pub struct Login {
    listener: TcpListener,
    redirect_uri: String,
    state: Zeroizing<String>,
    verifier: Zeroizing<String>,
    pub authorize_url: String,
}

impl Login {
    pub fn begin(sso: &Sso) -> Result<Self> {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .map_err(|e| RemoteError::Network(e.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|e| RemoteError::Network(e.to_string()))?
            .port();
        let redirect_uri = format!("http://127.0.0.1:{port}/callback");
        let state = random_b64(24)?;
        let verifier = random_b64(48)?;
        let mut url = sso.url("/oauth/authorize");
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", CLIENT_ID)
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("state", &state)
            .append_pair("code_challenge", &challenge(&verifier))
            .append_pair("code_challenge_method", "S256");
        Ok(Self {
            listener,
            redirect_uri,
            state,
            verifier,
            authorize_url: url.into(),
        })
    }

    /// Attend le retour du navigateur (jusqu'à `timeout`, ou `cancel`), échange le
    /// code, enregistre la session dans le coffre et renvoie le compte.
    pub fn finish(
        self,
        db: &impl Db,
        sso: &Sso,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<Account> {
        let code = self.wait_code(timeout, cancel)?;
        let tokens = sso.token(&[
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", &code),
            ("redirect_uri", &self.redirect_uri),
            ("code_verifier", &self.verifier),
            ("device_name", device_name()),
        ])?;
        store_refresh(db, &tokens.refresh_token)?;
        let account = sso.userinfo(&tokens.access_token)?;
        store_account(db, &account)?;
        Ok(account)
    }

    fn wait_code(&self, timeout: Duration, cancel: &AtomicBool) -> Result<Zeroizing<String>> {
        self.listener
            .set_nonblocking(true)
            .map_err(|e| RemoteError::Network(e.to_string()))?;
        let deadline = Instant::now() + timeout;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(RemoteError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(RemoteError::Timeout);
            }
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if let Some(result) = self.handle(stream) {
                        return result;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => return Err(RemoteError::Network(e.to_string())),
            }
        }
    }

    /// `None` : requête sans rapport (favicon…), on continue d'attendre.
    fn handle(&self, mut stream: TcpStream) -> Option<Result<Zeroizing<String>>> {
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        // Une ligne de requête ne dépasse jamais quelques kilo-octets.
        let mut line = String::new();
        BufReader::new((&stream).take(8192))
            .read_line(&mut line)
            .ok()
            .filter(|n| *n > 0)?;
        let target = line.split(' ').nth(1)?;
        let url = Url::parse("http://127.0.0.1").ok()?.join(target).ok()?;
        if url.path() != "/callback" {
            respond(&mut stream, "404 Not Found", "");
            return None;
        }
        let param = |k: &str| {
            url.query_pairs()
                .find(|(n, _)| n == k)
                .map(|(_, v)| v.into_owned())
        };
        let state_ok = param("state").is_some_and(|s| same(&s, &self.state));
        let result = match (state_ok, param("code"), param("error")) {
            (true, Some(code), None) => Ok(Zeroizing::new(code)),
            (true, _, Some(e)) if e == "access_denied" => Err(RemoteError::Cancelled),
            (true, ..) => Err(RemoteError::Unauthorized),
            // Mauvais `state` : ce n'est pas notre retour, on l'ignore.
            (false, ..) => {
                respond(&mut stream, "400 Bad Request", PAGE_ERROR);
                return None;
            }
        };
        respond(
            &mut stream,
            "200 OK",
            if result.is_ok() { PAGE_OK } else { PAGE_ERROR },
        );
        Some(result)
    }
}

const PAGE_OK: &str = "Connexion réussie. Vous pouvez fermer cet onglet et revenir à PluralHome.";
const PAGE_ERROR: &str = "La connexion n'a pas abouti. Revenez à PluralHome pour réessayer.";

fn respond(stream: &mut TcpStream, status: &str, text: &str) {
    let body = if text.is_empty() {
        String::new()
    } else {
        format!(
            "<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\"><title>PluralHome</title>\
             <body style=\"font:17px/1.5 system-ui,sans-serif;background:#f6f3ee;color:#2e3a3f;\
             display:grid;place-items:center;min-height:90vh;margin:0\"><p>{text}</p></body></html>"
        )
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();
}

fn device_name() -> &'static str {
    if cfg!(target_os = "android") {
        "PluralHome Android"
    } else {
        "PluralHome Linux"
    }
}

// ---------- session enregistrée ----------

fn store_refresh(db: &impl Db, token: &str) -> Result<()> {
    Ok(db.with(|c| kv::set(c, K_REFRESH, token))?)
}

fn store_account(db: &impl Db, account: &Account) -> Result<()> {
    let json = serde_json::to_string(account).map_err(|_| RemoteError::Decode)?;
    Ok(db.with(|c| kv::set(c, K_ACCOUNT, &json))?)
}

/// Compte connu localement (sans réseau).
pub fn account(db: &impl Db) -> Result<Option<Account>> {
    let raw = db.with(|c| kv::get(c, K_ACCOUNT))?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
}

/// Relit le profil auprès du SSO (nom, Discord). Une session refusée est oubliée.
pub fn refresh(db: &impl Db, sso: &Sso) -> Result<Account> {
    let refresh = db
        .with(|c| kv::get(c, K_REFRESH))?
        .map(Zeroizing::new)
        .ok_or(RemoteError::NotLinked)?;
    let tokens = match sso.token(&[
        ("grant_type", "refresh_token"),
        ("client_id", CLIENT_ID),
        ("refresh_token", &refresh),
    ]) {
        Ok(t) => t,
        Err(RemoteError::Unauthorized) => {
            forget(db)?;
            return Err(RemoteError::Unauthorized);
        }
        Err(e) => return Err(e),
    };
    store_refresh(db, &tokens.refresh_token)?;
    let account = sso.userinfo(&tokens.access_token)?;
    store_account(db, &account)?;
    Ok(account)
}

/// Déconnexion : session fermée côté SSO si possible, oubliée ici dans tous les cas.
pub fn logout(db: &impl Db, sso: &Sso) -> Result<()> {
    if let Some(token) = db.with(|c| kv::get(c, K_REFRESH))?.map(Zeroizing::new)
        && let Err(e) = sso.revoke(&token)
    {
        tracing::warn!(
            code = e.code(),
            "révocation de la session Aragonite impossible"
        );
    }
    forget(db)
}

fn forget(db: &impl Db) -> Result<()> {
    Ok(db.with(|c| {
        kv::delete(c, K_REFRESH)?;
        kv::delete(c, K_ACCOUNT)
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_s256() {
        // Valeur attendue calculée à part (Python hashlib + base64url sans remplissage).
        assert_eq!(
            challenge("dBjftJeZ4CVP-mJ92K9qkd7Ac7gXbyB-yYHTH8Ov3eA"),
            "eeeUIXdpUVN5nujQLvbOdjoO4KhrjAz43oo6pBeQMv4"
        );
        let v = random_b64(48).unwrap();
        assert_eq!(v.len(), 64);
        assert_eq!(challenge(&v).len(), 43);
    }

    #[test]
    fn account_prefers_primary_discord() {
        let user: SsoUser = serde_json::from_value(serde_json::json!({
            "profile": { "displayName": "" },
            "identities": [
                { "provider": "google", "providerUserId": "g1", "username": "x" },
                { "provider": "discord", "providerUserId": "1", "username": "old", "displayName": "Old" },
                { "provider": "discord", "providerUserId": "2", "username": "mila", "displayName": "Mila", "primary": true }
            ]
        }))
        .unwrap();
        let a = user.into_account();
        assert_eq!(a.display_name, "Mila");
        assert_eq!(a.discord.unwrap().id, "2");
    }

    #[test]
    fn loopback_returns_code_and_ignores_strangers() {
        let sso = Sso::with_base("http://127.0.0.1:9").unwrap();
        let login = Login::begin(&sso).unwrap();
        let url = Url::parse(&login.authorize_url).unwrap();
        let q = |k: &str| {
            url.query_pairs()
                .find(|(n, _)| n == k)
                .unwrap()
                .1
                .into_owned()
        };
        assert_eq!(q("code_challenge"), challenge(&login.verifier));
        let (redirect, state) = (q("redirect_uri"), q("state"));
        let browser = std::thread::spawn(move || {
            let http = Client::new();
            assert_eq!(
                http.get(format!("{redirect}/../favicon.ico"))
                    .send()
                    .unwrap()
                    .status(),
                404
            );
            assert_eq!(
                http.get(format!("{redirect}?code=x&state=faux"))
                    .send()
                    .unwrap()
                    .status(),
                400
            );
            http.get(format!("{redirect}?code=le-code&state={state}"))
                .send()
                .unwrap()
                .status()
        });
        let code = login
            .wait_code(Duration::from_secs(10), &AtomicBool::new(false))
            .unwrap();
        assert_eq!(code.as_str(), "le-code");
        assert_eq!(browser.join().unwrap(), 200);
    }

    #[test]
    fn login_can_be_cancelled() {
        let sso = Sso::with_base("http://127.0.0.1:9").unwrap();
        let login = Login::begin(&sso).unwrap();
        let err = login
            .wait_code(Duration::from_secs(10), &AtomicBool::new(true))
            .unwrap_err();
        assert!(matches!(err, RemoteError::Cancelled));
    }

    #[test]
    fn constant_time_compare() {
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "ab"));
    }
}
