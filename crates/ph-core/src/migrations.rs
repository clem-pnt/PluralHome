//! Migrations versionnées par `PRAGMA user_version`. Une migration publiée
//! n'est jamais modifiée : on en ajoute une nouvelle à la fin.

use rusqlite::Connection;

use crate::{CoreError, Result};

/// Horloge SQLite en millisecondes Unix (même unité que `time::now_ms`).
macro_rules! sql_now {
    () => {
        "CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)"
    };
}

/// Note toute écriture locale d'une table synchronisée dans `sync_state`, sauf
/// pendant l'application de changements reçus (marqueur `sync_applying`).
/// L'horodatage ne recule jamais : une écriture locale bat toujours la version connue.
macro_rules! track {
    ($t:literal, $new:literal, $old:literal, $wn:literal, $wo:literal) => {
        concat!(
            "CREATE TRIGGER sync_", $t, "_ins AFTER INSERT ON ", $t,
            " WHEN NOT EXISTS (SELECT 1 FROM sync_applying)", $wn, " BEGIN ",
            track!(@up $t, $new), " END;\n",
            "CREATE TRIGGER sync_", $t, "_upd AFTER UPDATE ON ", $t,
            " WHEN NOT EXISTS (SELECT 1 FROM sync_applying)", $wn, " BEGIN ",
            track!(@up $t, $new),
            " INSERT INTO sync_state (tbl, pk, at, dev, dirty) SELECT '", $t, "', ", $old, ", ",
            sql_now!(), ", NULL, 1 WHERE ", $old, " <> ", $new,
            " ON CONFLICT (tbl, pk) DO UPDATE SET at = max(excluded.at, sync_state.at + 1), dev = NULL, dirty = 1;",
            " END;\n",
            "CREATE TRIGGER sync_", $t, "_del AFTER DELETE ON ", $t,
            " WHEN NOT EXISTS (SELECT 1 FROM sync_applying)", $wo, " BEGIN ",
            track!(@up $t, $old), " END;\n",
        )
    };
    (@up $t:literal, $pk:literal) => {
        concat!(
            "INSERT INTO sync_state (tbl, pk, at, dev, dirty) VALUES ('", $t, "', ", $pk, ", ",
            sql_now!(), ", NULL, 1) ON CONFLICT (tbl, pk) DO UPDATE SET ",
            "at = max(excluded.at, sync_state.at + 1), dev = NULL, dirty = 1;"
        )
    };
    ($t:literal, $new:literal, $old:literal) => {
        track!($t, $new, $old, "", "")
    };
}

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
    // 7 — journal partagé. Plusieurs auteurs possibles (co-front) ; aucun = auteur inconnu.
    "CREATE TABLE journal_entry (
        id         BLOB PRIMARY KEY NOT NULL CHECK (length(id) = 16),
        title      TEXT CHECK (title IS NULL OR length(title) BETWEEN 1 AND 120),
        body       TEXT NOT NULL CHECK (length(body) BETWEEN 1 AND 20000),
        mood       INTEGER CHECK (mood IS NULL OR mood BETWEEN 1 AND 5),
        created_at INTEGER NOT NULL,
        updated_at INTEGER
    ) STRICT;
    CREATE TABLE journal_author (
        entry_id  BLOB NOT NULL REFERENCES journal_entry (id) ON DELETE CASCADE,
        member_id BLOB NOT NULL REFERENCES member (id) ON DELETE CASCADE,
        PRIMARY KEY (entry_id, member_id)
    ) STRICT, WITHOUT ROWID;
    CREATE INDEX journal_by_member ON journal_author (member_id, entry_id);",
    // 8 — synchronisation chiffrée entre appareils : version de chaque ligne
    // (horodatage + appareil, dernier écrivain gagne) et drapeau « à envoyer ».
    // dev NULL = cet appareil. Une ligne absente de sa table = suppression.
    concat!(
        "CREATE TABLE sync_state (
            tbl   TEXT NOT NULL,
            pk    TEXT NOT NULL,
            at    INTEGER NOT NULL,
            dev   TEXT,
            dirty INTEGER NOT NULL DEFAULT 1,
            PRIMARY KEY (tbl, pk)
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX sync_dirty ON sync_state (tbl, pk) WHERE dirty = 1;
        CREATE TABLE sync_applying (x INTEGER) STRICT;\n",
        track!("member", "json_array(hex(NEW.id))", "json_array(hex(OLD.id))"),
        track!("member_profile", "json_array(hex(NEW.member_id))", "json_array(hex(OLD.member_id))"),
        track!("front_session", "json_array(hex(NEW.id))", "json_array(hex(OLD.id))"),
        track!("message", "json_array(hex(NEW.id))", "json_array(hex(OLD.id))"),
        track!(
            "message_read",
            "json_array(hex(NEW.message_id), hex(NEW.member_id))",
            "json_array(hex(OLD.message_id), hex(OLD.member_id))"
        ),
        track!("catchup_ack", "json_array(hex(NEW.member_id))", "json_array(hex(OLD.member_id))"),
        track!("journal_entry", "json_array(hex(NEW.id))", "json_array(hex(OLD.id))"),
        track!(
            "journal_author",
            "json_array(hex(NEW.entry_id), hex(NEW.member_id))",
            "json_array(hex(OLD.entry_id), hex(OLD.member_id))"
        ),
        track!("pk_member", "json_array(hex(NEW.member_id))", "json_array(hex(OLD.member_id))"),
        track!("pk_switch", "json_array(NEW.pk_id)", "json_array(OLD.pk_id)"),
        track!(
            "kv",
            "json_array(NEW.key)",
            "json_array(OLD.key)",
            " AND NEW.key = 'safety.plan'",
            " AND OLD.key = 'safety.plan'"
        ),
    ),
    // 9 — catalogue de rôles propre au système, partagé entre appareils. Le nom
    // sert de clé : deux appareils qui créent le même rôle ne font qu'une ligne.
    // Amorcé avec les rôles déjà portés par les membres.
    concat!(
        "CREATE TABLE role (
            name       TEXT PRIMARY KEY NOT NULL CHECK (length(name) BETWEEN 1 AND 40),
            created_at INTEGER NOT NULL
        ) STRICT, WITHOUT ROWID;\n",
        track!("role", "json_array(NEW.name)", "json_array(OLD.name)"),
        "INSERT OR IGNORE INTO role (name, created_at)
            SELECT DISTINCT trim(j.value), ",
        sql_now!(),
        " FROM member_profile p, json_each(p.roles) j
            WHERE j.type = 'text' AND length(trim(j.value)) BETWEEN 1 AND 40;",
    ),
    // 10 — dossiers pour ranger les membres. Un membre peut être dans plusieurs
    // dossiers, ou aucun ; il reste toujours dans la liste de tous les membres.
    concat!(
        "CREATE TABLE folder (
            id         BLOB PRIMARY KEY NOT NULL CHECK (length(id) = 16),
            name       TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 60),
            created_at INTEGER NOT NULL
        ) STRICT;
        CREATE TABLE folder_member (
            folder_id BLOB NOT NULL REFERENCES folder (id) ON DELETE CASCADE,
            member_id BLOB NOT NULL REFERENCES member (id) ON DELETE CASCADE,
            PRIMARY KEY (folder_id, member_id)
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX folder_by_member ON folder_member (member_id, folder_id);\n",
        track!("folder", "json_array(hex(NEW.id))", "json_array(hex(OLD.id))"),
        track!(
            "folder_member",
            "json_array(hex(NEW.folder_id), hex(NEW.member_id))",
            "json_array(hex(OLD.folder_id), hex(OLD.member_id))"
        ),
    ),
    // 11 — sous-dossiers, statut au front (front, co-front, co-conscience),
    // messages à plusieurs auteurs et modifiables, votes, et nouveaux réglages
    // partagés entre appareils (plan d'accueil, proche de confiance, favoris).
    concat!(
        "ALTER TABLE folder ADD COLUMN parent_id BLOB REFERENCES folder (id) ON DELETE CASCADE;
        CREATE INDEX folder_by_parent ON folder (parent_id);
        ALTER TABLE front_session ADD COLUMN status TEXT NOT NULL DEFAULT 'front'
            CHECK (status IN ('front', 'cofront', 'coconscious'));
        ALTER TABLE message ADD COLUMN edited_at INTEGER;
        CREATE TABLE message_author (
            message_id BLOB NOT NULL REFERENCES message (id) ON DELETE CASCADE,
            member_id  BLOB NOT NULL REFERENCES member (id) ON DELETE CASCADE,
            PRIMARY KEY (message_id, member_id)
        ) STRICT, WITHOUT ROWID;
        CREATE TABLE message_recipient (
            message_id BLOB NOT NULL REFERENCES message (id) ON DELETE CASCADE,
            member_id  BLOB NOT NULL REFERENCES member (id) ON DELETE CASCADE,
            PRIMARY KEY (message_id, member_id)
        ) STRICT, WITHOUT ROWID;
        CREATE TABLE poll (
            id         BLOB PRIMARY KEY NOT NULL CHECK (length(id) = 16),
            question   TEXT NOT NULL CHECK (length(question) BETWEEN 1 AND 300),
            author_id  BLOB REFERENCES member (id) ON DELETE SET NULL,
            multi      INTEGER NOT NULL DEFAULT 0 CHECK (multi IN (0, 1)),
            created_at INTEGER NOT NULL,
            closed_at  INTEGER
        ) STRICT;
        CREATE TABLE poll_option (
            id       BLOB PRIMARY KEY NOT NULL CHECK (length(id) = 16),
            poll_id  BLOB NOT NULL REFERENCES poll (id) ON DELETE CASCADE,
            label    TEXT NOT NULL CHECK (length(label) BETWEEN 1 AND 100),
            position INTEGER NOT NULL
        ) STRICT;
        CREATE INDEX poll_option_by_poll ON poll_option (poll_id, position);
        CREATE TABLE poll_vote (
            option_id BLOB NOT NULL REFERENCES poll_option (id) ON DELETE CASCADE,
            member_id BLOB NOT NULL REFERENCES member (id) ON DELETE CASCADE,
            voted_at  INTEGER NOT NULL,
            PRIMARY KEY (option_id, member_id)
        ) STRICT, WITHOUT ROWID;
        INSERT INTO message_author SELECT id, author_id FROM message WHERE author_id IS NOT NULL;
        INSERT INTO message_recipient SELECT id, recipient_id FROM message WHERE recipient_id IS NOT NULL;
        DROP TRIGGER sync_kv_ins;
        DROP TRIGGER sync_kv_upd;
        DROP TRIGGER sync_kv_del;\n",
        track!(
            "message_author",
            "json_array(hex(NEW.message_id), hex(NEW.member_id))",
            "json_array(hex(OLD.message_id), hex(OLD.member_id))"
        ),
        track!(
            "message_recipient",
            "json_array(hex(NEW.message_id), hex(NEW.member_id))",
            "json_array(hex(OLD.message_id), hex(OLD.member_id))"
        ),
        track!("poll", "json_array(hex(NEW.id))", "json_array(hex(OLD.id))"),
        track!("poll_option", "json_array(hex(NEW.id))", "json_array(hex(OLD.id))"),
        track!(
            "poll_vote",
            "json_array(hex(NEW.option_id), hex(NEW.member_id))",
            "json_array(hex(OLD.option_id), hex(OLD.member_id))"
        ),
        track!(
            "kv",
            "json_array(NEW.key)",
            "json_array(OLD.key)",
            " AND NEW.key IN ('safety.plan', 'newcomer.plan', 'trusted.contact', 'front.favorites')",
            " AND OLD.key IN ('safety.plan', 'newcomer.plan', 'trusted.contact', 'front.favorites')"
        ),
    ),
    // 12 — groupes de discussion nommés, réponses à un message, et nom et
    // photo du système partagés entre appareils.
    concat!(
        "CREATE TABLE chat_group (
            id         BLOB PRIMARY KEY NOT NULL CHECK (length(id) = 16),
            name       TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
            avatar     BLOB,
            created_at INTEGER NOT NULL
        ) STRICT;
        CREATE TABLE chat_group_member (
            group_id  BLOB NOT NULL REFERENCES chat_group (id) ON DELETE CASCADE,
            member_id BLOB NOT NULL REFERENCES member (id) ON DELETE CASCADE,
            PRIMARY KEY (group_id, member_id)
        ) STRICT, WITHOUT ROWID;
        ALTER TABLE message ADD COLUMN group_id BLOB REFERENCES chat_group (id) ON DELETE CASCADE;
        ALTER TABLE message ADD COLUMN reply_to BLOB REFERENCES message (id) ON DELETE SET NULL;
        CREATE INDEX message_by_group ON message (group_id);
        CREATE INDEX message_by_reply ON message (reply_to);
        DROP TRIGGER sync_kv_ins;
        DROP TRIGGER sync_kv_upd;
        DROP TRIGGER sync_kv_del;\n",
        track!("chat_group", "json_array(hex(NEW.id))", "json_array(hex(OLD.id))"),
        track!(
            "chat_group_member",
            "json_array(hex(NEW.group_id), hex(NEW.member_id))",
            "json_array(hex(OLD.group_id), hex(OLD.member_id))"
        ),
        track!(
            "kv",
            "json_array(NEW.key)",
            "json_array(OLD.key)",
            " AND NEW.key IN ('safety.plan', 'newcomer.plan', 'trusted.contact', 'front.favorites', 'system.name', 'system.avatar', 'pluralkit.ignored')",
            " AND OLD.key IN ('safety.plan', 'newcomer.plan', 'trusted.contact', 'front.favorites', 'system.name', 'system.avatar', 'pluralkit.ignored')"
        ),
    ),
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
    fn role_catalog_is_seeded_from_member_roles() {
        let mut conn = Connection::open_in_memory().unwrap();
        for sql in &MIGRATIONS[..8] {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", 8).unwrap();
        let id = [3u8; 16];
        conn.execute(
            "INSERT INTO member (id, name, color, created_at) VALUES (?1, 'Mila', '#4f8583', 0)",
            [id],
        )
        .unwrap();
        conn.execute(
            r#"INSERT INTO member_profile (member_id, roles) VALUES (?1, '["Protecteur", " Petit·e ", 3]')"#,
            [id],
        )
        .unwrap();

        run(&mut conn).unwrap();
        let roles: Vec<String> = conn
            .prepare("SELECT name FROM role ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(roles, vec!["Petit·e", "Protecteur"]);
        // Suivis pour la synchro entre appareils.
        let tracked: i64 = conn
            .query_row(
                "SELECT count(*) FROM sync_state WHERE tbl = 'role'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tracked, 2);
    }

    #[test]
    fn refuses_newer_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 999).unwrap();
        assert!(matches!(run(&mut conn), Err(CoreError::FutureSchema)));
    }
}
