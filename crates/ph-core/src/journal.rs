//! Journal partagé : ce qui s'est passé, écrit par qui était là. Il sert de
//! mémoire commune quand l'amnésie coupe le fil d'une journée.

use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CoreError, Result, members};

pub const MAX_TITLE: usize = 120;
pub const MAX_BODY: usize = 20_000;
pub const MAX_AUTHORS: usize = 20;
const PAGE_MAX: u32 = 100;

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: Uuid,
    pub title: Option<String>,
    pub body: String,
    /// Humeur de 1 (très difficile) à 5 (très bien), facultative.
    pub mood: Option<u8>,
    /// Vide : auteur inconnu.
    pub author_ids: Vec<Uuid>,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub updated_at: Option<i64>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewEntry {
    pub title: Option<String>,
    pub body: String,
    pub mood: Option<u8>,
    pub author_ids: Vec<Uuid>,
}

struct Clean {
    title: Option<String>,
    body: String,
    mood: Option<u8>,
    authors: Vec<Uuid>,
}

fn validate(conn: &Connection, input: NewEntry) -> Result<Clean> {
    let title = input
        .title
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty());
    if title
        .as_ref()
        .is_some_and(|t| t.chars().count() > MAX_TITLE)
    {
        return Err(CoreError::Invalid("title"));
    }
    let body = input.body.trim().to_owned();
    if body.is_empty() || body.chars().count() > MAX_BODY {
        return Err(CoreError::Invalid("body"));
    }
    if input.mood.is_some_and(|m| !(1..=5).contains(&m)) {
        return Err(CoreError::Invalid("mood"));
    }
    let mut authors = input.author_ids;
    authors.sort_unstable();
    authors.dedup();
    if authors.len() > MAX_AUTHORS {
        return Err(CoreError::Invalid("authors"));
    }
    members::ensure_active(conn, &authors)?;
    Ok(Clean {
        title,
        body,
        mood: input.mood,
        authors,
    })
}

fn set_authors(conn: &Connection, id: Uuid, authors: &[Uuid]) -> Result<()> {
    conn.execute(
        "DELETE FROM journal_author WHERE entry_id = ?1",
        [id.as_bytes()],
    )?;
    let mut st =
        conn.prepare_cached("INSERT INTO journal_author (entry_id, member_id) VALUES (?1, ?2)")?;
    for a in authors {
        st.execute(params![id.as_bytes(), a.as_bytes()])?;
    }
    Ok(())
}

pub fn create(conn: &mut Connection, input: NewEntry) -> Result<Entry> {
    let tx = conn.transaction()?;
    let c = validate(&tx, input)?;
    let id = Uuid::now_v7();
    tx.execute(
        "INSERT INTO journal_entry (id, title, body, mood, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id.as_bytes(),
            c.title,
            c.body,
            c.mood,
            crate::time::now_ms()?
        ],
    )?;
    set_authors(&tx, id, &c.authors)?;
    let e = get(&tx, id)?;
    tx.commit()?;
    Ok(e)
}

/// Modifie une entrée (texte, humeur, auteurs) ; la date d'écriture ne bouge pas.
pub fn update(conn: &mut Connection, id: Uuid, input: NewEntry) -> Result<Entry> {
    let tx = conn.transaction()?;
    let c = validate(&tx, input)?;
    let n = tx.execute(
        "UPDATE journal_entry SET title = ?2, body = ?3, mood = ?4, updated_at = ?5 WHERE id = ?1",
        params![
            id.as_bytes(),
            c.title,
            c.body,
            c.mood,
            crate::time::now_ms()?
        ],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    set_authors(&tx, id, &c.authors)?;
    let e = get(&tx, id)?;
    tx.commit()?;
    Ok(e)
}

pub fn delete(conn: &Connection, id: Uuid) -> Result<()> {
    let n = conn.execute("DELETE FROM journal_entry WHERE id = ?1", [id.as_bytes()])?;
    if n == 0 {
        Err(CoreError::NotFound)
    } else {
        Ok(())
    }
}

const COLUMNS: &str = "e.id, e.title, e.body, e.mood, e.created_at, e.updated_at,
    (SELECT group_concat(hex(a.member_id)) FROM journal_author a WHERE a.entry_id = e.id)";

fn from_row(r: &Row<'_>) -> rusqlite::Result<Entry> {
    let id: [u8; 16] = r.get(0)?;
    let authors: Option<String> = r.get(6)?;
    let mut author_ids: Vec<Uuid> = authors
        .unwrap_or_default()
        .split(',')
        .filter_map(|h| Uuid::try_parse(h).ok())
        .collect();
    author_ids.sort_unstable();
    Ok(Entry {
        id: Uuid::from_bytes(id),
        title: r.get(1)?,
        body: r.get(2)?,
        mood: r.get(3)?,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
        author_ids,
    })
}

pub fn get(conn: &Connection, id: Uuid) -> Result<Entry> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM journal_entry e WHERE e.id = ?1"),
        [id.as_bytes()],
        from_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound,
        e => e.into(),
    })
}

