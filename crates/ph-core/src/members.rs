use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
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
    fn validated(self) -> Result<Self> {
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

pub fn create(conn: &Connection, input: NewMember) -> Result<Member> {
    let m = input.validated()?;
    let now = i64::try_from(OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
        .map_err(|_| CoreError::Invalid("clock"))?;
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
