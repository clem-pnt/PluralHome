//! Migrations versionnées par `PRAGMA user_version`. Une migration publiée
//! n'est jamais modifiée : on en ajoute une nouvelle à la fin.

use rusqlite::Connection;

use crate::{CoreError, Result};

const MIGRATIONS: &[&str] = &[
    // 1 — membres du système
    "CREATE TABLE member (
        id          BLOB PRIMARY KEY NOT NULL CHECK (length(id) = 16),
        name        TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 64),
        pronouns    TEXT CHECK (pronouns IS NULL OR length(pronouns) <= 32),
        color       TEXT NOT NULL CHECK (color GLOB '#[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]'),
        created_at  INTEGER NOT NULL,
        archived_at INTEGER
    ) STRICT;
    CREATE INDEX member_active ON member (archived_at, name);",
];

pub fn run(conn: &mut Connection) -> Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let current = usize::try_from(current).map_err(|_| CoreError::FutureSchema)?;
    if current > MIGRATIONS.len() {
        return Err(CoreError::FutureSchema);
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(
            None,
            "user_version",
            i64::try_from(i + 1).unwrap_or(i64::MAX),
        )?;
        tx.commit()?;
        tracing::info!(version = i + 1, "migration appliquée");
    }
    Ok(())
}