/// Du plus récent au plus ancien, pagination par curseur `UUIDv7` (voir `messages::list`).
/// `author` : seulement les entrées écrites (au moins en partie) par ce membre.
pub fn list(
    conn: &Connection,
    before: Option<Uuid>,
    author: Option<Uuid>,
    limit: u32,
) -> Result<Vec<Entry>> {
    let limit = limit.clamp(1, PAGE_MAX);
    let mut st = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM journal_entry e
         WHERE (?1 IS NULL OR e.id < ?1)
           AND (?2 IS NULL OR EXISTS (
                SELECT 1 FROM journal_author a WHERE a.entry_id = e.id AND a.member_id = ?2))
         ORDER BY e.id DESC
         LIMIT ?3"
    ))?;
    Ok(st
        .query_map(
            params![
                before.map(|u| *u.as_bytes()),
                author.map(|u| *u.as_bytes()),
                limit
            ],
            from_row,
        )?
        .collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vault;
    use crate::keyfile::KdfParams;
    use crate::members::NewMember;

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
        let b = mk(&v, "Sam");
        (tmp, v, a, b)
    }

    fn entry(body: &str, authors: Vec<Uuid>) -> NewEntry {
        NewEntry {
            title: None,
            body: body.into(),
            mood: None,
            author_ids: authors,
        }
    }

    #[test]
    fn create_edit_filter_paginate() {
        let (_t, mut v, a, b) = setup();
        let c = v.conn_mut().unwrap();
        let e1 = create(c, entry(" Rendez-vous chez le médecin. ", vec![a, b, a])).unwrap();
        let mut expected = vec![a, b];
        expected.sort_unstable();
        assert_eq!(e1.author_ids, expected);
        assert_eq!(e1.body, "Rendez-vous chez le médecin.");
        let e2 = create(c, entry("Courses.", vec![b])).unwrap();
        let e3 = create(c, entry("Inconnu.", vec![])).unwrap();
        assert!(e3.author_ids.is_empty());

        let all: Vec<Uuid> = list(c, None, None, 10)
            .unwrap()
            .iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(all, vec![e3.id, e2.id, e1.id]);
        let page2: Vec<Uuid> = list(c, Some(e2.id), None, 10)
            .unwrap()
            .iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(page2, vec![e1.id]);
        let by_a: Vec<Uuid> = list(c, None, Some(a), 10)
            .unwrap()
            .iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(by_a, vec![e1.id]);

        let edited = update(
            c,
            e1.id,
            NewEntry {
                title: Some("Médecin".into()),
                body: "Vu le Dr L.".into(),
                mood: Some(4),
                author_ids: vec![a],
            },
        )
        .unwrap();
        assert_eq!(edited.author_ids, vec![a]);
        assert_eq!(edited.created_at, e1.created_at);
        assert!(edited.updated_at.is_some());
        assert_eq!(list(c, None, Some(b), 10).unwrap().len(), 1);

        delete(c, e2.id).unwrap();
        assert!(matches!(delete(c, e2.id), Err(CoreError::NotFound)));
    }

    #[test]
    fn rejects_bad_input() {
        let (_t, mut v, _a, _b) = setup();
        let c = v.conn_mut().unwrap();
        assert!(matches!(
            create(c, entry("   ", vec![])),
            Err(CoreError::Invalid("body"))
        ));
        let bad_mood = NewEntry {
            mood: Some(6),
            ..entry("x", vec![])
        };
        assert!(matches!(
            create(c, bad_mood),
            Err(CoreError::Invalid("mood"))
        ));
        assert!(matches!(
            create(c, entry("x", vec![Uuid::now_v7()])),
            Err(CoreError::NotFound)
        ));
        // Rien n'a été écrit à moitié.
        assert!(list(c, None, None, 10).unwrap().is_empty());
    }
}
