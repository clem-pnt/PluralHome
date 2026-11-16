//! Groupes de discussion nommés : un nom, une photo facultative et des membres.
//! Leurs messages portent `message.group_id` ; supprimer le groupe les efface.
//!
//! Un groupe peut être partagé avec un système ami (`friend_id`) : il réunit
//! alors des membres des deux côtés, et sa création ou sa modification part
//! chez l'ami. Le supprimer ne le supprime que chez soi.
//!
//! D'autres systèmes peuvent s'y joindre (`group_peer`) : amis directs, ou
//! systèmes connus seulement par un relais. Chacun relaie les messages et les
//! changements du groupe à ses voisins directs, si bien que deux systèmes qui
//! ne sont pas amis entre eux s'y parlent quand même.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::friends::{self, Envelope, PeerInfo, RemoteRef};
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
    /// Système ami avec qui le groupe est partagé.
    pub friend_id: Option<String>,
    /// Membres de l'ami ; vide (groupe partagé) : tout son système.
    pub remote_members: Vec<RemoteRef>,
    /// Les autres systèmes du groupe.
    pub peers: Vec<GroupPeer>,
}

/// Autre système d'un groupe : ami direct (`friend_id`) ou connu par un relais
/// (`name`).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupPeer {
    pub key: String,
    pub friend_id: Option<String>,
    pub name: Option<String>,
    /// Vide : tout son système.
    pub members: Vec<RemoteRef>,
}

/// Ami ajouté au groupe en plus du premier.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerInput {
    pub friend_id: String,
    /// Identifiants de sa fiche ; vide : tout son système.
    #[serde(default)]
    pub member_refs: Vec<String>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInput {
    pub name: String,
    pub member_ids: Vec<Uuid>,
    #[serde(default)]
    pub avatar: AvatarEdit,
    /// Fixé à la création, ignoré ensuite.
    #[serde(default)]
    pub friend_id: Option<String>,
    /// Membres de l'ami (identifiants de sa fiche) ; vide : tout son système.
    #[serde(default)]
    pub remote_member_refs: Vec<String>,
    /// Autres amis du groupe (remplacent les précédents à la modification).
    /// Sans `friend_id`, le premier devient l'ami principal.
    #[serde(default)]
    pub peers: Vec<PeerInput>,
}

/// Nom nettoyé, membres dédoublonnés, et image : `None` = inchangée,
/// `Some(None)` = retirée.
type Validated = (String, Vec<Uuid>, Option<Option<Vec<u8>>>);
type GroupRow = ([u8; 16], String, Option<Vec<u8>>, i64, Option<String>);
const COLUMNS: &str = "id, name, avatar, created_at, friend_id";

fn group_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<GroupRow> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
}

