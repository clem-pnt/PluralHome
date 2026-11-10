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

/// Provenance d'un changement de front.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Saisi dans l'application : à propager aux services liés.
    Local,
    /// Reçu d'un service lié (PluralKit) : ne pas le renvoyer, sinon boucle.
    Remote,
}

/// Sujet de la file d'envoi pour les switchs à pousser vers PluralKit.
pub const PK_SWITCH_TOPIC: &str = "pluralkit.switch";

/// Contenu d'un switch en attente d'envoi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingSwitch {
    pub members: Vec<Uuid>,
    pub blurry: bool,
    pub at: i64,
}

/// Applique un changement de front maintenant, depuis l'application.
pub fn change(conn: &mut Connection, change: &FrontChange) -> Result<Vec<FrontSession>> {
    change_at(conn, change, crate::time::now_ms()?, Origin::Local)
}

/// Applique un changement de front de façon atomique à l'instant `at` : ferme les
/// intervalles des membres qui partent, ouvre ceux des membres qui arrivent, et
/// laisse intacts ceux qui restent (un co-front qui s'élargit ne coupe personne).
///
/// `at` ne peut pas précéder le début d'un intervalle encore ouvert : l'historique
/// resterait incohérent. Un changement qui ne change rien n'est pas enregistré.
pub fn change_at(
    conn: &mut Connection,
    change: &FrontChange,
    at: i64,
    origin: Origin,
) -> Result<Vec<FrontSession>> {
    let wanted: HashSet<Uuid> = change.members.iter().copied().collect();
    if wanted.len() != change.members.len() {
        return Err(CoreError::Invalid("members"));
    }
    let tx = conn.transaction()?;
    members::ensure_active(&tx, &change.members)?;

    let open = current(&tx)?;
    if open.iter().any(|s| s.started_at > at) {
        return Err(CoreError::Invalid("timestamp"));
    }
    let before: HashSet<Option<Uuid>> = open.iter().map(|s| s.member_id).collect();
    let after: HashSet<Option<Uuid>> = change
        .members
        .iter()
        .map(|m| Some(*m))
        .chain(change.blurry.then_some(None))
        .collect();
    if before == after {
        return Ok(open);
    }

    {
        let mut close =
            tx.prepare_cached("UPDATE front_session SET ended_at = ?2 WHERE id = ?1")?;
        for s in open.iter().filter(|s| !after.contains(&s.member_id)) {
            close.execute(params![s.id.as_bytes(), at])?;
        }
        let mut insert = tx.prepare_cached(
            "INSERT INTO front_session (id, member_id, started_at) VALUES (?1, ?2, ?3)",
        )?;
        // Ordre stable : celui de la saisie, le flou en dernier.
        let arriving = change
            .members
            .iter()
            .map(|m| Some(*m))
            .chain(change.blurry.then_some(None))
            .filter(|m| !before.contains(m));
        for member in arriving {
            insert.execute(params![
                Uuid::now_v7().as_bytes(),
                member.map(|m| *m.as_bytes()),
                at
            ])?;
        }
    }

    if origin == Origin::Local && crate::kv::flag(&tx, "pluralkit.enabled")? {
        let pending = PendingSwitch {
            members: change.members.clone(),
            blurry: change.blurry,
            at,
        };
        let payload = serde_json::to_string(&pending).map_err(|_| CoreError::Invalid("payload"))?;
        crate::outbox::push(&tx, PK_SWITCH_TOPIC, &payload, at)?;
    }

    let result = current(&tx)?;
    tx.commit()?;
    Ok(result)
}

/// Instant du dernier changement de front enregistré (début ou fin d'intervalle).
pub fn last_change(conn: &Connection) -> Result<Option<i64>> {
    Ok(conn.query_row(
        "SELECT max(max(started_at), ifnull(max(ended_at), 0)) FROM front_session",
        [],
        |r| r.get(0),
    )?)
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

#[cfg(test)]
mod timed_tests {
    use super::*;
    use crate::keyfile::KdfParams;
    use crate::members::NewMember;
    use crate::vault::Vault;

    #[test]
    fn remote_changes_are_dated_and_not_echoed() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = Vault::new(tmp.path()).with_kdf(KdfParams::TEST);
        v.create("phrase secrete longue").unwrap();
        let a = members::create(
            v.conn().unwrap(),
            NewMember {
                name: "Mila".into(),
                pronouns: None,
                color: "#4f8583".into(),
            },
        )
        .unwrap()
        .id;
        let c = v.conn_mut().unwrap();
        crate::kv::set(c, "pluralkit.enabled", "1").unwrap();

        let one = FrontChange {
            members: vec![a],
            blurry: false,
        };
        change_at(c, &one, 1_000, Origin::Remote).unwrap();
        assert_eq!(
            crate::outbox::count(c, PK_SWITCH_TOPIC).unwrap(),
            0,
            "pas d'écho"
        );
        assert_eq!(current(c).unwrap()[0].started_at, 1_000);

        // Antérieur à un intervalle ouvert : refusé.
        let none = FrontChange {
            members: vec![],
            blurry: false,
        };
        assert!(matches!(
            change_at(c, &none, 500, Origin::Remote),
            Err(CoreError::Invalid("timestamp"))
        ));

        change_at(c, &none, 2_000, Origin::Local).unwrap();
        let pending = crate::outbox::pending(c, PK_SWITCH_TOPIC, 10).unwrap();
        assert_eq!(pending.len(), 1);
        let p: PendingSwitch = serde_json::from_str(&pending[0].payload).unwrap();
        assert_eq!(
            p,
            PendingSwitch {
                members: vec![],
                blurry: false,
                at: 2_000
            }
        );
        assert_eq!(last_change(c).unwrap(), Some(2_000));
    }
}
