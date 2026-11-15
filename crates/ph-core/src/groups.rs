//! Groupes de discussion nommés : un nom, une photo facultative et des membres.
//! Leurs messages portent `message.group_id` ; supprimer le groupe les efface.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::members::{AvatarEdit, MAX_AVATAR_BYTES, MAX_NAME, data_url, image_mime};
use crate::{CoreError, Result, members};

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGroup {
    pub id: Uuid,
    pub name: String,
    /// Adresse `data:` prête pour `<img src>`.
    pub avatar: Option<String>,
    pub member_ids: Vec<Uuid>,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInput {
    pub name: String,
    pub member_ids: Vec<Uuid>,
    #[serde(default)]
    pub avatar: AvatarEdit,
}

/// Nom nettoyé, membres dédoublonnés, et image : `None` = inchangée,
/// `Some(None)` = retirée.
type Validated = (String, Vec<Uuid>, Option<Option<Vec<u8>>>);
type GroupRow = ([u8; 16], String, Option<Vec<u8>>, i64);

fn validated(input: &GroupInput) -> Result<Validated> {
    let name = input.name.trim().to_owned();
    if !(1..=MAX_NAME).contains(&name.chars().count()) {
        return Err(CoreError::Invalid("name"));
    }
    let mut ids: Vec<Uuid> = Vec::new();
    for id in &input.member_ids {
        if !ids.contains(id) {
            ids.push(*id);
        }
    }
    if ids.is_empty() {
        return Err(CoreError::Invalid("members"));
    }
    let avatar = match &input.avatar {
        AvatarEdit::Keep => None,
        AvatarEdit::Remove => Some(None),
        AvatarEdit::Set { data } => {
            let bytes = STANDARD
                .decode(data.as_bytes())
                .map_err(|_| CoreError::Invalid("avatar"))?;
            if bytes.len() > MAX_AVATAR_BYTES || image_mime(&bytes).is_none() {
                return Err(CoreError::Invalid("avatar"));
            }
            Some(Some(bytes))
        }
    };
    Ok((name, ids, avatar))
}

fn member_ids(conn: &Connection, id: &[u8; 16]) -> Result<Vec<Uuid>> {
    let mut st = conn.prepare_cached(
        "SELECT gm.member_id FROM chat_group_member gm JOIN member m ON m.id = gm.member_id
         WHERE gm.group_id = ?1 ORDER BY m.name COLLATE NOCASE",
    )?;
    let rows = st.query_map([id], |r| r.get::<_, [u8; 16]>(0))?;
    Ok(rows
        .map(|r| r.map(Uuid::from_bytes))
        .collect::<rusqlite::Result<_>>()?)
}

fn load(conn: &Connection, row: GroupRow) -> Result<ChatGroup> {
    let (id, name, avatar, created_at) = row;
    Ok(ChatGroup {
        id: Uuid::from_bytes(id),
        name,
        avatar: avatar.as_deref().and_then(data_url),
        member_ids: member_ids(conn, &id)?,
        created_at,
    })
}

pub fn get(conn: &Connection, id: Uuid) -> Result<ChatGroup> {
    let row = conn
        .query_row(
            "SELECT id, name, avatar, created_at FROM chat_group WHERE id = ?1",
            [id.as_bytes()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    load(conn, row)
}

pub fn list(conn: &Connection) -> Result<Vec<ChatGroup>> {
    let rows: Vec<GroupRow> = conn
        .prepare_cached("SELECT id, name, avatar, created_at FROM chat_group ORDER BY id")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    rows.into_iter().map(|row| load(conn, row)).collect()
}

fn write_members(conn: &Connection, id: Uuid, ids: &[Uuid]) -> Result<()> {
    // Seuls les membres retirés partent : les autres gardent leur ligne (et sa version de synchro).
    let current = member_ids(conn, id.as_bytes())?;
    let mut del = conn
        .prepare_cached("DELETE FROM chat_group_member WHERE group_id = ?1 AND member_id = ?2")?;
    for m in current.iter().filter(|m| !ids.contains(m)) {
        del.execute(params![id.as_bytes(), m.as_bytes()])?;
    }
    let mut ins = conn.prepare_cached(
        "INSERT OR IGNORE INTO chat_group_member (group_id, member_id) VALUES (?1, ?2)",
    )?;
    for m in ids {
        ins.execute(params![id.as_bytes(), m.as_bytes()])?;
    }
    Ok(())
}

pub fn create(conn: &mut Connection, input: &GroupInput) -> Result<ChatGroup> {
    let (name, ids, avatar) = validated(input)?;
    let tx = conn.transaction()?;
    members::ensure_active(&tx, &ids)?;
    let id = Uuid::now_v7();
    tx.execute(
        "INSERT INTO chat_group (id, name, avatar, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            id.as_bytes(),
            name,
            avatar.flatten(),
            crate::time::now_ms()?
        ],
    )?;
    write_members(&tx, id, &ids)?;
    let g = get(&tx, id)?;
    tx.commit()?;
    Ok(g)
}

pub fn update(conn: &mut Connection, id: Uuid, input: &GroupInput) -> Result<ChatGroup> {
    let (name, ids, avatar) = validated(input)?;
    let tx = conn.transaction()?;
    get(&tx, id)?;
    // Un membre archivé déjà présent peut rester ; seuls les ajouts doivent être actifs.
    let before = member_ids(&tx, id.as_bytes())?;
    let added: Vec<Uuid> = ids
        .iter()
        .copied()
        .filter(|m| !before.contains(m))
        .collect();
    members::ensure_active(&tx, &added)?;
    tx.execute(
        "UPDATE chat_group SET name = ?2 WHERE id = ?1",
        params![id.as_bytes(), name],
    )?;
    if let Some(avatar) = avatar {
        tx.execute(
            "UPDATE chat_group SET avatar = ?2 WHERE id = ?1",
            params![id.as_bytes(), avatar],
        )?;
    }
    write_members(&tx, id, &ids)?;
    let g = get(&tx, id)?;
    tx.commit()?;
    Ok(g)
}

/// Supprime le groupe et tous ses messages.
pub fn delete(conn: &Connection, id: Uuid) -> Result<()> {
    let n = conn.execute("DELETE FROM chat_group WHERE id = ?1", [id.as_bytes()])?;
    if n == 0 {
        Err(CoreError::NotFound)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::members::NewMember;
    use crate::messages::{self, NewMessage};

    fn db() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        crate::migrations::run(&mut c).unwrap();
        c
    }

    fn member(c: &Connection, name: &str) -> Uuid {
        members::create(
            c,
            NewMember {
                name: name.into(),
                pronouns: None,
                color: "#336699".into(),
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn group_lifecycle_and_messages() {
        let mut c = db();
        let ana = member(&c, "Ana");
        let bo = member(&c, "Bo");
        let zoe = member(&c, "Zoé");
        let g = create(
            &mut c,
            &GroupInput {
                name: "  Les protecteurs ".into(),
                member_ids: vec![bo, ana, ana],
                avatar: AvatarEdit::Keep,
            },
        )
        .unwrap();
        assert_eq!(g.name, "Les protecteurs");
        assert_eq!(g.member_ids, vec![ana, bo]);

        let first = messages::post(
            &mut c,
            &NewMessage {
                author_ids: vec![ana],
                recipient_ids: vec![bo],
                body: "salut".into(),
                group_id: Some(g.id),
                reply_to: None,
            },
        )
        .unwrap();
        let reply = messages::post(
            &mut c,
            &NewMessage {
                author_ids: vec![bo],
                recipient_ids: vec![ana],
                body: "coucou".into(),
                group_id: Some(g.id),
                reply_to: Some(first.id),
            },
        )
        .unwrap();
        assert_eq!(reply.group_id, Some(g.id));
        assert_eq!(reply.reply_to, Some(first.id));

        // Supprimer le message cité laisse la réponse, sans citation.
        messages::delete(&c, first.id).unwrap();
        let left = messages::list(&c, None, 50, false).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].reply_to, None);

        let g2 = update(
            &mut c,
            g.id,
            &GroupInput {
                name: "Protecteurs".into(),
                member_ids: vec![ana, zoe],
                avatar: AvatarEdit::Keep,
            },
        )
        .unwrap();
        assert_eq!(g2.member_ids, vec![ana, zoe]);
        assert_eq!(list(&c).unwrap().len(), 1);

        delete(&c, g.id).unwrap();
        assert!(messages::list(&c, None, 50, false).unwrap().is_empty());
        assert!(matches!(get(&c, g.id), Err(CoreError::NotFound)));
    }

    #[test]
    fn rejects_empty_group_and_unknown_group() {
        let mut c = db();
        let a = member(&c, "Ana");
        assert!(
            create(
                &mut c,
                &GroupInput {
                    name: "x".into(),
                    member_ids: vec![],
                    avatar: AvatarEdit::Keep,
                },
            )
            .is_err()
        );
        let err = messages::post(
            &mut c,
            &NewMessage {
                author_ids: vec![a],
                body: "?".into(),
                group_id: Some(Uuid::now_v7()),
                ..Default::default()
            },
        );
        assert!(matches!(err, Err(CoreError::NotFound)));
    }
}