/// `shared` : groupe avec un ami, qui peut ne compter aucun de nos membres
/// (c'est alors tout le système qui y parle).
fn validated(input: &GroupInput, shared: bool) -> Result<Validated> {
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
    if ids.is_empty() && !shared {
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

fn remote_members(conn: &Connection, id: &[u8; 16]) -> Result<Vec<RemoteRef>> {
    let mut st = conn.prepare_cached(
        "SELECT ref, name, color FROM group_remote WHERE group_id = ?1 ORDER BY name COLLATE NOCASE",
    )?;
    let rows = st.query_map([id], |r| {
        Ok(RemoteRef {
            reference: r.get(0)?,
            name: r.get(1)?,
            color: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn load(conn: &Connection, row: GroupRow) -> Result<ChatGroup> {
    let (id, name, avatar, created_at, friend_id) = row;
    Ok(ChatGroup {
        id: Uuid::from_bytes(id),
        name,
        avatar: avatar.as_deref().and_then(data_url),
        member_ids: member_ids(conn, &id)?,
        created_at,
        remote_members: remote_members(conn, &id)?,
        peers: peers(conn, Uuid::from_bytes(id))?,
        friend_id,
    })
}

fn refs_json(refs: &[RemoteRef]) -> String {
    serde_json::to_string(refs).unwrap_or_else(|_| "[]".into())
}

fn refs_parse(json: &str) -> Vec<RemoteRef> {
    serde_json::from_str(json).unwrap_or_default()
}

fn peers(conn: &Connection, id: Uuid) -> Result<Vec<GroupPeer>> {
    let mut st = conn.prepare_cached(
        "SELECT key, friend_id, name, members FROM group_peer WHERE group_id = ?1
         ORDER BY friend_id IS NULL, coalesce(name, friend_id) COLLATE NOCASE, key",
    )?;
    let rows = st.query_map([id.as_bytes()], |r| {
        Ok(GroupPeer {
            key: r.get(0)?,
            friend_id: r.get(1)?,
            name: r.get(2)?,
            members: refs_parse(&r.get::<_, String>(3)?),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Amis avec qui le groupe est partagé directement : le principal puis les autres.
pub(crate) fn neighbors(conn: &Connection, id: Uuid) -> Result<Vec<String>> {
    let mut out: Vec<String> = conn
        .query_row(
            "SELECT friend_id FROM chat_group WHERE id = ?1",
            [id.as_bytes()],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
        .into_iter()
        .collect();
    let mut st = conn.prepare_cached(
        "SELECT friend_id FROM group_peer WHERE group_id = ?1 AND friend_id IS NOT NULL ORDER BY key",
    )?;
    for f in st.query_map([id.as_bytes()], |r| r.get::<_, String>(0))? {
        let f = f?;
        if !out.contains(&f) {
            out.push(f);
        }
    }
    Ok(out)
}

pub(crate) fn is_neighbor(conn: &Connection, id: Uuid, friend: &str) -> Result<bool> {
    Ok(neighbors(conn, id)?.iter().any(|f| f == friend))
}

pub(crate) fn is_direct_peer(conn: &Connection, id: Uuid, friend: &str) -> Result<bool> {
    Ok(conn
        .prepare_cached("SELECT 1 FROM group_peer WHERE group_id = ?1 AND friend_id = ?2")?
        .exists(params![id.as_bytes(), friend])?)
}

/// Membres d'un ami direct dans le groupe.
fn members_of(conn: &Connection, id: Uuid, friend: &str) -> Result<Vec<RemoteRef>> {
    let primary: Option<String> = conn
        .query_row(
            "SELECT friend_id FROM chat_group WHERE id = ?1",
            [id.as_bytes()],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    if primary.as_deref() == Some(friend) {
        return remote_members(conn, id.as_bytes());
    }
    let json: Option<String> = conn
        .query_row(
            "SELECT members FROM group_peer WHERE group_id = ?1 AND friend_id = ?2",
            params![id.as_bytes(), friend],
            |r| r.get(0),
        )
        .optional()?;
    Ok(json.as_deref().map(refs_parse).unwrap_or_default())
}

pub(crate) fn set_peer_members(
    conn: &Connection,
    id: Uuid,
    friend: &str,
    refs: &[RemoteRef],
) -> Result<()> {
    conn.execute(
        "UPDATE group_peer SET members = ?3 WHERE group_id = ?1 AND friend_id = ?2 AND members <> ?3",
        params![id.as_bytes(), friend, refs_json(refs)],
    )?;
    Ok(())
}

/// Ajoute (ou passe en direct) un ami du groupe.
fn put_direct_peer(conn: &Connection, id: Uuid, friend: &str, refs: &[RemoteRef]) -> Result<()> {
    let key = friends::friend_peer_key(conn, id, friend)?.ok_or(CoreError::NotFound)?;
    conn.execute(
        "INSERT INTO group_peer (group_id, key, friend_id, via, name, members)
         VALUES (?1, ?2, ?3, NULL, NULL, ?4)
         ON CONFLICT (group_id, key) DO UPDATE SET friend_id = excluded.friend_id, via = NULL,
             name = NULL, members = excluded.members
         WHERE group_peer.friend_id IS NOT excluded.friend_id OR group_peer.via IS NOT NULL
            OR group_peer.members <> excluded.members",
        params![id.as_bytes(), key, friend, refs_json(refs)],
    )?;
    Ok(())
}

const MAX_OTHERS: usize = 50;

/// Systèmes du groupe connus par `via` (remplacent ce qu'il en disait avant).
/// On s'y ignore soi-même, ainsi que nos amis directs du groupe.
pub(crate) fn write_others(
    conn: &Connection,
    id: Uuid,
    via: &str,
    others: Vec<PeerInfo>,
) -> Result<()> {
    let mut skip = vec![friends::my_peer_key(conn, id)?];
    for f in neighbors(conn, id)? {
        if let Some(k) = friends::friend_peer_key(conn, id, &f)? {
            skip.push(k);
        }
    }
    let mut keep: Vec<(String, String, String)> = Vec::new();
    for o in others {
        if keep.len() >= MAX_OTHERS {
            break;
        }
        if !friends::is_peer_key(&o.key)
            || skip.contains(&o.key)
            || keep.iter().any(|k| k.0 == o.key)
        {
            continue;
        }
        keep.push((
            o.key,
            friends::clean_name(&o.name),
            refs_json(&friends::clean_refs(o.members)),
        ));
    }
    // Seules les lignes qui changent sont réécrites (et synchronisées).
    let mut st = conn.prepare_cached(
        "SELECT key FROM group_peer WHERE group_id = ?1 AND friend_id IS NULL AND via = ?2",
    )?;
    let old: Vec<String> = st
        .query_map(params![id.as_bytes(), via], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut del = conn.prepare_cached("DELETE FROM group_peer WHERE group_id = ?1 AND key = ?2")?;
    for k in old.iter().filter(|k| !keep.iter().any(|n| &n.0 == *k)) {
        del.execute(params![id.as_bytes(), k])?;
    }
    let mut ins = conn.prepare_cached(
        "INSERT INTO group_peer (group_id, key, friend_id, via, name, members)
         VALUES (?1, ?2, NULL, ?3, ?4, ?5)
         ON CONFLICT (group_id, key) DO UPDATE SET name = excluded.name, members = excluded.members
         WHERE group_peer.friend_id IS NULL AND group_peer.via = excluded.via
           AND (group_peer.name IS NOT excluded.name OR group_peer.members <> excluded.members)",
    )?;
    for (key, name, members) in &keep {
        ins.execute(params![id.as_bytes(), key, via, name, members])?;
    }
    Ok(())
}

/// Relaie une enveloppe du groupe à nos voisins directs, sauf `except`.
/// Un voisin qui n'accepte plus nos messages est sauté.
pub(crate) fn forward(conn: &Connection, id: Uuid, except: &str, env: &Envelope) -> Result<()> {
    for f in neighbors(conn, id)? {
        if f != except && friends::ensure_writable(conn, &f).is_ok() {
            friends::queue(conn, &f, env.clone())?;
        }
    }
    Ok(())
}

/// Envoie le groupe à chacun de nos voisins directs (sauf `except`) : nos
/// membres, les siens tels qu'on les a choisis, et les autres systèmes.
/// Un voisin de `strict` doit accepter nos messages ; les autres sont sautés.
pub(crate) fn share_all(
    conn: &Connection,
    id: Uuid,
    except: Option<&str>,
    strict: &[String],
) -> Result<()> {
    let g = get(conn, id)?;
    let around = neighbors(conn, id)?;
    let mine = friends::own_refs(conn, &g.member_ids)?;
    for n in around.iter().filter(|n| Some(n.as_str()) != except) {
        if let Err(e) = friends::ensure_writable(conn, n) {
            if strict.contains(n) {
                return Err(e);
            }
            continue;
        }
        let yours = members_of(conn, id, n)?
            .iter()
            .filter_map(|r| Uuid::try_parse(&r.reference).ok())
            .collect();
        let mut others = Vec::new();
        for d in around.iter().filter(|d| *d != n) {
            if let Some(key) = friends::friend_peer_key(conn, id, d)? {
                others.push(PeerInfo {
                    key,
                    name: friends::card_name(conn, d)?,
                    members: members_of(conn, id, d)?,
                });
            }
        }
        let mut st = conn.prepare_cached(
            "SELECT key, name, members FROM group_peer
             WHERE group_id = ?1 AND friend_id IS NULL AND via IS NOT ?2",
        )?;
        let far = st.query_map(params![id.as_bytes(), n], |r| {
            Ok(PeerInfo {
                key: r.get(0)?,
                name: r.get::<_, Option<String>>(1)?.unwrap_or_else(|| "?".into()),
                members: refs_parse(&r.get::<_, String>(2)?),
            })
        })?;
        for p in far {
            others.push(p?);
        }
        friends::queue(
            conn,
            n,
            Envelope::Group {
                id,
                name: g.name.clone(),
                mine: mine.clone(),
                yours,
                others,
            },
        )?;
    }
    Ok(())
}

pub fn get(conn: &Connection, id: Uuid) -> Result<ChatGroup> {
    let row = conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM chat_group WHERE id = ?1"),
            [id.as_bytes()],
            group_row,
        )
        .optional()?
        .ok_or(CoreError::NotFound)?;
    load(conn, row)
}

pub fn list(conn: &Connection) -> Result<Vec<ChatGroup>> {
    let rows: Vec<GroupRow> = conn
        .prepare_cached(&format!("SELECT {COLUMNS} FROM chat_group ORDER BY id"))?
        .query_map([], group_row)?
        .collect::<rusqlite::Result<_>>()?;
    rows.into_iter().map(|row| load(conn, row)).collect()
}

pub(crate) fn write_members(conn: &Connection, id: Uuid, ids: &[Uuid]) -> Result<()> {
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

pub(crate) fn write_remote(conn: &Connection, id: Uuid, refs: &[RemoteRef]) -> Result<()> {
    conn.execute(
        "DELETE FROM group_remote WHERE group_id = ?1",
        [id.as_bytes()],
    )?;
    let mut st = conn.prepare_cached(
        "INSERT OR IGNORE INTO group_remote (group_id, ref, name, color) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for r in refs {
        st.execute(params![id.as_bytes(), r.reference, r.name, r.color])?;
    }
    Ok(())
}

/// Membres de l'ami principal et autres amis directs, d'après leurs fiches.
/// Renvoie les amis qui doivent accepter nos messages.
fn set_friends(
    conn: &Connection,
    id: Uuid,
    primary: &str,
    refs: &[String],
    peers: &[PeerInput],
) -> Result<Vec<String>> {
    write_remote(conn, id, &friends::friend_refs(conn, primary, refs)?)?;
    let mut strict = vec![primary.to_owned()];
    for p in peers {
        if strict.contains(&p.friend_id) {
            continue;
        }
        if friends::friend_pub(conn, &p.friend_id)?.is_none() {
            return Err(CoreError::NotFound);
        }
        let remote = friends::friend_refs(conn, &p.friend_id, &p.member_refs)?;
        put_direct_peer(conn, id, &p.friend_id, &remote)?;
        strict.push(p.friend_id.clone());
    }
    // Amis retirés : eux et ce qu'ils nous avaient appris du groupe.
    let mut st =
        conn.prepare_cached("SELECT key, friend_id, via FROM group_peer WHERE group_id = ?1")?;
    let rows: Vec<(String, Option<String>, Option<String>)> = st
        .query_map([id.as_bytes()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut del = conn.prepare_cached("DELETE FROM group_peer WHERE group_id = ?1 AND key = ?2")?;
    for (key, friend, via) in rows {
        let gone = match (friend, via) {
            (Some(f), _) => !strict.contains(&f),
            (None, Some(v)) => !strict.contains(&v),
            (None, None) => true,
        };
        if gone {
            del.execute(params![id.as_bytes(), key])?;
        }
    }
    Ok(strict)
}

pub fn create(conn: &mut Connection, input: &GroupInput) -> Result<ChatGroup> {
    create_from(conn, input, &[])
}

/// Crée le groupe et y range des messages déjà échangés : une discussion
/// privée devient ainsi un groupe qu'on peut nommer, illustrer et compléter,
/// sans perdre son historique. Seuls les messages hors groupe et du même
/// échange (même ami, ou aucun) sont repris ; les autres sont ignorés.
pub fn create_from(
    conn: &mut Connection,
    input: &GroupInput,
    message_ids: &[Uuid],
) -> Result<ChatGroup> {
    // Sans ami principal, le premier ami ajouté le devient.
    let (friend, refs, peers): (Option<&str>, &[String], &[PeerInput]) =
        match (input.friend_id.as_deref(), input.peers.split_first()) {
            (Some(f), _) => (Some(f), &input.remote_member_refs, &input.peers),
            (None, Some((first, rest))) if input.remote_member_refs.is_empty() => {
                (Some(first.friend_id.as_str()), &first.member_refs, rest)
            }
            (None, None) if input.remote_member_refs.is_empty() => (None, &[], &[]),
            (None, _) => return Err(CoreError::Invalid("members")),
        };
    let (name, ids, avatar) = validated(input, friend.is_some())?;
    let tx = conn.transaction()?;
    members::ensure_active(&tx, &ids)?;
    let id = Uuid::now_v7();
    tx.execute(
        "INSERT INTO chat_group (id, name, avatar, created_at, friend_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id.as_bytes(),
            name,
            avatar.flatten(),
            crate::time::now_ms()?,
            friend,
        ],
    )?;
    write_members(&tx, id, &ids)?;
    if let Some(f) = friend {
        let strict = set_friends(&tx, id, f, refs, peers)?;
        share_all(&tx, id, None, &strict)?;
    }
    let mut adopt = tx.prepare_cached(
        "UPDATE message SET group_id = ?1
         WHERE id = ?2 AND group_id IS NULL AND friend_id IS ?3",
    )?;
    for m in message_ids {
        adopt.execute(params![id.as_bytes(), m.as_bytes(), friend])?;
    }
    drop(adopt);
    let g = get(&tx, id)?;
    tx.commit()?;
    Ok(g)
}

pub fn update(conn: &mut Connection, id: Uuid, input: &GroupInput) -> Result<ChatGroup> {
    let tx = conn.transaction()?;
    let friend = get(&tx, id)?.friend_id;
    let (name, ids, avatar) = validated(input, friend.is_some())?;
    if friend.is_none() && (!input.remote_member_refs.is_empty() || !input.peers.is_empty()) {
        return Err(CoreError::Invalid("members"));
    }
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
    if let Some(f) = friend.as_deref() {
        // Les amis déjà là qui n'acceptent plus nos messages ne bloquent pas la modification.
        let before: Vec<String> = neighbors(&tx, id)?;
        let strict = set_friends(&tx, id, f, &input.remote_member_refs, &input.peers)?;
        let strict: Vec<String> = strict
            .into_iter()
            .filter(|s| s == f || !before.contains(s))
            .collect();
        share_all(&tx, id, None, &strict)?;
    }
    let g = get(&tx, id)?;
    tx.commit()?;
    Ok(g)
}

/// Supprime le groupe et tous ses messages (chez soi seulement).
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
                friend_id: None,
                remote_member_refs: vec![],
                peers: vec![],
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
                ..Default::default()
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
                ..Default::default()
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
                friend_id: None,
                remote_member_refs: vec![],
                peers: vec![],
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
    fn conversation_becomes_group_with_its_history() {
        let mut c = db();
        let ana = member(&c, "Ana");
        let bo = member(&c, "Bo");
        let post = |c: &mut Connection, group_id| {
            messages::post(
                c,
                &NewMessage {
                    author_ids: vec![ana],
                    recipient_ids: vec![bo],
                    body: "salut".into(),
                    group_id,
                    ..Default::default()
                },
            )
            .unwrap()
            .id
        };
        let old = post(&mut c, None);
        let other = create(
            &mut c,
            &GroupInput {
                name: "Autre".into(),
                member_ids: vec![ana],
                ..Default::default()
            },
        )
        .unwrap();
        let elsewhere = post(&mut c, Some(other.id));
        let g = create_from(
            &mut c,
            &GroupInput {
                name: "Ana et Bo".into(),
                member_ids: vec![ana, bo],
                ..Default::default()
            },
            &[old, elsewhere, Uuid::now_v7()],
        )
        .unwrap();
        let all = messages::list(&c, None, 50, false).unwrap();
        let group_of = |id| all.iter().find(|m| m.id == id).unwrap().group_id;
        assert_eq!(group_of(old), Some(g.id));
        // Un message déjà dans un autre groupe y reste.
        assert_eq!(group_of(elsewhere), Some(other.id));
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
                    friend_id: None,
                    remote_member_refs: vec![],
                    peers: vec![],
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
