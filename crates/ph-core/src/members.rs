use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CoreError, Result};

/// Mêmes limites que PluralKit, pour que la synchro ne tronque jamais rien.
pub const MAX_NAME: usize = 100;
pub const MAX_PRONOUNS: usize = 100;
/// Profil local (non synchronisé).
pub const MAX_DESCRIPTION: usize = 2000;
pub const MAX_ROLES: usize = 12;
pub const MAX_ROLE: usize = 40;
pub const MAX_AVATAR_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Member {
    pub id: Uuid,
    pub name: String,
    pub pronouns: Option<String>,
    pub color: String,
    pub description: Option<String>,
    pub roles: Vec<String>,
    /// Image en adresse `data:` (PNG, JPEG ou WebP), prête pour `<img src>`.
    pub avatar: Option<String>,
    /// Millisecondes depuis l'époque Unix (sûr en `number` JS jusqu'en l'an 287 396).
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NewMember {
    pub name: String,
    pub pronouns: Option<String>,
    pub color: String,
}

impl NewMember {
    pub(crate) fn validated(self) -> Result<Self> {
        let name = self.name.trim().to_owned();
        if !(1..=MAX_NAME).contains(&name.chars().count()) {
            return Err(CoreError::Invalid("name"));
        }
        let pronouns = self
            .pronouns
            .map(|p| p.trim().to_owned())
            .filter(|p| !p.is_empty());
        if pronouns
            .as_ref()
            .is_some_and(|p| p.chars().count() > MAX_PRONOUNS)
        {
            return Err(CoreError::Invalid("pronouns"));
        }
        let color = self.color.to_ascii_lowercase();
        let ok = color.len() == 7
            && color.starts_with('#')
            && color[1..].bytes().all(|b| b.is_ascii_hexdigit());
        if !ok {
            return Err(CoreError::Invalid("color"));
        }
        Ok(Self {
            name,
            pronouns,
            color,
        })
    }
}

const SELECT: &str = "SELECT m.id, m.name, m.pronouns, m.color, m.created_at,
        p.description, p.roles, p.avatar
    FROM member m LEFT JOIN member_profile p ON p.member_id = m.id";

fn from_row(r: &Row<'_>) -> rusqlite::Result<Member> {
    let id: [u8; 16] = r.get(0)?;
    let roles: Option<String> = r.get(6)?;
    let avatar: Option<Vec<u8>> = r.get(7)?;
    Ok(Member {
        id: Uuid::from_bytes(id),
        name: r.get(1)?,
        pronouns: r.get(2)?,
        color: r.get(3)?,
        created_at: r.get(4)?,
        description: r.get(5)?,
        roles: roles
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default(),
        avatar: avatar.as_deref().and_then(data_url),
    })
}

fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some("image/png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("image/jpeg"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("image/webp"),
        _ => None,
    }
}

