//! Réplication ligne à ligne pour la synchronisation entre appareils.
//!
//! Chaque écriture locale d'une table suivie est notée dans `sync_state` par des
//! déclencheurs (migration 8). Un changement transporte la ligne entière (ou rien,
//! pour une suppression) avec sa version `(at, dev)` : le dernier écrivain gagne,
//! l'identifiant d'appareil départage les égalités. Les données ne sortent d'ici
//! qu'en clair ; le chiffrement est fait par l'appelant (`cloudkey`).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::{CoreError, Result, time};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Blob,
    Text,
    Int,
}

struct Spec {
    name: &'static str,
    pk: &'static [(&'static str, Kind)],
    /// Seules clés de `kv` partagées : le reste (jetons, réglages d'appareil) reste local.
    only_key: Option<&'static [&'static str]>,
}

use Kind::{Blob, Int, Text};

/// Clés de `kv` synchronisées (mêmes que dans les déclencheurs de la migration 13).
pub const SHARED_KEYS: &[&str] = &[
    "safety.plan",
    "newcomer.plan",
    "trusted.contact",
    "front.favorites",
    "system.name",
    "system.avatar",
    "system.kind",
    "pluralkit.ignored",
    "friends.identity",
    "friends.privacy",
    "friends.audience",
];

/// Ordre parents → enfants : les insertions le suivent, les suppressions l'inversent.
const TABLES: &[Spec] = &[
    Spec {
        name: "member",
        pk: &[("id", Blob)],
        only_key: None,
    },
    Spec {
        name: "role",
        pk: &[("name", Text)],
        only_key: None,
    },
    Spec {
        name: "folder",
        pk: &[("id", Blob)],
        only_key: None,
    },
    Spec {
        name: "journal_entry",
        pk: &[("id", Blob)],
        only_key: None,
    },
    Spec {
        name: "chat_group",
        pk: &[("id", Blob)],
        only_key: None,
    },
    Spec {
        name: "message",
        pk: &[("id", Blob)],
        only_key: None,
    },
    Spec {
        name: "friend_pref",
        pk: &[("id", Text)],
        only_key: None,
    },
    Spec {
        name: "poll",
        pk: &[("id", Blob)],
        only_key: None,
    },
    Spec {
        name: "member_profile",
        pk: &[("member_id", Blob)],
        only_key: None,
    },
    Spec {
        name: "front_session",
        pk: &[("id", Blob)],
        only_key: None,
    },
    Spec {
        name: "message_read",
        pk: &[("message_id", Blob), ("member_id", Blob)],
        only_key: None,
    },
    Spec {
        name: "poll_option",
        pk: &[("id", Blob)],
        only_key: None,
    },
    Spec {
        name: "catchup_ack",
        pk: &[("member_id", Blob)],
        only_key: None,
    },
    Spec {
        name: "journal_author",
        pk: &[("entry_id", Blob), ("member_id", Blob)],
        only_key: None,
    },
    Spec {
        name: "folder_member",
        pk: &[("folder_id", Blob), ("member_id", Blob)],
        only_key: None,
    },
    Spec {
        name: "message_author",
        pk: &[("message_id", Blob), ("member_id", Blob)],
        only_key: None,
    },
    Spec {
        name: "chat_group_member",
        pk: &[("group_id", Blob), ("member_id", Blob)],
        only_key: None,
    },
    Spec {
        name: "message_remote",
        pk: &[("message_id", Blob), ("role", Int), ("ref", Text)],
        only_key: None,
    },
    Spec {
        name: "group_remote",
        pk: &[("group_id", Blob), ("ref", Text)],
        only_key: None,
    },
    Spec {
        name: "group_peer",
        pk: &[("group_id", Blob), ("key", Text)],
        only_key: None,
    },
    Spec {
        name: "message_system",
        pk: &[("message_id", Blob)],
        only_key: None,
    },
    Spec {
        name: "message_recipient",
        pk: &[("message_id", Blob), ("member_id", Blob)],
        only_key: None,
    },
    Spec {
        name: "poll_vote",
        pk: &[("option_id", Blob), ("member_id", Blob)],
        only_key: None,
    },
    Spec {
        name: "pk_member",
        pk: &[("member_id", Blob)],
        only_key: None,
    },
    Spec {
        name: "pk_switch",
        pk: &[("pk_id", Text)],
        only_key: None,
    },
    Spec {
        name: "kv",
        pk: &[("key", Text)],
        only_key: Some(SHARED_KEYS),
    },
];

