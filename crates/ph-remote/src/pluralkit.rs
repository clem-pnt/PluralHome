//! Client de l'API PluralKit v2 (<https://pluralkit.me/api/>).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use zeroize::Zeroizing;

use crate::{RemoteError, Result, USER_AGENT};

pub const API: &str = "https://api.pluralkit.me/v2";
/// Écritures limitées à 3 par seconde par PluralKit : on se tient un peu en dessous.
const WRITE_SPACING: Duration = Duration::from_millis(400);
const MAX_RETRY_WAIT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PkSystem {
    pub id: String,
    pub uuid: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PkMember {
    pub id: String,
    pub uuid: String,
    pub name: String,
    #[serde(default)]
    pub pronouns: Option<String>,
    /// Hexadécimal sans `#`.
    #[serde(default)]
    pub color: Option<String>,
}

/// Champs écrits par PluralHome ; les autres (avatar, description…) ne sont jamais touchés.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PkMemberPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `Some(None)` efface les pronoms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pronouns: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

impl PkMemberPatch {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.pronouns.is_none() && self.color.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkSwitch {
    pub id: String,
    /// Millisecondes Unix.
    pub timestamp: i64,
    /// Identifiants courts des membres présents (liste vide : personne).
    pub members: Vec<String>,
}

#[derive(Deserialize)]
struct RawSwitch {
    id: String,
    timestamp: String,
    members: Vec<Value>,
}

impl TryFrom<RawSwitch> for PkSwitch {
    type Error = RemoteError;
    fn try_from(raw: RawSwitch) -> Result<Self> {
        // La liste contient des identifiants ou des objets membres selon la route.
        let members = raw
            .members
            .into_iter()
            .map(|m| match m {
                Value::String(id) => Some(id),
                Value::Object(o) => o.get("id").and_then(Value::as_str).map(str::to_owned),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(RemoteError::Decode)?;
        Ok(Self {
            id: raw.id,
            timestamp: parse_time(&raw.timestamp)?,
            members,
        })
    }
}

pub fn parse_time(s: &str) -> Result<i64> {
    let t = OffsetDateTime::parse(s, &Rfc3339).map_err(|_| RemoteError::Decode)?;
    i64::try_from(t.unix_timestamp_nanos() / 1_000_000).map_err(|_| RemoteError::Decode)
}

pub fn format_time(ms: i64) -> Result<String> {
    let t = OffsetDateTime::from_unix_timestamp_nanos(i128::from(ms) * 1_000_000)
        .map_err(|_| RemoteError::Decode)?;
    t.format(&Rfc3339).map_err(|_| RemoteError::Decode)
}

/// Ce dont la synchro a besoin. Implémenté en HTTP, et en mémoire pour les tests.
pub trait PkApi: Sync {
    fn system(&self) -> Result<PkSystem>;
    fn members(&self) -> Result<Vec<PkMember>>;
    fn create_member(&self, patch: &PkMemberPatch) -> Result<PkMember>;
    fn update_member(&self, id: &str, patch: &PkMemberPatch) -> Result<PkMember>;
    /// Les plus récents d'abord.
    fn switches(&self, limit: u32) -> Result<Vec<PkSwitch>>;
    fn post_switch(&self, members: &[String], timestamp: i64) -> Result<PkSwitch>;
}

pub struct HttpPk {
    client: Client,
    base: String,
    token: Zeroizing<String>,
    last_write: Mutex<Option<Instant>>,
}

impl HttpPk {
    pub fn new(token: &str) -> Result<Self> {
        Self::with_base(API, token)
    }

    pub fn with_base(base: &str, token: &str) -> Result<Self> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(20))
            .https_only(base.starts_with("https://"))
            .build()?;
        Ok(Self {
            client,
            base: base.trim_end_matches('/').to_owned(),
            token: Zeroizing::new(token.trim().to_owned()),
            last_write: Mutex::new(None),
        })
    }

    fn pace_write(&self) {
        let mut last = self
            .last_write
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(at) = *last {
            let elapsed = at.elapsed();
            if elapsed < WRITE_SPACING {
                std::thread::sleep(WRITE_SPACING.saturating_sub(elapsed));
            }
        }
        *last = Some(Instant::now());
    }

    fn call<T: DeserializeOwned>(
        &self,
        method: &reqwest::Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Option<T>> {
        let write = method != reqwest::Method::GET;
        for attempt in 0..3 {
            if write {
                self.pace_write();
            }
            let mut req = self
                .client
                .request(method.clone(), format!("{}{path}", self.base))
                .header("Authorization", self.token.as_str());
            if let Some(b) = body {
                req = req.json(b);
            }
            let res = req.send()?;
            let status = res.status();
            if status == StatusCode::NO_CONTENT {
                return Ok(None);
            }
            if status.is_success() {
                return Ok(Some(res.json()?));
            }
            let payload: Value = res.json().unwrap_or(Value::Null);
            match status {
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                    return Err(RemoteError::Unauthorized);
                }
                StatusCode::TOO_MANY_REQUESTS if attempt < 2 => {
                    let ms = payload
                        .get("retry_after")
                        .and_then(Value::as_u64)
                        .unwrap_or(1000);
                    std::thread::sleep(Duration::from_millis(ms).min(MAX_RETRY_WAIT));
                }
                StatusCode::TOO_MANY_REQUESTS => return Err(RemoteError::RateLimited),
                _ => {
                    let message = payload
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .chars()
                        .take(200)
                        .collect();
                    return Err(RemoteError::Http {
                        status: status.as_u16(),
                        message,
                    });
                }
            }
        }
        Err(RemoteError::RateLimited)
    }

    fn expect<T: DeserializeOwned>(
        &self,
        method: &reqwest::Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<T> {
        self.call(method, path, body)?.ok_or(RemoteError::Decode)
    }
}

impl PkApi for HttpPk {
    fn system(&self) -> Result<PkSystem> {
        self.expect(&reqwest::Method::GET, "/systems/@me", None)
    }

    fn members(&self) -> Result<Vec<PkMember>> {
        self.expect(&reqwest::Method::GET, "/systems/@me/members", None)
    }

    fn create_member(&self, patch: &PkMemberPatch) -> Result<PkMember> {
        let body = serde_json::to_value(patch).map_err(|_| RemoteError::Decode)?;
        self.expect(&reqwest::Method::POST, "/members", Some(&body))
    }

    fn update_member(&self, id: &str, patch: &PkMemberPatch) -> Result<PkMember> {
        let body = serde_json::to_value(patch).map_err(|_| RemoteError::Decode)?;
        let id: String = id.chars().filter(char::is_ascii_alphanumeric).collect();
        self.expect(
            &reqwest::Method::PATCH,
            &format!("/members/{id}"),
            Some(&body),
        )
    }

    fn switches(&self, limit: u32) -> Result<Vec<PkSwitch>> {
        let raw: Vec<RawSwitch> = self.expect(
            &reqwest::Method::GET,
            &format!("/systems/@me/switches?limit={}", limit.clamp(1, 100)),
            None,
        )?;
        raw.into_iter().map(PkSwitch::try_from).collect()
    }

    fn post_switch(&self, members: &[String], timestamp: i64) -> Result<PkSwitch> {
        let body = serde_json::json!({ "members": members, "timestamp": format_time(timestamp)? });
        let raw: RawSwitch =
            self.expect(&reqwest::Method::POST, "/systems/@me/switches", Some(&body))?;
        raw.try_into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_members_as_ids_or_objects() {
        let ids: RawSwitch = serde_json::from_value(serde_json::json!({
            "id": "s1", "timestamp": "2026-11-10T12:00:00.123456Z", "members": ["abcde", "fghij"]
        }))
        .unwrap();
        let objs: RawSwitch = serde_json::from_value(serde_json::json!({
            "id": "s2", "timestamp": "2026-11-10T12:00:00Z", "members": [{"id": "abcde", "name": "Mila"}]
        }))
        .unwrap();
        let a = PkSwitch::try_from(ids).unwrap();
        assert_eq!(a.members, vec!["abcde", "fghij"]);
        assert_eq!(a.timestamp % 1000, 123);
        assert_eq!(PkSwitch::try_from(objs).unwrap().members, vec!["abcde"]);
    }

    #[test]
    fn time_roundtrip() {
        let ms = 1_794_316_800_123;
        assert_eq!(parse_time(&format_time(ms).unwrap()).unwrap(), ms);
    }

    #[test]
    fn patch_serialization_skips_untouched_fields() {
        let p = PkMemberPatch {
            pronouns: Some(None),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            serde_json::json!({ "pronouns": null })
        );
    }
}
