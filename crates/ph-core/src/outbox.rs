//! File d'envoi durable : les changements locaux destinés à un service distant
//! y sont écrits dans la même transaction que le changement lui-même. Ils
//! survivent ainsi à une coupure réseau ou à une fermeture de l'application.

use rusqlite::{Connection, params};

use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: i64,
    pub topic: String,
    pub payload: String,
    pub created_at: i64,
    pub attempts: i64,
}

pub fn push(conn: &Connection, topic: &str, payload: &str, created_at: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO outbox (topic, payload, created_at) VALUES (?1, ?2, ?3)",
        params![topic, payload, created_at],
    )?;
    Ok(())
}

/// Les plus anciens d'abord : l'ordre des changements est préservé.
pub fn pending(conn: &Connection, topic: &str, limit: u32) -> Result<Vec<Item>> {
    let mut st = conn.prepare_cached(
        "SELECT id, topic, payload, created_at, attempts FROM outbox
         WHERE topic = ?1 ORDER BY id LIMIT ?2",
    )?;
    let rows = st.query_map(params![topic, limit], |r| {
        Ok(Item {
            id: r.get(0)?,
            topic: r.get(1)?,
            payload: r.get(2)?,
            created_at: r.get(3)?,
            attempts: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn done(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM outbox WHERE id = ?1", [id])?;
    Ok(())
}

pub fn failed(conn: &Connection, id: i64, error: &str) -> Result<()> {
    conn.execute(
        "UPDATE outbox SET attempts = attempts + 1, last_error = ?2 WHERE id = ?1",
        params![id, error],
    )?;
    Ok(())
}

pub fn count(conn: &Connection, topic: &str) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM outbox WHERE topic = ?1",
        [topic],
        |r| r.get(0),
    )?)
}
