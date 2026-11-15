//! Présences au front.
//!
//! Modèle en intervalles : une ligne par membre et par passage au front, ouverte
//! tant que `ended_at` est NULL. Le co-front est simplement plusieurs intervalles
//! ouverts en même temps ; le front « flou » est un intervalle sans membre.

use std::collections::{HashMap, HashSet};

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
    pub status: FrontStatus,
}

/// Place d'un membre présent : au front, en co-front ou co-conscient.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrontStatus {
    #[default]
    Front,
    Cofront,
    Coconscious,
}

impl FrontStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Front => "front",
            Self::Cofront => "cofront",
            Self::Coconscious => "coconscious",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "cofront" => Self::Cofront,
            "coconscious" => Self::Coconscious,
            _ => Self::Front,
        }
    }
}

/// Nouvel état du front. Liste vide et `blurry = false` : plus personne au front.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontChange {
    pub members: Vec<Uuid>,
    pub blurry: bool,
    /// Statut par membre ; absent : « front » pour un arrivant, inchangé sinon
    /// (un switch reçu de PluralKit ne connaît pas les statuts).
    #[serde(default)]
    pub statuses: HashMap<Uuid, FrontStatus>,
}

fn from_row(r: &Row<'_>) -> rusqlite::Result<FrontSession> {
    let id: [u8; 16] = r.get(0)?;
    let member: Option<[u8; 16]> = r.get(1)?;
    Ok(FrontSession {
        id: Uuid::from_bytes(id),
        member_id: member.map(Uuid::from_bytes),
        started_at: r.get(2)?,
        ended_at: r.get(3)?,
        status: FrontStatus::parse(&r.get::<_, String>(4)?),
    })
}

