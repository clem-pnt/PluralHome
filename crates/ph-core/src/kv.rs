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

/// Toutes les entrées dont la clé commence par `prefix`, triées par clé.
pub fn list_prefix(conn: &Connection, prefix: &str) -> Result<Vec<(String, String)>> {
    let mut stmt = conn
        .prepare("SELECT key, value FROM kv WHERE substr(key, 1, length(?1)) = ?1 ORDER BY key")?;
    let rows = stmt.query_map([prefix], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
