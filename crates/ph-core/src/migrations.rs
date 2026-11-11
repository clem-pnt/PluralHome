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
    // 4 — réglages (dont secrets : jetons), file d'envoi vers PluralKit et correspondances.
    "CREATE TABLE kv (
        key   TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    ) STRICT, WITHOUT ROWID;
    CREATE TABLE outbox (
        id         INTEGER PRIMARY KEY,
        topic      TEXT NOT NULL,
        payload    TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        attempts   INTEGER NOT NULL DEFAULT 0,
        last_error TEXT
    ) STRICT;
    CREATE TABLE pk_member (
        member_id BLOB PRIMARY KEY NOT NULL REFERENCES member (id) ON DELETE CASCADE,
        pk_uuid   TEXT NOT NULL UNIQUE,
        pk_hid    TEXT NOT NULL,
        snapshot  TEXT NOT NULL
    ) STRICT, WITHOUT ROWID;
    CREATE TABLE pk_switch (
        pk_id     TEXT PRIMARY KEY NOT NULL,
        timestamp INTEGER NOT NULL
    ) STRICT, WITHOUT ROWID;",
    // 5 — limites alignées sur PluralKit (100 caractères), pour ne jamais tronquer
    // un nom reçu puis le renvoyer amputé. SQLite impose de reconstruire la table.
    "CREATE TABLE member_v5 (
        id          BLOB PRIMARY KEY NOT NULL CHECK (length(id) = 16),
        name        TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
        pronouns    TEXT CHECK (pronouns IS NULL OR length(pronouns) <= 100),
        color       TEXT NOT NULL CHECK (color GLOB '#[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]'),
        created_at  INTEGER NOT NULL,
        archived_at INTEGER
    ) STRICT;
    INSERT INTO member_v5 SELECT id, name, pronouns, color, created_at, archived_at FROM member;
    DROP TABLE member;
    ALTER TABLE member_v5 RENAME TO member;
    CREATE INDEX member_active ON member (archived_at, name);",
    // 6 — profil détaillé, propre à l'appareil : jamais envoyé à PluralKit.
    // roles : tableau JSON de libellés ; avatar : image PNG, JPEG ou WebP déjà réduite.
    "CREATE TABLE member_profile (
        member_id   BLOB PRIMARY KEY NOT NULL REFERENCES member (id) ON DELETE CASCADE,
        description TEXT CHECK (description IS NULL OR length(description) <= 2000),
        roles       TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(roles)),
        avatar      BLOB CHECK (avatar IS NULL OR length(avatar) <= 262144)
    ) STRICT;",
];

pub fn run(conn: &mut Connection) -> Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let current = usize::try_from(current).map_err(|_| CoreError::FutureSchema)?;
    if current > MIGRATIONS.len() {
        return Err(CoreError::FutureSchema);
    }
    if current == MIGRATIONS.len() {
        return Ok(());
    }
    // Les reconstructions de table exigent de suspendre les clés étrangères
    // (impossible dans une transaction) ; on vérifie leur cohérence ensuite.
    conn.pragma_update(None, "foreign_keys", false)?;
    let result = apply(conn, current);
    conn.pragma_update(None, "foreign_keys", true)?;
    result?;
    let broken = conn
        .prepare("PRAGMA foreign_key_check")?
        .query([])?
        .next()?
        .is_some();
    if broken {
        return Err(CoreError::Invalid("schema"));
    }
    Ok(())
}

fn apply(conn: &mut Connection, current: usize) -> Result<()> {
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
    fn member_rebuild_keeps_references() {
        // Repartir d'une base en version 4 avec des données liées au membre.
        let mut conn4 = Connection::open_in_memory().unwrap();
        for sql in &MIGRATIONS[..4] {
            conn4.execute_batch(sql).unwrap();
        }
        conn4.pragma_update(None, "user_version", 4).unwrap();
        let id = [9u8; 16];
        conn4.execute("INSERT INTO member (id, name, color, created_at) VALUES (?1, 'Mila', '#4f8583', 0)", [id]).unwrap();
        conn4
            .execute(
                "INSERT INTO front_session (id, member_id, started_at) VALUES (?1, ?2, 0)",
                [[1u8; 16], id],
            )
            .unwrap();
        conn4
            .execute(
                "INSERT INTO message (id, author_id, body, created_at) VALUES (?1, ?2, 'salut', 0)",
                [[2u8; 16], id],
            )
            .unwrap();
        conn4.pragma_update(None, "foreign_keys", true).unwrap();

        run(&mut conn4).unwrap();
        let long = "n".repeat(100);
        conn4
            .execute("UPDATE member SET name = ?1", [&long])
            .unwrap();
        let fronts: i64 = conn4
            .query_row(
                "SELECT count(*) FROM front_session WHERE member_id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fronts, 1);
        // Les clés étrangères pointent toujours vers la nouvelle table.
        conn4.execute("DELETE FROM member", []).unwrap();
        let fronts: i64 = conn4
            .query_row("SELECT count(*) FROM front_session", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fronts, 0, "ON DELETE CASCADE toujours actif");
    }

    #[test]
    fn refuses_newer_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 999).unwrap();
        assert!(matches!(run(&mut conn), Err(CoreError::FutureSchema)));
    }
}