fn spec(name: &str) -> Option<(usize, &'static Spec)> {
    TABLES.iter().enumerate().find(|(_, s)| s.name == name)
}

/// Un changement à transmettre ou reçu.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Change {
    /// Table.
    pub t: String,
    /// Clé primaire, tableau JSON (hexadécimal pour les BLOB).
    pub p: String,
    /// Ligne complète, `None` pour une suppression.
    pub r: Option<Map<String, Json>>,
    /// Horodatage de la version (ms).
    pub a: i64,
    /// Appareil auteur de la version.
    pub d: String,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    pub applied: usize,
    pub kept_local: usize,
    pub skipped: usize,
}

fn to_json(v: Value) -> Json {
    match v {
        Value::Null => Json::Null,
        Value::Integer(i) => Json::from(i),
        Value::Real(f) => Json::from(f),
        Value::Text(s) => Json::String(s),
        Value::Blob(b) => {
            let mut m = Map::new();
            m.insert("b".into(), Json::String(B64.encode(b)));
            Json::Object(m)
        }
    }
}

fn from_json(v: &Json) -> Result<Value> {
    Ok(match v {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Integer(i64::from(*b)),
        Json::Number(n) => match n.as_i64() {
            Some(i) => Value::Integer(i),
            None => Value::Real(n.as_f64().ok_or(CoreError::Invalid("sync_value"))?),
        },
        Json::String(s) => Value::Text(s.clone()),
        Json::Object(m) => match m.get("b") {
            Some(Json::String(b)) => Value::Blob(
                B64.decode(b)
                    .map_err(|_| CoreError::Invalid("sync_value"))?,
            ),
            _ => return Err(CoreError::Invalid("sync_value")),
        },
        Json::Array(_) => return Err(CoreError::Invalid("sync_value")),
    })
}

/// Décode la clé primaire texte en valeurs SQL, en vérifiant sa forme.
fn pk_values(s: &Spec, pk: &str) -> Result<Vec<Value>> {
    let parts: Vec<String> = serde_json::from_str(pk).map_err(|_| CoreError::Invalid("sync_pk"))?;
    if parts.len() != s.pk.len() {
        return Err(CoreError::Invalid("sync_pk"));
    }
    let values = parts
        .into_iter()
        .zip(s.pk)
        .map(|(p, (_, kind))| match kind {
            Blob => hex::decode(&p)
                .map(Value::Blob)
                .map_err(|_| CoreError::Invalid("sync_pk")),
            Text => Ok(Value::Text(p)),
            Int => p
                .parse::<i64>()
                .map(Value::Integer)
                .map_err(|_| CoreError::Invalid("sync_pk")),
        })
        .collect::<Result<Vec<_>>>()?;
    if let Some(only) = s.only_key
        && !matches!(values.first(), Some(Value::Text(k)) if only.contains(&k.as_str()))
    {
        return Err(CoreError::Invalid("sync_pk"));
    }
    Ok(values)
}

fn pk_where(s: &Spec) -> String {
    s.pk.iter()
        .enumerate()
        .map(|(i, (c, _))| format!("{c} = ?{}", i + 1))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn pk_select(s: &Spec) -> String {
    let parts =
        s.pk.iter()
            .map(|(c, k)| match k {
                Blob => format!("hex({c})"),
                Text => (*c).to_string(),
                Int => format!("CAST({c} AS TEXT)"),
            })
            .collect::<Vec<_>>()
            .join(", ");
    format!("json_array({parts})")
}

fn columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut st = conn.prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))?;
    let cols = st
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(cols)
}

