//! Présences au front.
//!
//! Modèle en intervalles : une ligne par membre et par passage au front, ouverte
//! tant que `ended_at` est NULL. Le co-front est simplement plusieurs intervalles
//! ouverts en même temps ; le front « flou » est un intervalle sans membre.

use std::collections::HashSet;

use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CoreError, Result, members};

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontSession {
    pub id: Uuid,
    /// `None` : front flou.
    pub member_id: Option<Uuid>,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub started_at: i64,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub ended_at: Option<i64>,
}

/// Nouvel état du front. Liste vide et `blurry = false` : plus personne au front.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontChange {
    pub members: Vec<Uuid>,
    pub blurry: bool,
}

fn from_row(r: &Row<'_>) -> rusqlite::Result<FrontSession> {
    let id: [u8; 16] = r.get(0)?;
    let member: Option<[u8; 16]> = r.get(1)?;
    Ok(FrontSession {
        id: Uuid::from_bytes(id),
        member_id: member.map(Uuid::from_bytes),
        started_at: r.get(2)?,
        ended_at: r.get(3)?,
    })
}

const COLUMNS: &str = "id, member_id, started_at, ended_at";

pub fn current(conn: &Connection) -> Result<Vec<FrontSession>> {
    let mut st = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM front_session WHERE ended_at IS NULL ORDER BY started_at, id"
    ))?;
    Ok(st
        .query_map([], from_row)?
        .collect::<rusqlite::Result<_>>()?)
}

/// Intervalles qui chevauchent `[from, to)`, y compris ceux encore ouverts.
pub fn history(conn: &Connection, from: i64, to: i64) -> Result<Vec<FrontSession>> {
    if to <= from {
        return Err(CoreError::Invalid("range"));
    }
    let mut st = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM front_session
         WHERE started_at < ?2 AND (ended_at IS NULL OR ended_at > ?1)
         ORDER BY started_at, id"
    ))?;
    Ok(st
        .query_map(params![from, to], from_row)?
        .collect::<rusqlite::Result<_>>()?)
}

/// Applique un changement de front de façon atomique : ferme les intervalles
/// des membres qui partent, ouvre ceux des membres qui arrivent, et laisse
/// intacts ceux qui restent (un co-front qui s'élargit ne coupe personne).
pub fn change(conn: &mut Connection, change: &FrontChange) -> Result<Vec<FrontSession>> {
    let wanted: HashSet<Uuid> = change.members.iter().copied().collect();
    if wanted.len() != change.members.len() {
        return Err(CoreError::Invalid("members"));
    }
    let now = crate::time::now_ms()?;
    let tx = conn.transaction()?;
    members::ensure_active(&tx, &change.members)?;

    let open = current(&tx)?;
    let mut still_open: HashSet<Option<Uuid>> = HashSet::new();
    {
        let mut close = tx.prepare_cached(
            "UPDATE front_session SET ended_at = max(?2, started_at) WHERE id = ?1",
        )?;
        for s in &open {
            let stays = match s.member_id {
                Some(m) => wanted.contains(&m),
                None => change.blurry,
            };
            if stays {
                still_open.insert(s.member_id);
            } else {
                close.execute(params![s.id.as_bytes(), now])?;
            }
        }
        let mut insert = tx.prepare_cached(
            "INSERT INTO front_session (id, member_id, started_at) VALUES (?1, ?2, ?3)",
        )?;
        let arriving = change
            .members
            .iter()
            .map(|m| Some(*m))
            .chain(change.blurry.then_some(None))
            .filter(|m| !still_open.contains(m));
        for member in arriving {
            insert.execute(params![
                Uuid::now_v7().as_bytes(),
                member.map(|m| *m.as_bytes()),
                now
            ])?;
        }
    }
    let result = current(&tx)?;
    tx.commit()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyfile::KdfParams;
    use crate::members::NewMember;
    use crate::vault::Vault;

    fn setup() -> (tempfile::TempDir, Vault, Uuid, Uuid) {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::new(tmp.path()).with_kdf(KdfParams::TEST);
        v.create("phrase secrete longue").unwrap();
        let mk = |v: &Vault, n: &str| {
            members::create(
                v.conn().unwrap(),
                NewMember {
                    name: n.into(),
                    pronouns: None,
                    color: "#4f8583".into(),
                },
            )
            .unwrap()
            .id
        };
        let a = mk(&v, "Mila");
        let b = mk(&v, "Sacha");
        (tmp, v, a, b)
    }

    fn ids(s: &[FrontSession]) -> HashSet<Option<Uuid>> {
        s.iter().map(|s| s.member_id).collect()
    }

    #[test]
    fn switch_cofront_and_blurry() {
        let (_t, mut v, a, b) = setup();
        let conn = v.conn_mut().unwrap();

        let cur = change(
            conn,
            &FrontChange {
                members: vec![a],
                blurry: false,
            },
        )
        .unwrap();
        assert_eq!(ids(&cur), HashSet::from([Some(a)]));
        let a_session = cur[0].id;

        // Co-front : Sacha arrive, la session de Mila n'est pas coupée.
        let cur = change(
            conn,
            &FrontChange {
                members: vec![a, b],
                blurry: false,
            },
        )
        .unwrap();
        assert_eq!(ids(&cur), HashSet::from([Some(a), Some(b)]));
        assert!(cur.iter().any(|s| s.id == a_session));

        // Flou seul : tout le monde sort.
        let cur = change(
            conn,
            &FrontChange {
                members: vec![],
                blurry: true,
            },
        )
        .unwrap();
        assert_eq!(ids(&cur), HashSet::from([None]));

        // Personne.
        assert!(
            change(
                conn,
                &FrontChange {
                    members: vec![],
                    blurry: false
                }
            )
            .unwrap()
            .is_empty()
        );

        let all = history(conn, 0, i64::MAX).unwrap();
        assert_eq!(all.len(), 3);
        assert!(all.iter().all(|s| s.ended_at.is_some()));
    }

    #[test]
    fn rejects_unknown_and_duplicate_members() {
        let (_t, mut v, a, _) = setup();
        let conn = v.conn_mut().unwrap();
        let unknown = FrontChange {
            members: vec![Uuid::now_v7()],
            blurry: false,
        };
        assert!(matches!(change(conn, &unknown), Err(CoreError::NotFound)));
        let dup = FrontChange {
            members: vec![a, a],
            blurry: false,
        };
        assert!(matches!(
            change(conn, &dup),
            Err(CoreError::Invalid("members"))
        ));
        assert!(current(conn).unwrap().is_empty());
    }

    #[test]
    fn history_rejects_empty_range() {
        let (_t, v, _, _) = setup();
        assert!(matches!(
            history(v.conn().unwrap(), 10, 10),
            Err(CoreError::Invalid("range"))
        ));
    }
}
