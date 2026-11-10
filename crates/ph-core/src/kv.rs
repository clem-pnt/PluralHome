//! Réglages clé/valeur, stockés dans la base chiffrée : on y range aussi les
//! jetons (compte Aragonite, PluralKit), qui ne doivent jamais exister en clair sur disque.

use rusqlite::{Connection, OptionalExtension, params};

use crate::Result;

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0))
        .optional()?)
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO kv (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM kv WHERE key = ?1", [key])?;
    Ok(())
}

pub fn flag(conn: &Connection, key: &str) -> Result<bool> {
    Ok(get(conn, key)?.as_deref() == Some("1"))
}
