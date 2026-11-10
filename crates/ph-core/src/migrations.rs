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
    // 2 — présences au front. member_id NULL = front « flou » (on ne sait pas qui est là).
    "CREATE TABLE front_session (
        id         BLOB PRIMARY KEY NOT NULL CHECK (length(id) = 16),
        member_id  BLOB REFERENCES member (id) ON DELETE CASCADE,
        started_at INTEGER NOT NULL,
        ended_at   INTEGER CHECK (ended_at IS NULL OR ended_at >= started_at)
    ) STRICT;
    CREATE INDEX front_open ON front_session (member_id) WHERE ended_at IS NULL;
    CREATE INDEX front_range ON front_session (started_at, ended_at);
    CREATE UNIQUE INDEX front_one_open_per_member
        ON front_session (ifnull(member_id, x'00')) WHERE ended_at IS NULL;",
    // 3 — messagerie interne. author_id NULL = auteur inconnu ; recipient_id NULL = tout le système.
    "CREATE TABLE message (
        id           BLOB PRIMARY KEY NOT NULL CHECK (length(id) = 16),
        author_id    BLOB REFERENCES member (id) ON DELETE SET NULL,
        recipient_id BLOB REFERENCES member (id) ON DELETE CASCADE,
        body         TEXT NOT NULL CHECK (length(body) BETWEEN 1 AND 4000),
        pinned       INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
        created_at   INTEGER NOT NULL
    ) STRICT;
    CREATE INDEX message_recent ON message (created_at DESC);
    CREATE INDEX message_for ON message (recipient_id, created_at);
    CREATE TABLE message_read (
        message_id BLOB NOT NULL REFERENCES message (id) ON DELETE CASCADE,
        member_id  BLOB NOT NULL REFERENCES member (id) ON DELETE CASCADE,
        read_at    INTEGER NOT NULL,
        PRIMARY KEY (message_id, member_id)
    ) STRICT, WITHOUT ROWID;
    CREATE TABLE catchup_ack (
        member_id BLOB PRIMARY KEY NOT NULL REFERENCES member (id) ON DELETE CASCADE,
        acked_at  INTEGER NOT NULL
    ) STRICT, WITHOUT ROWID;",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn version(conn: &Connection) -> i64 {
        conn.pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn upgrades_existing_database_without_data_loss() {
        let mut conn = Connection::open_in_memory().unwrap();
        // Base telle que livrée en 0.1.0 : seule la première migration.
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute(
            "INSERT INTO member (id, name, color, created_at) VALUES (?1, 'Mila', '#4f8583', 0)",
            [[7u8; 16]],
        )
        .unwrap();

        run(&mut conn).unwrap();
        assert_eq!(version(&conn), i64::try_from(MIGRATIONS.len()).unwrap());
        let n: i64 = conn
            .query_row("SELECT count(*) FROM member", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);

        // Idempotent.
        run(&mut conn).unwrap();
        assert_eq!(version(&conn), i64::try_from(MIGRATIONS.len()).unwrap());
    }

    #[test]
    fn refuses_newer_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 999).unwrap();
        assert!(matches!(run(&mut conn), Err(CoreError::FutureSchema)));
    }
}
