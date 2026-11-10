use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CoreError, Result};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Member {
    pub id: Uuid,
    pub name: String,
    pub pronouns: Option<String>,
    pub color: String,
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
        if !(1..=64).contains(&name.chars().count()) {
            return Err(CoreError::Invalid("name"));
        }
        let pronouns = self
            .pronouns
            .map(|p| p.trim().to_owned())
            .filter(|p| !p.is_empty());
        if pronouns.as_ref().is_some_and(|p| p.chars().count() > 32) {
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

fn from_row(r: &Row<'_>) -> rusqlite::Result<Member> {
    let id: [u8; 16] = r.get(0)?;
    Ok(Member {
        id: Uuid::from_bytes(id),
        name: r.get(1)?,
        pronouns: r.get(2)?,
        color: r.get(3)?,
        created_at: r.get(4)?,
    })
}

pub fn list(conn: &Connection) -> Result<Vec<Member>> {
    let mut st = conn.prepare_cached(
        "SELECT id, name, pronouns, color, created_at FROM member
         WHERE archived_at IS NULL ORDER BY name COLLATE NOCASE",
    )?;
    let rows = st.query_map([], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get(conn: &Connection, id: Uuid) -> Result<Member> {
    conn.query_row(
        "SELECT id, name, pronouns, color, created_at FROM member WHERE id = ?1",
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
        created_at: now,
    })
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
