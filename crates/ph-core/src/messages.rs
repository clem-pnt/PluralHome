//! Messagerie interne et « depuis ton dernier passage ».
//!
//! Les messages sont un tableau partagé : adressés à un membre ou à tout le
//! système. L'état de lecture est suivi par membre, pour qu'au retour au front
//! chacun retrouve ce qui lui a été laissé.

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::front::{self, FrontSession};
use crate::{CoreError, Result, members};

pub const MAX_BODY_CHARS: usize = 4000;
const PAGE_MAX: u32 = 200;

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: Uuid,
    pub author_id: Option<Uuid>,
    /// `None` : pour tout le système.
    pub recipient_id: Option<Uuid>,
    pub body: String,
    pub pinned: bool,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewMessage {
    pub author_id: Option<Uuid>,
    pub recipient_id: Option<Uuid>,
    pub body: String,
}

/// Ce qu'un membre a manqué depuis son dernier passage au front.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatchUp {
    pub member_id: Uuid,
    /// Fin du dernier passage terminé ; `None` si c'est le premier.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub last_seen: Option<i64>,
    /// Passages des autres depuis `last_seen`.
    pub fronts: Vec<FrontSession>,
    /// Messages qui le concernent et qu'il n'a pas encore lus.
    pub unread: Vec<Message>,
}

fn from_row(r: &Row<'_>) -> rusqlite::Result<Message> {
    let id: [u8; 16] = r.get(0)?;
    let author: Option<[u8; 16]> = r.get(1)?;
    let recipient: Option<[u8; 16]> = r.get(2)?;
    Ok(Message {
        id: Uuid::from_bytes(id),
        author_id: author.map(Uuid::from_bytes),
        recipient_id: recipient.map(Uuid::from_bytes),
        body: r.get(3)?,
        pinned: r.get(4)?,
        created_at: r.get(5)?,
    })
}

const COLUMNS: &str = "m.id, m.author_id, m.recipient_id, m.body, m.pinned, m.created_at";

pub fn post(conn: &Connection, input: &NewMessage) -> Result<Message> {
    // On garde le texte tel quel (mise en forme voulue), seuls les blancs aux bords partent.
    let body = input.body.trim().to_owned();
    if body.is_empty() || body.chars().count() > MAX_BODY_CHARS {
        return Err(CoreError::Invalid("body"));
    }
    let people: Vec<Uuid> = input
        .author_id
        .into_iter()
        .chain(input.recipient_id)
        .collect();
    members::ensure_active(conn, &people)?;
    let m = Message {
        id: Uuid::now_v7(),
        author_id: input.author_id,
        recipient_id: input.recipient_id,
        body,
        pinned: false,
        created_at: crate::time::now_ms()?,
    };
    conn.execute(
        "INSERT INTO message (id, author_id, recipient_id, body, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            m.id.as_bytes(),
            m.author_id.map(|u| *u.as_bytes()),
            m.recipient_id.map(|u| *u.as_bytes()),
            m.body,
            m.created_at
        ],
    )?;
    Ok(m)
}

/// Tableau : épinglés d'abord, puis du plus récent au plus ancien.
/// Pagination par curseur sur l'identifiant (`UUIDv7`, strictement croissant dans le
/// temps) : contrairement à l'horodatage, il ne connaît pas d'égalité, donc rien n'est sauté.
pub fn list(conn: &Connection, before: Option<Uuid>, limit: u32) -> Result<Vec<Message>> {
    let limit = limit.clamp(1, PAGE_MAX);
    let mut st = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM message m
         WHERE ?1 IS NULL OR (m.id < ?1 AND m.pinned = 0)
         ORDER BY m.pinned DESC, m.id DESC
         LIMIT ?2"
    ))?;
    Ok(st
        .query_map(params![before.map(|u| *u.as_bytes()), limit], from_row)?
        .collect::<rusqlite::Result<_>>()?)
}

pub fn set_pinned(conn: &Connection, id: Uuid, pinned: bool) -> Result<()> {
    let n = conn.execute(
        "UPDATE message SET pinned = ?2 WHERE id = ?1",
        params![id.as_bytes(), pinned],
    )?;
    if n == 0 {
        Err(CoreError::NotFound)
    } else {
        Ok(())
    }
}

pub fn delete(conn: &Connection, id: Uuid) -> Result<()> {
    let n = conn.execute("DELETE FROM message WHERE id = ?1", [id.as_bytes()])?;
    if n == 0 {
        Err(CoreError::NotFound)
    } else {
        Ok(())
    }
}

pub fn catch_up(conn: &Connection, member_id: Uuid) -> Result<CatchUp> {
    members::ensure_active(conn, &[member_id])?;
    let last_seen: Option<i64> = conn
        .query_row(
            "SELECT max(ended_at) FROM front_session WHERE member_id = ?1 AND ended_at IS NOT NULL",
            [member_id.as_bytes()],
            |r| r.get(0),
        )
        .optional()?
        .flatten();

    let acked_at: Option<i64> = conn
        .query_row(
            "SELECT acked_at FROM catchup_ack WHERE member_id = ?1",
            [member_id.as_bytes()],
            |r| r.get(0),
        )
        .optional()?;

    // Un accusé postérieur au dernier passage masque ce qui a déjà été vu ;
    // seuls les passages commencés depuis restent à rattraper.
    let fronts = match last_seen {
        Some(since) => {
            let seen_until = acked_at.filter(|a| *a >= since);
            front::history(conn, since, i64::MAX)?
                .into_iter()
                .filter(|s| s.member_id != Some(member_id))
                .filter(|s| seen_until.is_none_or(|a| s.started_at > a))
                .collect()
        }
        None => Vec::new(),
    };

    let mut st = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM message m
         WHERE (m.recipient_id IS NULL OR m.recipient_id = ?1)
           AND (m.author_id IS NULL OR m.author_id <> ?1)
           AND NOT EXISTS (SELECT 1 FROM message_read r WHERE r.message_id = m.id AND r.member_id = ?1)
         ORDER BY m.id"
    ))?;
    let unread = st
        .query_map([member_id.as_bytes()], from_row)?
        .collect::<rusqlite::Result<_>>()?;

    Ok(CatchUp {
        member_id,
        last_seen,
        fronts,
        unread,
    })
}