fn read_row(conn: &Connection, s: &Spec, pk: &str) -> Result<Option<Map<String, Json>>> {
    let values = pk_values(s, pk)?;
    let mut st = conn.prepare(&format!("SELECT * FROM {} WHERE {}", s.name, pk_where(s)))?;
    let names: Vec<String> = st.column_names().map(String::from).collect();
    let row = st
        .query_row(params_from_iter(values), |r| {
            let mut m = Map::new();
            for (i, n) in names.iter().enumerate() {
                m.insert(n.clone(), to_json(r.get::<_, Value>(i)?));
            }
            Ok(m)
        })
        .optional()?;
    Ok(row)
}

/// Ajoute au suivi toutes les lignes existantes qui n'y sont pas encore, avec une
/// version très ancienne : en cas de conflit, ce qui est déjà sur le serveur gagne.
/// Tout est ensuite marqué « à envoyer » (nouveau jeu de clés, nouvel appareil).
pub fn seed(conn: &Connection) -> Result<()> {
    for s in TABLES {
        let filter = s
            .only_key
            .map(|k| format!(" WHERE key IN ('{}')", k.join("', '")))
            .unwrap_or_default();
        conn.execute(
            &format!(
                "INSERT OR IGNORE INTO sync_state (tbl, pk, at, dev, dirty)
                 SELECT '{}', {}, 1, NULL, 1 FROM {}{filter}",
                s.name,
                pk_select(s),
                s.name
            ),
            [],
        )?;
    }
    conn.execute("UPDATE sync_state SET dirty = 1", [])?;
    Ok(())
}

/// Nombre de changements locaux pas encore envoyés.
pub fn pending_count(conn: &Connection) -> Result<i64> {
    Ok(
        conn.query_row("SELECT count(*) FROM sync_state WHERE dirty = 1", [], |r| {
            r.get(0)
        })?,
    )
}

/// Changements locaux à envoyer, au plus `limit`.
pub fn pending(conn: &Connection, me: &str, limit: usize) -> Result<Vec<Change>> {
    let mut st = conn.prepare(
        "SELECT tbl, pk, at, dev FROM sync_state WHERE dirty = 1 ORDER BY tbl, pk LIMIT ?1",
    )?;
    let rows = st
        .query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut out = Vec::with_capacity(rows.len());
    for (t, p, a, d) in rows {
        let Some((_, s)) = spec(&t) else { continue };
        let r = match read_row(conn, s, &p) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(table = %t, error = %e, "ligne illisible, ignorée");
                continue;
            }
        };
        out.push(Change {
            t,
            p,
            r,
            a,
            d: d.unwrap_or_else(|| me.to_owned()),
        });
    }
    Ok(out)
}

