//! Nom et photo du système, partagés entre appareils (clés `system.*` de `kv`).
//! Le nom peut venir de PluralKit ; ce qui est saisi ici l'emporte toujours.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::members::{AvatarEdit, MAX_AVATAR_BYTES, MAX_NAME, data_url, image_mime};
use crate::{CoreError, Result, kv};

const K_NAME: &str = "system.name";
const K_AVATAR: &str = "system.avatar";
const K_KIND: &str = "system.kind";

/// Une seule personne (singlet) ou un système de plusieurs alters.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SystemKind {
    System,
    Singlet,
}

impl SystemKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Singlet => "singlet",
        }
    }
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemProfile {
    pub name: Option<String>,
    /// Adresse `data:` prête pour `<img src>`.
    pub avatar: Option<String>,
    /// Absent tant que rien n'a été choisi (bases d'avant ce réglage : système).
    pub kind: Option<SystemKind>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInput {
    pub name: Option<String>,
    #[serde(default)]
    pub avatar: AvatarEdit,
}

pub fn get(conn: &Connection) -> Result<SystemProfile> {
    Ok(SystemProfile {
        name: kv::get(conn, K_NAME)?,
        avatar: kv::get(conn, K_AVATAR)?
            .and_then(|b| STANDARD.decode(b).ok())
            .and_then(|b| data_url(&b)),
        kind: match kv::get(conn, K_KIND)?.as_deref() {
            Some("singlet") => Some(SystemKind::Singlet),
            Some("system") => Some(SystemKind::System),
            _ => None,
        },
    })
}

pub fn set_kind(conn: &Connection, kind: SystemKind) -> Result<SystemProfile> {
    kv::set(conn, K_KIND, kind.as_str())?;
    get(conn)
}

fn clean_name(name: Option<&str>) -> Result<Option<String>> {
    let name = name.map(str::trim).filter(|n| !n.is_empty());
    if name.is_some_and(|n| n.chars().count() > MAX_NAME) {
        return Err(CoreError::Invalid("name"));
    }
    Ok(name.map(str::to_owned))
}

/// Valide une image en base64 ; renvoie le base64 normalisé.
fn clean_avatar(data: &str) -> Result<String> {
    let bytes = STANDARD
        .decode(data.as_bytes())
        .map_err(|_| CoreError::Invalid("avatar"))?;
    if bytes.len() > MAX_AVATAR_BYTES || image_mime(&bytes).is_none() {
        return Err(CoreError::Invalid("avatar"));
    }
    Ok(STANDARD.encode(bytes))
}

pub fn set(conn: &mut Connection, input: &SystemInput) -> Result<SystemProfile> {
    let name = clean_name(input.name.as_deref())?;
    let avatar = match &input.avatar {
        AvatarEdit::Keep => None,
        AvatarEdit::Remove => Some(None),
        AvatarEdit::Set { data } => Some(Some(clean_avatar(data)?)),
    };
    let tx = conn.transaction()?;
    match name {
        Some(n) => kv::set(&tx, K_NAME, &n)?,
        None => kv::delete(&tx, K_NAME)?,
    }
    match avatar {
        Some(Some(a)) => kv::set(&tx, K_AVATAR, &a)?,
        Some(None) => kv::delete(&tx, K_AVATAR)?,
        None => {}
    }
    let out = get(&tx)?;
    tx.commit()?;
    Ok(out)
}

/// Nom venu de PluralKit : seulement si aucun nom n'est encore choisi.
pub fn adopt_name(conn: &Connection, name: &str) -> Result<bool> {
    if kv::get(conn, K_NAME)?.is_some() {
        return Ok(false);
    }
    match clean_name(Some(name)) {
        Ok(Some(n)) => {
            kv::set(conn, K_NAME, &n)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub fn has_avatar(conn: &Connection) -> Result<bool> {
    Ok(kv::get(conn, K_AVATAR)?.is_some())
}

/// Photo venue de PluralKit (octets déjà mis au carré) : seulement s'il n'y en a pas.
pub fn adopt_avatar(conn: &Connection, bytes: &[u8]) -> Result<bool> {
    if has_avatar(conn)? || bytes.len() > MAX_AVATAR_BYTES || image_mime(bytes).is_none() {
        return Ok(false);
    }
    kv::set(conn, K_AVATAR, &STANDARD.encode(bytes))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_is_stored() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::migrations::run(&mut c).unwrap();
        assert_eq!(get(&c).unwrap().kind, None);
        assert_eq!(
            set_kind(&c, SystemKind::Singlet).unwrap().kind,
            Some(SystemKind::Singlet)
        );
        assert_eq!(
            set_kind(&c, SystemKind::System).unwrap().kind,
            Some(SystemKind::System)
        );
        let dirty: i64 = c
            .query_row(
                "SELECT count(*) FROM sync_state WHERE tbl = 'kv' AND pk = '[\"system.kind\"]'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dirty, 1);
    }

    #[test]
    fn local_choice_wins_over_pluralkit() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::migrations::run(&mut c).unwrap();
        assert_eq!(get(&c).unwrap(), SystemProfile::default());
        assert!(adopt_name(&c, " Les Étoiles ").unwrap());
        assert_eq!(get(&c).unwrap().name.as_deref(), Some("Les Étoiles"));
        set(
            &mut c,
            &SystemInput {
                name: Some("Constellation".into()),
                avatar: AvatarEdit::Keep,
            },
        )
        .unwrap();
        assert!(!adopt_name(&c, "Autre").unwrap());
        assert_eq!(get(&c).unwrap().name.as_deref(), Some("Constellation"));
        let png = [0x89, b'P', b'N', b'G', 0, 0];
        assert!(adopt_avatar(&c, &png).unwrap());
        assert!(
            get(&c)
                .unwrap()
                .avatar
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
    }
}