/// « J'ai lu » : marque les messages donnés comme lus pour ce membre et
/// enregistre qu'il a pris connaissance des passages au front jusqu'à maintenant.
pub fn mark_read(conn: &mut Connection, member_id: Uuid, ids: &[Uuid]) -> Result<()> {
    let now = crate::time::now_ms()?;
    let tx = conn.transaction()?;
    members::ensure_active(&tx, &[member_id])?;
    {
        let mut st = tx.prepare_cached(
            "INSERT OR IGNORE INTO message_read (message_id, member_id, read_at)
             SELECT id, ?2, ?3 FROM message WHERE id = ?1",
        )?;
        for id in ids {
            st.execute(params![id.as_bytes(), member_id.as_bytes(), now])?;
        }
    }
    tx.execute(
        "INSERT INTO catchup_ack (member_id, acked_at) VALUES (?1, ?2)
         ON CONFLICT (member_id) DO UPDATE SET acked_at = max(acked_at, excluded.acked_at)",
        params![member_id.as_bytes(), now],
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::front::FrontChange;
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

    fn msg(author: Option<Uuid>, to: Option<Uuid>, body: &str) -> NewMessage {
        NewMessage {
            author_id: author,
            recipient_id: to,
            body: body.into(),
        }
    }

    #[test]
    fn post_validates() {
        let (_t, v, a, _) = setup();
        let c = v.conn().unwrap();
        assert!(matches!(
            post(c, &msg(Some(a), None, "   ")),
            Err(CoreError::Invalid("body"))
        ));
        let long = "x".repeat(MAX_BODY_CHARS + 1);
        assert!(matches!(
            post(c, &msg(Some(a), None, &long)),
            Err(CoreError::Invalid("body"))
        ));
        assert!(matches!(
            post(c, &msg(Some(Uuid::now_v7()), None, "salut")),
            Err(CoreError::NotFound)
        ));
        assert_eq!(
            post(c, &msg(None, None, "  salut\n")).unwrap().body,
            "salut"
        );
    }

    #[test]
    fn list_orders_pinned_first_and_paginates() {
        let (_t, v, a, _) = setup();
        let c = v.conn().unwrap();
        let first = post(c, &msg(Some(a), None, "un")).unwrap();
        for body in ["deux", "trois"] {
            post(c, &msg(Some(a), None, body)).unwrap();
        }
        set_pinned(c, first.id, true).unwrap();
        let all = list(c, None, 50).unwrap();
        assert_eq!(all[0].id, first.id);
        assert_eq!(all.len(), 3);
        let page = list(c, None, 2).unwrap();
        assert_eq!(page.len(), 2);
        let next = list(c, Some(page[1].id), 2).unwrap();
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].body, "deux");
        delete(c, first.id).unwrap();
        assert!(matches!(delete(c, first.id), Err(CoreError::NotFound)));
    }

    #[test]
    fn catch_up_after_absence() {
        let (_t, mut v, mila, sacha) = setup();
        let c = v.conn_mut().unwrap();
        // Mila au front, puis Sacha prend le relais.
        front::change(
            c,
            &FrontChange {
                members: vec![mila],
                blurry: false,
            },
        )
        .unwrap();
        front::change(
            c,
            &FrontChange {
                members: vec![sacha],
                blurry: false,
            },
        )
        .unwrap();

        let for_mila = post(c, &msg(Some(sacha), Some(mila), "J'ai payé la facture.")).unwrap();
        let for_all = post(c, &msg(Some(sacha), None, "RDV mardi 10 h.")).unwrap();
        post(c, &msg(Some(mila), Some(sacha), "pas pour Mila")).unwrap();

        let cu = catch_up(c, mila).unwrap();
        assert!(cu.last_seen.is_some());
        assert_eq!(cu.fronts.len(), 1);
        assert_eq!(cu.fronts[0].member_id, Some(sacha));
        let ids: Vec<Uuid> = cu.unread.iter().map(|m| m.id).collect();
        assert_eq!(ids, vec![for_mila.id, for_all.id]);

        mark_read(c, mila, &ids).unwrap();
        mark_read(c, mila, &ids).unwrap(); // idempotent
        let after = catch_up(c, mila).unwrap();
        assert!(after.unread.is_empty());
        assert!(
            after.fronts.is_empty(),
            "les passages déjà vus ne reviennent pas"
        );
        // Un auteur ne se voit jamais présenter ses propres messages.
        assert!(
            catch_up(c, sacha)
                .unwrap()
                .unread
                .iter()
                .all(|m| m.author_id != Some(sacha))
        );
    }

    #[test]
    fn first_time_has_no_history() {
        let (_t, v, mila, _) = setup();
        let cu = catch_up(v.conn().unwrap(), mila).unwrap();
        assert_eq!(cu.last_seen, None);
        assert!(cu.fronts.is_empty());
    }
}