const COLUMNS: &str = "id, member_id, started_at, ended_at, status";

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
///
/// Saisi à la main, le front doit garder quelqu'un « au front » : un membre au
/// statut front, ou le flou. Les switchs reçus de PluralKit ne sont pas filtrés.
pub fn change(conn: &mut Connection, change: &FrontChange) -> Result<Vec<FrontSession>> {
    let open = current(conn)?;
    let status_of = |m: &Uuid| {
        change.statuses.get(m).copied().unwrap_or_else(|| {
            open.iter()
                .find(|s| s.member_id == Some(*m))
                .map_or(FrontStatus::Front, |s| s.status)
        })
    };
    if !change.blurry
        && !change
            .members
            .iter()
            .any(|m| status_of(m) == FrontStatus::Front)
    {
        return Err(CoreError::Invalid("fronter"));
    }
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
    if change.statuses.keys().any(|m| !wanted.contains(m)) {
        return Err(CoreError::Invalid("statuses"));
    }
    let restatus: Vec<(Uuid, FrontStatus)> = open
        .iter()
        .filter_map(|s| {
            let m = s.member_id?;
            let want = *change.statuses.get(&m)?;
            (want != s.status && wanted.contains(&m)).then_some((s.id, want))
        })
        .collect();
    if before == after && restatus.is_empty() {
        return Ok(open);
    }

    {
        let mut close =
            tx.prepare_cached("UPDATE front_session SET ended_at = ?2 WHERE id = ?1")?;
        for s in open.iter().filter(|s| !after.contains(&s.member_id)) {
            close.execute(params![s.id.as_bytes(), at])?;
        }
        let mut restat = tx.prepare_cached("UPDATE front_session SET status = ?2 WHERE id = ?1")?;
        for (id, status) in &restatus {
            restat.execute(params![id.as_bytes(), status.as_str()])?;
        }
        let mut insert = tx.prepare_cached(
            "INSERT INTO front_session (id, member_id, started_at, status) VALUES (?1, ?2, ?3, ?4)",
        )?;
        // Ordre stable : celui de la saisie, le flou en dernier.
        let arriving = change
            .members
            .iter()
            .map(|m| Some(*m))
            .chain(change.blurry.then_some(None))
            .filter(|m| !before.contains(m));
        for member in arriving {
            let status = member
                .and_then(|m| change.statuses.get(&m).copied())
                .unwrap_or_default();
            insert.execute(params![
                Uuid::now_v7().as_bytes(),
                member.map(|m| *m.as_bytes()),
                at,
                status.as_str()
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

/// Favoris du switch rapide, partagés entre appareils.
const FAVORITES: &str = "front.favorites";
pub const MAX_FAVORITES: usize = 30;

/// Favoris choisis, sans les membres archivés depuis. Vide : jamais choisis.
pub fn favorites(conn: &Connection) -> Result<Vec<Uuid>> {
    let Some(raw) = crate::kv::get(conn, FAVORITES)? else {
        return Ok(Vec::new());
    };
    let ids: Vec<Uuid> = serde_json::from_str(&raw).unwrap_or_default();
    let mut st =
        conn.prepare_cached("SELECT 1 FROM member WHERE id = ?1 AND archived_at IS NULL")?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if st.exists([id.as_bytes()])? {
            out.push(id);
        }
    }
    Ok(out)
}

pub fn set_favorites(conn: &Connection, ids: &[Uuid]) -> Result<Vec<Uuid>> {
    let mut seen = HashSet::new();
    let ids: Vec<Uuid> = ids.iter().copied().filter(|id| seen.insert(*id)).collect();
    if ids.len() > MAX_FAVORITES {
        return Err(CoreError::Invalid("favorites"));
    }
    members::ensure_active(conn, &ids)?;
    if ids.is_empty() {
        crate::kv::delete(conn, FAVORITES)?;
    } else {
        let raw = serde_json::to_string(&ids).map_err(|_| CoreError::Invalid("favorites"))?;
        crate::kv::set(conn, FAVORITES, &raw)?;
    }
    Ok(ids)
}

/// Membres du switch rapide : les favoris choisis, sinon les huit plus fréquents
/// des 90 derniers jours. Le booléen dit s'il s'agit de favoris choisis.
pub fn quick(conn: &Connection) -> Result<(Vec<Uuid>, bool)> {
    let fav = favorites(conn)?;
    if !fav.is_empty() {
        return Ok((fav, true));
    }
    let since = crate::time::now_ms()? - 90 * 24 * 60 * 60 * 1000;
    Ok((frequent(conn, since, 8)?, false))
}

/// Membres actifs les plus souvent passés au front depuis `since`, du plus
/// fréquent au moins fréquent.
pub fn frequent(conn: &Connection, since: i64, limit: usize) -> Result<Vec<Uuid>> {
    let mut st = conn.prepare_cached(
        "SELECT f.member_id FROM front_session f
         JOIN member m ON m.id = f.member_id AND m.archived_at IS NULL
         WHERE f.started_at >= ?1 OR f.ended_at IS NULL OR f.ended_at >= ?1
         GROUP BY f.member_id
         ORDER BY count(*) DESC, max(f.started_at) DESC
         LIMIT ?2",
    )?;
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    Ok(st
        .query_map(params![since, limit], |r| {
            Ok(Uuid::from_bytes(r.get::<_, [u8; 16]>(0)?))
        })?
        .collect::<rusqlite::Result<_>>()?)
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
        v.create().unwrap();
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
    fn favorites_and_frequent() {
        let (_t, mut v, a, b) = setup();
        let c = v.conn_mut().unwrap();
        assert!(favorites(c).unwrap().is_empty());
        assert_eq!(set_favorites(c, &[b, a, b]).unwrap(), vec![b, a]);
        assert_eq!(favorites(c).unwrap(), vec![b, a]);
        assert!(set_favorites(c, &[Uuid::now_v7()]).is_err());
        let sw = |c: &mut Connection, m: Uuid, at| {
            change_at(
                c,
                &FrontChange {
                    members: vec![m],
                    blurry: false,
                    statuses: HashMap::new(),
                },
                at,
                Origin::Local,
            )
            .unwrap();
        };
        sw(c, b, 1_000);
        sw(c, a, 2_000);
        sw(c, b, 3_000);
        assert_eq!(frequent(c, 0, 10).unwrap(), vec![b, a]);
        assert_eq!(frequent(c, 0, 1).unwrap(), vec![b]);
        members::archive(c, b).unwrap();
        assert_eq!(favorites(c).unwrap(), vec![a]);
    }

    #[test]
    fn statuses_change_without_cutting() {
        let (_t, mut v, a, b) = setup();
        let c = v.conn_mut().unwrap();
        let st = |pairs: &[(Uuid, FrontStatus)]| pairs.iter().copied().collect();
        let both = vec![a, b];
        let cur = change_at(
            c,
            &FrontChange {
                members: both.clone(),
                blurry: false,
                statuses: st(&[(b, FrontStatus::Coconscious)]),
            },
            1_000,
            Origin::Local,
        )
        .unwrap();
        let of =
            |s: &[FrontSession], m: Uuid| s.iter().find(|x| x.member_id == Some(m)).unwrap().status;
        assert_eq!(of(&cur, a), FrontStatus::Front);
        assert_eq!(of(&cur, b), FrontStatus::Coconscious);

        // Changer le statut seul ne coupe pas l'intervalle.
        let cur = change_at(
            c,
            &FrontChange {
                members: both.clone(),
                blurry: false,
                statuses: st(&[(b, FrontStatus::Cofront)]),
            },
            2_000,
            Origin::Local,
        )
        .unwrap();
        assert_eq!(of(&cur, b), FrontStatus::Cofront);
        assert!(cur.iter().all(|s| s.started_at == 1_000));

        // Un switch sans statuts (PluralKit) garde ceux en place.
        let cur = change_at(
            c,
            &FrontChange {
                members: both,
                blurry: false,
                statuses: HashMap::new(),
            },
            3_000,
            Origin::Remote,
        )
        .unwrap();
        assert_eq!(of(&cur, b), FrontStatus::Cofront);

        // Statut pour un membre absent du changement : refusé.
        let bad = FrontChange {
            members: vec![a],
            blurry: false,
            statuses: st(&[(b, FrontStatus::Front)]),
        };
        assert!(change_at(c, &bad, 4_000, Origin::Local).is_err());
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
                statuses: std::collections::HashMap::new(),
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
                statuses: std::collections::HashMap::new(),
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
                statuses: std::collections::HashMap::new(),
            },
        )
        .unwrap();
        assert_eq!(ids(&cur), HashSet::from([None]));

        // Personne : refusé à la main, possible depuis PluralKit.
        let nobody = FrontChange {
            members: vec![],
            blurry: false,
            statuses: std::collections::HashMap::new(),
        };
        assert!(change(conn, &nobody).is_err());
        let now = crate::time::now_ms().unwrap();
        assert!(
            change_at(conn, &nobody, now, Origin::Remote)
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
            statuses: std::collections::HashMap::new(),
        };
        assert!(matches!(change(conn, &unknown), Err(CoreError::NotFound)));
        let dup = FrontChange {
            members: vec![a, a],
            blurry: false,
            statuses: std::collections::HashMap::new(),
        };
        assert!(matches!(
            change(conn, &dup),
            Err(CoreError::Invalid("members"))
        ));
        assert!(current(conn).unwrap().is_empty());
    }

    #[test]
    fn manual_change_needs_someone_at_front() {
        let (_t, mut v, a, b) = setup();
        let c = v.conn_mut().unwrap();
        let mk = |members: Vec<Uuid>, blurry: bool, st: &[(Uuid, FrontStatus)]| FrontChange {
            members,
            blurry,
            statuses: st.iter().copied().collect(),
        };
        // Personne, ou seulement du co-front : refusé.
        let nobody = mk(vec![], false, &[]);
        assert!(matches!(
            change(c, &nobody),
            Err(CoreError::Invalid("fronter"))
        ));
        let co = mk(vec![a], false, &[(a, FrontStatus::Cofront)]);
        assert!(matches!(change(c, &co), Err(CoreError::Invalid("fronter"))));
        assert!(current(c).unwrap().is_empty());
        // Un alter au front, ou le flou : accepté.
        change(c, &mk(vec![a, b], false, &[(b, FrontStatus::Coconscious)])).unwrap();
        change(c, &mk(vec![], true, &[])).unwrap();
        // Statut absent : celui déjà ouvert compte (Sacha reste co-conscient).
        change(c, &mk(vec![b], false, &[(b, FrontStatus::Coconscious)])).unwrap_err();
        change(c, &mk(vec![a, b], false, &[(b, FrontStatus::Coconscious)])).unwrap();
        assert!(matches!(
            change(c, &mk(vec![b], false, &[])),
            Err(CoreError::Invalid("fronter"))
        ));
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
        v.create().unwrap();
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
            statuses: std::collections::HashMap::new(),
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
            statuses: std::collections::HashMap::new(),
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