fn data_url(bytes: &[u8]) -> Option<String> {
    image_mime(bytes).map(|mime| format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

pub fn list(conn: &Connection) -> Result<Vec<Member>> {
    let mut st = conn.prepare_cached(&format!(
        "{SELECT} WHERE m.archived_at IS NULL ORDER BY m.name COLLATE NOCASE"
    ))?;
    let rows = st.query_map([], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get(conn: &Connection, id: Uuid) -> Result<Member> {
    conn.query_row(
        &format!("{SELECT} WHERE m.id = ?1"),
        [id.as_bytes()],
        from_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound,
        e => e.into(),
    })
}

/// Remplace le nom, les pronoms et la couleur d'un membre actif.
pub fn update(conn: &Connection, id: Uuid, input: NewMember) -> Result<Member> {
    let m = input.validated()?;
    let n = conn.execute(
        "UPDATE member SET name = ?2, pronouns = ?3, color = ?4 WHERE id = ?1 AND archived_at IS NULL",
        params![id.as_bytes(), m.name, m.pronouns, m.color],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    get(conn, id)
}

/// Archive un membre : il disparaît des listes mais son historique reste intact.
pub fn archive(conn: &mut Connection, id: Uuid) -> Result<()> {
    let now = crate::time::now_ms()?;
    let tx = conn.transaction()?;
    let n = tx.execute(
        "UPDATE member SET archived_at = ?2 WHERE id = ?1 AND archived_at IS NULL",
        params![id.as_bytes(), now],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    // Un membre archivé ne peut pas rester au front.
    tx.execute(
        "UPDATE front_session SET ended_at = max(?2, started_at) WHERE member_id = ?1 AND ended_at IS NULL",
        params![id.as_bytes(), now],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn create(conn: &Connection, input: NewMember) -> Result<Member> {
    let m = input.validated()?;
    let now = crate::time::now_ms()?;
    // UUIDv7 : triable dans le temps, sans coordination, prêt pour une synchro future.
    let id = Uuid::now_v7();
    conn.execute(
        "INSERT INTO member (id, name, pronouns, color, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id.as_bytes(), m.name, m.pronouns, m.color, now],
    )?;
    Ok(Member {
        id,
        name: m.name,
        pronouns: m.pronouns,
        color: m.color,
        description: None,
        roles: Vec::new(),
        avatar: None,
        created_at: now,
    })
}

/// Profil détaillé d'un membre. Local à l'appareil : la synchro PluralKit ne le lit jamais.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Profile {
    pub description: Option<String>,
    pub roles: Vec<String>,
    pub avatar: AvatarEdit,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum AvatarEdit {
    /// Ne pas toucher à l'image actuelle.
    #[default]
    Keep,
    Remove,
    /// Image encodée en base64 (PNG, JPEG ou WebP, 256 Kio au plus).
    Set {
        data: String,
    },
}

/// Image après validation : `None` = garder l'actuelle, `Some(None)` = l'effacer.
type AvatarChange = Option<Option<Vec<u8>>>;

impl Profile {
    fn validated(self) -> Result<(Option<String>, Vec<String>, AvatarChange)> {
        let description = self
            .description
            .map(|d| d.trim().to_owned())
            .filter(|d| !d.is_empty());
        if description
            .as_ref()
            .is_some_and(|d| d.chars().count() > MAX_DESCRIPTION)
        {
            return Err(CoreError::Invalid("description"));
        }
        let mut roles: Vec<String> = Vec::new();
        for r in self.roles {
            let r = r.trim().to_owned();
            if r.is_empty() || roles.iter().any(|x| x.to_lowercase() == r.to_lowercase()) {
                continue;
            }
            if r.chars().count() > MAX_ROLE {
                return Err(CoreError::Invalid("roles"));
            }
            roles.push(r);
        }
        if roles.len() > MAX_ROLES {
            return Err(CoreError::Invalid("roles"));
        }
        let avatar = match self.avatar {
            AvatarEdit::Keep => None,
            AvatarEdit::Remove => Some(None),
            AvatarEdit::Set { data } => {
                let bytes = STANDARD
                    .decode(data.as_bytes())
                    .map_err(|_| CoreError::Invalid("avatar"))?;
                if bytes.len() > MAX_AVATAR_BYTES || image_mime(&bytes).is_none() {
                    return Err(CoreError::Invalid("avatar"));
                }
                Some(Some(bytes))
            }
        };
        Ok((description, roles, avatar))
    }
}

/// Enregistre le profil d'un membre actif et renvoie le membre à jour.
pub fn set_profile(conn: &Connection, id: Uuid, profile: Profile) -> Result<Member> {
    let (description, roles, avatar) = profile.validated()?;
    ensure_active(conn, &[id])?;
    let roles = serde_json::to_string(&roles).map_err(|_| CoreError::Invalid("roles"))?;
    let current: Option<Vec<u8>> = conn
        .query_row(
            "SELECT avatar FROM member_profile WHERE member_id = ?1",
            [id.as_bytes()],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    let avatar = avatar.unwrap_or(current);
    conn.execute(
        "INSERT INTO member_profile (member_id, description, roles, avatar) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (member_id) DO UPDATE SET
            description = excluded.description, roles = excluded.roles, avatar = excluded.avatar",
        params![id.as_bytes(), description, roles, avatar],
    )?;
    get(conn, id)
}

/// Vérifie que tous les identifiants désignent des membres actifs.
pub(crate) fn ensure_active(conn: &Connection, ids: &[Uuid]) -> Result<()> {
    let mut st =
        conn.prepare_cached("SELECT 1 FROM member WHERE id = ?1 AND archived_at IS NULL")?;
    for id in ids {
        if !st.exists([id.as_bytes()])? {
            return Err(CoreError::NotFound);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vault;
    use crate::keyfile::KdfParams;

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];

    fn setup() -> (tempfile::TempDir, Vault, Uuid) {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::new(tmp.path()).with_kdf(KdfParams::TEST);
        v.create("phrase secrete longue").unwrap();
        let id = create(
            v.conn().unwrap(),
            NewMember {
                name: "Mila".into(),
                pronouns: None,
                color: "#4f8583".into(),
            },
        )
        .unwrap()
        .id;
        (tmp, v, id)
    }

    #[test]
    fn profile_round_trip_keeps_avatar_unless_told() {
        let (_t, v, id) = setup();
        let c = v.conn().unwrap();
        let m = set_profile(
            c,
            id,
            Profile {
                description: Some("  Protège les petits.  ".into()),
                roles: vec![
                    "Protectrice".into(),
                    "protectrice".into(),
                    " ".into(),
                    "Hôte".into(),
                ],
                avatar: AvatarEdit::Set {
                    data: STANDARD.encode(PNG),
                },
            },
        )
        .unwrap();
        assert_eq!(m.description.as_deref(), Some("Protège les petits."));
        assert_eq!(m.roles, vec!["Protectrice", "Hôte"]);
        assert!(
            m.avatar
                .as_deref()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );

        // Keep : l'image reste ; l'identité (nom…) n'est pas touchée.
        let m = set_profile(
            c,
            id,
            Profile {
                avatar: AvatarEdit::Keep,
                ..Profile::default()
            },
        )
        .unwrap();
        assert!(m.avatar.is_some());
        assert!(m.roles.is_empty());
        assert_eq!(m.name, "Mila");
        assert_eq!(list(c).unwrap()[0].avatar, m.avatar);

        let m = set_profile(
            c,
            id,
            Profile {
                avatar: AvatarEdit::Remove,
                ..Profile::default()
            },
        )
        .unwrap();
        assert!(m.avatar.is_none());
    }

    #[test]
    fn profile_rejects_bad_input() {
        let (_t, v, id) = setup();
        let c = v.conn().unwrap();
        let bad = |p: Profile| set_profile(c, id, p).unwrap_err();
        let svg = STANDARD.encode(b"<svg onload=alert(1)>");
        assert!(matches!(
            bad(Profile {
                avatar: AvatarEdit::Set { data: svg },
                ..Profile::default()
            }),
            CoreError::Invalid("avatar")
        ));
        let mut big = PNG.to_vec();
        big.resize(MAX_AVATAR_BYTES + 1, 0);
        assert!(matches!(
            bad(Profile {
                avatar: AvatarEdit::Set {
                    data: STANDARD.encode(big)
                },
                ..Profile::default()
            }),
            CoreError::Invalid("avatar")
        ));
        assert!(matches!(
            bad(Profile {
                roles: vec!["x".repeat(MAX_ROLE + 1)],
                ..Profile::default()
            }),
            CoreError::Invalid("roles")
        ));
        assert!(matches!(
            bad(Profile {
                description: Some("x".repeat(MAX_DESCRIPTION + 1)),
                ..Profile::default()
            }),
            CoreError::Invalid("description")
        ));
    }

    #[test]
    fn archived_member_has_no_profile_edits() {
        let (_t, mut v, id) = setup();
        archive(v.conn_mut().unwrap(), id).unwrap();
        let err = set_profile(v.conn().unwrap(), id, Profile::default()).unwrap_err();
        assert!(matches!(err, CoreError::NotFound));
    }
}