/// Marque comme envoyés les changements dont la version n'a pas bougé entre-temps.
pub fn mark_pushed(conn: &mut Connection, changes: &[Change], me: &str) -> Result<()> {
    let tx = conn.transaction()?;
    {
        let mut st = tx.prepare(
            "UPDATE sync_state SET dirty = 0, dev = ifnull(dev, ?4)
             WHERE tbl = ?1 AND pk = ?2 AND at = ?3",
        )?;
        for c in changes {
            st.execute(params![c.t, c.p, c.a, me])?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn is_unique_violation(e: &CoreError) -> bool {
    matches!(
        e,
        CoreError::Storage(rusqlite::Error::SqliteFailure(f, _))
            if f.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    )
}

/// Note une écriture faite pendant l'application (déclencheurs suspendus).
fn touch(conn: &Connection, table: &str, pk: &str) -> Result<()> {
    let now = time::now_ms()?;
    conn.execute(
        "INSERT INTO sync_state (tbl, pk, at, dev, dirty) VALUES (?1, ?2, ?3, NULL, 1)
         ON CONFLICT (tbl, pk) DO UPDATE SET at = max(excluded.at, sync_state.at + 1),
         dev = NULL, dirty = 1",
        params![table, pk, now],
    )?;
    Ok(())
}

fn write(conn: &Connection, s: &Spec, pk: &[Value], row: Option<&Map<String, Json>>) -> Result<()> {
    let Some(row) = row else {
        conn.execute(
            &format!("DELETE FROM {} WHERE {}", s.name, pk_where(s)),
            params_from_iter(pk.iter()),
        )?;
        return Ok(());
    };
    let known = columns(conn, s.name)?;
    let mut cols = Vec::new();
    let mut vals = Vec::new();
    for ((c, _), v) in s.pk.iter().zip(pk) {
        cols.push((*c).to_string());
        vals.push(v.clone());
    }
    for (k, v) in row {
        if s.pk.iter().any(|(c, _)| c == k) || !known.contains(k) {
            continue;
        }
        cols.push(k.clone());
        vals.push(from_json(v)?);
    }
    let marks = (1..=cols.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let pkcols = s.pk.iter().map(|(c, _)| *c).collect::<Vec<_>>().join(", ");
    let update = cols[s.pk.len()..]
        .iter()
        .map(|c| format!("{c} = excluded.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let action = if update.is_empty() {
        "NOTHING".to_owned()
    } else {
        format!("UPDATE SET {update}")
    };
    conn.execute(
        &format!(
            "INSERT INTO {} ({}) VALUES ({marks}) ON CONFLICT ({pkcols}) DO {action}",
            s.name,
            cols.join(", ")
        ),
        params_from_iter(vals),
    )?;
    Ok(())
}

/// Lève un conflit d'unicité local pour laisser passer la version reçue. Renvoie
/// `false` si la table n'a pas de résolution connue.
fn resolve_unique(
    conn: &Connection,
    s: &Spec,
    pk: &[Value],
    row: &Map<String, Json>,
) -> Result<bool> {
    match s.name {
        // Deux présences ouvertes pour la même personne : on ferme la locale.
        "front_session" => {
            let member = row
                .get("member_id")
                .map(from_json)
                .transpose()?
                .unwrap_or(Value::Null);
            let started = row.get("started_at").and_then(Json::as_i64).unwrap_or(0);
            let mut st = conn.prepare(
                "SELECT json_array(hex(id)) FROM front_session
                 WHERE ended_at IS NULL AND ifnull(member_id, x'00') = ifnull(?1, x'00') AND id <> ?2",
            )?;
            let clash = st
                .query_map(params![member, pk[0]], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            conn.execute(
                "UPDATE front_session SET ended_at = max(started_at, ?3)
                 WHERE ended_at IS NULL AND ifnull(member_id, x'00') = ifnull(?1, x'00') AND id <> ?2",
                params![member, pk[0], started],
            )?;
            for p in clash {
                touch(conn, "front_session", &p)?;
            }
            Ok(true)
        }
        // Même membre PluralKit relié à deux membres : la correspondance reçue gagne.
        "pk_member" => {
            let Some(Json::String(uuid)) = row.get("pk_uuid") else {
                return Ok(false);
            };
            let mut st = conn.prepare(
                "SELECT json_array(hex(member_id)) FROM pk_member WHERE pk_uuid = ?1 AND member_id <> ?2",
            )?;
            let clash = st
                .query_map(params![uuid, pk[0]], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            conn.execute(
                "DELETE FROM pk_member WHERE pk_uuid = ?1 AND member_id <> ?2",
                params![uuid, pk[0]],
            )?;
            for p in clash {
                touch(conn, "pk_member", &p)?;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn apply_one(conn: &Connection, s: &Spec, c: &Change) -> Result<()> {
    let pk = pk_values(s, &c.p)?;
    match write(conn, s, &pk, c.r.as_ref()) {
        Err(e) if is_unique_violation(&e) => match c.r.as_ref() {
            Some(row) if resolve_unique(conn, s, &pk, row)? => write(conn, s, &pk, Some(row)),
            _ => Err(e),
        },
        r => r,
    }
}

/// Supprime ce que des suppressions croisées ont laissé orphelin (les tables sans
/// rowid échappent à `foreign_key_check`, d'où des requêtes explicites).
fn sweep_orphans(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "DELETE FROM front_session WHERE member_id IS NOT NULL AND member_id NOT IN (SELECT id FROM member);
         UPDATE message SET author_id = NULL WHERE author_id IS NOT NULL AND author_id NOT IN (SELECT id FROM member);
         DELETE FROM message WHERE recipient_id IS NOT NULL AND recipient_id NOT IN (SELECT id FROM member);
         DELETE FROM message_read WHERE member_id NOT IN (SELECT id FROM member) OR message_id NOT IN (SELECT id FROM message);
         DELETE FROM catchup_ack WHERE member_id NOT IN (SELECT id FROM member);
         DELETE FROM member_profile WHERE member_id NOT IN (SELECT id FROM member);
         DELETE FROM pk_member WHERE member_id NOT IN (SELECT id FROM member);
         DELETE FROM journal_author WHERE member_id NOT IN (SELECT id FROM member) OR entry_id NOT IN (SELECT id FROM journal_entry);
         DELETE FROM folder_member WHERE member_id NOT IN (SELECT id FROM member) OR folder_id NOT IN (SELECT id FROM folder);
         DELETE FROM folder WHERE parent_id IS NOT NULL AND parent_id NOT IN (SELECT id FROM folder);
         DELETE FROM message_author WHERE member_id NOT IN (SELECT id FROM member) OR message_id NOT IN (SELECT id FROM message);
         DELETE FROM message_recipient WHERE member_id NOT IN (SELECT id FROM member) OR message_id NOT IN (SELECT id FROM message);
         DELETE FROM message WHERE group_id IS NOT NULL AND group_id NOT IN (SELECT id FROM chat_group);
         UPDATE message SET reply_to = NULL WHERE reply_to IS NOT NULL AND reply_to NOT IN (SELECT id FROM message);
         DELETE FROM chat_group_member WHERE member_id NOT IN (SELECT id FROM member) OR group_id NOT IN (SELECT id FROM chat_group);
         DELETE FROM group_peer WHERE group_id NOT IN (SELECT id FROM chat_group);
         DELETE FROM message_system WHERE message_id NOT IN (SELECT id FROM message);
         UPDATE poll SET author_id = NULL WHERE author_id IS NOT NULL AND author_id NOT IN (SELECT id FROM member);
         DELETE FROM poll_option WHERE poll_id NOT IN (SELECT id FROM poll);
         DELETE FROM poll_vote WHERE member_id NOT IN (SELECT id FROM member) OR option_id NOT IN (SELECT id FROM poll_option);",
    )?;
    Ok(())
}

/// Applique des changements reçus, en une transaction. Une version locale plus
/// récente est gardée et renvoyée au prochain envoi.
pub fn apply(conn: &mut Connection, changes: &[Change], me: &str) -> Result<Applied> {
    let mut order: Vec<(usize, &Change)> = changes
        .iter()
        .filter_map(|c| match spec(&c.t) {
            Some((i, _)) if c.r.is_some() => Some((i, c)),
            Some((i, _)) => Some((TABLES.len() * 2 - i, c)),
            None => None,
        })
        .collect();
    let mut out = Applied {
        skipped: changes.len() - order.len(),
        ..Applied::default()
    };
    order.sort_by_key(|(i, _)| *i);

    let tx = conn.transaction()?;
    tx.execute("INSERT INTO sync_applying (x) VALUES (1)", [])?;
    tx.pragma_update(None, "defer_foreign_keys", true)?;
    for (_, c) in order {
        let Some((_, s)) = spec(&c.t) else { continue };
        let local: Option<(i64, Option<String>)> = tx
            .query_row(
                "SELECT at, dev FROM sync_state WHERE tbl = ?1 AND pk = ?2",
                params![c.t, c.p],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((at, dev)) = local {
            let dev = dev.unwrap_or_else(|| me.to_owned());
            match (c.a, c.d.as_str()).cmp(&(at, dev.as_str())) {
                std::cmp::Ordering::Equal => {
                    tx.execute(
                        "UPDATE sync_state SET dirty = 0, dev = ?3 WHERE tbl = ?1 AND pk = ?2",
                        params![c.t, c.p, dev],
                    )?;
                    continue;
                }
                std::cmp::Ordering::Less => {
                    tx.execute(
                        "UPDATE sync_state SET dirty = 1 WHERE tbl = ?1 AND pk = ?2",
                        params![c.t, c.p],
                    )?;
                    out.kept_local += 1;
                    continue;
                }
                std::cmp::Ordering::Greater => {}
            }
        }
        tx.execute_batch("SAVEPOINT one")?;
        match apply_one(&tx, s, c) {
            Ok(()) => {
                tx.execute_batch("RELEASE one")?;
                tx.execute(
                    "INSERT INTO sync_state (tbl, pk, at, dev, dirty) VALUES (?1, ?2, ?3, ?4, 0)
                     ON CONFLICT (tbl, pk) DO UPDATE SET at = excluded.at, dev = excluded.dev, dirty = 0",
                    params![c.t, c.p, c.a, c.d],
                )?;
                out.applied += 1;
            }
            Err(e) => {
                tx.execute_batch("ROLLBACK TO one; RELEASE one")?;
                tracing::warn!(table = %c.t, error = %e, "changement reçu ignoré");
                out.skipped += 1;
            }
        }
    }
    sweep_orphans(&tx)?;
    tx.execute("DELETE FROM sync_applying", [])?;
    tx.commit()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;

    fn db() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        migrations::run(&mut c).unwrap();
        c
    }

    fn add_member(c: &Connection, id: u8, name: &str) {
        c.execute(
            "INSERT INTO member (id, name, color, created_at) VALUES (?1, ?2, '#aabbcc', 1)",
            params![vec![id; 16], name],
        )
        .unwrap();
    }

    fn name(c: &Connection, id: u8) -> Option<String> {
        c.query_row(
            "SELECT name FROM member WHERE id = ?1",
            [vec![id; 16]],
            |r| r.get(0),
        )
        .optional()
        .unwrap()
    }

    /// Envoie tout ce qui est en attente de `a` vers `b`.
    fn ship(a: &mut Connection, da: &str, b: &mut Connection, db_: &str) -> Applied {
        let ch = pending(a, da, 1000).unwrap();
        mark_pushed(a, &ch, da).unwrap();
        apply(b, &ch, db_).unwrap()
    }

    #[test]
    fn tracks_and_replicates() {
        let (mut a, mut b) = (db(), db());
        add_member(&a, 1, "Alex");
        a.execute(
            "INSERT INTO front_session (id, member_id, started_at) VALUES (?1, ?2, 5)",
            params![vec![9u8; 16], vec![1u8; 16]],
        )
        .unwrap();
        a.execute(
            "INSERT INTO kv (key, value) VALUES ('safety.plan', '{}'), ('aragonite.refresh', 'x')",
            [],
        )
        .unwrap();
        assert_eq!(pending_count(&a).unwrap(), 3);
        let r = ship(&mut a, "A", &mut b, "B");
        assert_eq!(r.applied, 3);
        assert_eq!(pending_count(&a).unwrap(), 0);
        assert_eq!(pending_count(&b).unwrap(), 0, "rien n'est renvoyé en écho");
        assert_eq!(name(&b, 1).as_deref(), Some("Alex"));
        let secret: Option<String> = b
            .query_row(
                "SELECT value FROM kv WHERE key = 'aragonite.refresh'",
                [],
                |r| r.get(0),
            )
            .optional()
            .unwrap();
        assert_eq!(secret, None, "les jetons restent locaux");

        b.execute("DELETE FROM member WHERE id = ?1", [vec![1u8; 16]])
            .unwrap();
        ship(&mut b, "B", &mut a, "A");
        assert_eq!(name(&a, 1), None);
        let fronts: i64 = a
            .query_row("SELECT count(*) FROM front_session", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fronts, 0);
    }

    #[test]
    fn last_writer_wins_and_local_newer_is_kept() {
        let (mut a, mut b) = (db(), db());
        add_member(&a, 1, "Alex");
        ship(&mut a, "A", &mut b, "B");
        a.execute(
            "UPDATE member SET name = 'Old' WHERE id = ?1",
            [vec![1u8; 16]],
        )
        .unwrap();
        a.execute("UPDATE sync_state SET at = 10 WHERE tbl = 'member'", [])
            .unwrap();
        b.execute(
            "UPDATE member SET name = 'New' WHERE id = ?1",
            [vec![1u8; 16]],
        )
        .unwrap();
        let r = ship(&mut a, "A", &mut b, "B");
        assert_eq!(r.kept_local, 1);
        assert_eq!(name(&b, 1).as_deref(), Some("New"));
        assert_eq!(pending_count(&b).unwrap(), 1);
        ship(&mut b, "B", &mut a, "A");
        assert_eq!(name(&a, 1).as_deref(), Some("New"));
    }

    #[test]
    fn rejects_foreign_kv_and_bad_pk() {
        let mut b = db();
        let mut r = Map::new();
        r.insert("key".into(), Json::String("aragonite.refresh".into()));
        r.insert("value".into(), Json::String("pirate".into()));
        let ch = vec![
            Change {
                t: "kv".into(),
                p: "[\"aragonite.refresh\"]".into(),
                r: Some(r),
                a: 5,
                d: "X".into(),
            },
            Change {
                t: "outbox".into(),
                p: "[1]".into(),
                r: None,
                a: 5,
                d: "X".into(),
            },
            Change {
                t: "member".into(),
                p: "[\"zz\"]".into(),
                r: None,
                a: 5,
                d: "X".into(),
            },
        ];
        let out = apply(&mut b, &ch, "B").unwrap();
        assert_eq!(out.applied, 0);
        assert_eq!(out.skipped, 3);
        let n: i64 = b
            .query_row("SELECT count(*) FROM kv", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn open_front_conflict_closes_local() {
        let (mut a, mut b) = (db(), db());
        add_member(&a, 1, "Alex");
        ship(&mut a, "A", &mut b, "B");
        a.execute(
            "INSERT INTO front_session (id, member_id, started_at) VALUES (?1, ?2, 100)",
            params![vec![7u8; 16], vec![1u8; 16]],
        )
        .unwrap();
        b.execute(
            "INSERT INTO front_session (id, member_id, started_at) VALUES (?1, ?2, 50)",
            params![vec![8u8; 16], vec![1u8; 16]],
        )
        .unwrap();
        let r = ship(&mut a, "A", &mut b, "B");
        assert_eq!(r.applied, 1);
        let open: i64 = b
            .query_row(
                "SELECT count(*) FROM front_session WHERE ended_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(open, 1);
        let ch = pending(&b, "B", 10).unwrap();
        assert_eq!(ch.len(), 1, "seule la fermeture locale part au serveur");
        assert_eq!(ch[0].p, format!("[\"{}\"]", "08".repeat(16)));
        assert_eq!(ch[0].r.as_ref().unwrap()["ended_at"], Json::from(100));
    }

    #[test]
    fn seed_lets_remote_win() {
        let (mut a, mut b) = (db(), db());
        add_member(&a, 1, "Serveur");
        b.execute("INSERT INTO sync_applying VALUES (1)", [])
            .unwrap();
        add_member(&b, 1, "Local");
        b.execute("DELETE FROM sync_applying", []).unwrap();
        seed(&b).unwrap();
        assert_eq!(pending_count(&b).unwrap(), 1);
        ship(&mut a, "A", &mut b, "B");
        assert_eq!(name(&b, 1).as_deref(), Some("Serveur"));
        let profile_blob = vec![1u8, 2, 3];
        a.execute(
            "INSERT INTO member_profile (member_id, avatar) VALUES (?1, ?2)",
            params![vec![1u8; 16], profile_blob],
        )
        .unwrap();
        ship(&mut a, "A", &mut b, "B");
        let got: Vec<u8> = b
            .query_row("SELECT avatar FROM member_profile", [], |r| r.get(0))
            .unwrap();
        assert_eq!(got, vec![1, 2, 3]);
    }
}
