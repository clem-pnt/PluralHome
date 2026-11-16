//! Messagerie interne et « depuis ton dernier passage ».
//!
//! Les messages sont un tableau partagé : adressés à un membre ou à tout le
//! système. L'état de lecture est suivi par membre, pour qu'au retour au front
//! chacun retrouve ce qui lui a été laissé.
//!
//! Un message peut aussi s'échanger avec un système ami (`friend_id`) : ses
//! membres y figurent par leur nom et leur couleur (`message_remote`), et
//! l'envoi, la modification ou la suppression partent chez l'ami chiffrés.

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::friends::{self, Envelope, RemoteRef, SystemTag};
use crate::front::{self, FrontSession};
use crate::{CoreError, Result, members};

pub const MAX_BODY_CHARS: usize = 4000;
const PAGE_MAX: u32 = 200;

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: Uuid,
    /// Vide : auteur inconnu ou supprimé.
    pub author_ids: Vec<Uuid>,
    /// Vide : pour tout le système.
    pub recipient_ids: Vec<Uuid>,
    pub body: String,
    pub pinned: bool,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub edited_at: Option<i64>,
    /// Membres qui l'ont marqué comme lu.
    pub read_by: Vec<Uuid>,
    /// Groupe de discussion nommé ; `None` : conversation entre ses participants.
    pub group_id: Option<Uuid>,
    /// Message auquel celui-ci répond (`None` s'il a été supprimé depuis).
    pub reply_to: Option<Uuid>,
    /// Conversation avec un système ami.
    pub friend_id: Option<String>,
    /// Envoyé par nous (avec un ami) ; `false` : reçu de l'ami.
    pub outgoing: bool,
    /// Signé par le système entier plutôt que par des membres.
    pub by_system: bool,
    /// Auteurs chez l'ami (message reçu).
    pub remote_authors: Vec<RemoteRef>,
    /// Destinataires chez l'ami (message envoyé) ; vide : tout son système.
    pub remote_recipients: Vec<RemoteRef>,
    /// Groupe à plusieurs systèmes : système d'origine, s'il n'est pas l'ami
    /// qui nous l'a transmis.
    pub system: Option<SystemTag>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewMessage {
    pub author_ids: Vec<Uuid>,
    /// Vide : pour tout le système.
    pub recipient_ids: Vec<Uuid>,
    pub body: String,
    /// Fixés à l'envoi, ignorés à la modification.
    #[serde(default)]
    pub group_id: Option<Uuid>,
    #[serde(default)]
    pub reply_to: Option<Uuid>,
    /// Système ami destinataire ; pris du groupe s'il est partagé.
    #[serde(default)]
    pub friend_id: Option<String>,
    /// Membres de l'ami (identifiants de sa fiche) ; vide : tout son système.
    #[serde(default)]
    pub remote_recipient_refs: Vec<String>,
    /// Signé par tout le système (`author_ids` doit être vide).
    #[serde(default)]
    pub by_system: bool,
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

// Auteurs et destinataires vivent dans `message_author` / `message_recipient`.
// Les colonnes historiques `author_id` / `recipient_id` sont encore lues, au cas
// où un appareil pas encore à jour écrirait un message à l'ancienne.
const AUTHORS: &str = "SELECT member_id FROM message_author WHERE message_id = m.id
     UNION SELECT m.author_id WHERE m.author_id IS NOT NULL";
const RECIPIENTS: &str = "SELECT member_id FROM message_recipient WHERE message_id = m.id
     UNION SELECT m.recipient_id WHERE m.recipient_id IS NOT NULL";

fn columns() -> String {
    format!(
        "m.id, m.body, m.pinned, m.created_at, m.edited_at,
         (SELECT group_concat(hex(x.member_id)) FROM ({AUTHORS}) x),
         (SELECT group_concat(hex(x.member_id)) FROM ({RECIPIENTS}) x),
         (SELECT group_concat(hex(member_id)) FROM message_read WHERE message_id = m.id),
         m.group_id, m.reply_to, m.friend_id, m.outgoing, m.by_system,
         (SELECT json_group_array(json_object('ref', ref, 'name', name, 'color', color))
            FROM message_remote WHERE message_id = m.id AND role = 0),
         (SELECT json_group_array(json_object('ref', ref, 'name', name, 'color', color))
            FROM message_remote WHERE message_id = m.id AND role = 1),
         (SELECT key FROM message_system WHERE message_id = m.id),
         (SELECT name FROM message_system WHERE message_id = m.id)"
    )
}

fn refs(json: Option<String>) -> Vec<RemoteRef> {
    let mut out: Vec<RemoteRef> = json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

fn ids(list: Option<String>) -> Vec<Uuid> {
    let mut out: Vec<Uuid> = list
        .unwrap_or_default()
        .split(',')
        .filter_map(|h| Uuid::try_parse(h).ok())
        .collect();
    out.sort_unstable();
    out
}

fn from_row(r: &Row<'_>) -> rusqlite::Result<Message> {
    let id: [u8; 16] = r.get(0)?;
    Ok(Message {
        id: Uuid::from_bytes(id),
        body: r.get(1)?,
        pinned: r.get(2)?,
        created_at: r.get(3)?,
        edited_at: r.get(4)?,
        author_ids: ids(r.get(5)?),
        recipient_ids: ids(r.get(6)?),
        read_by: ids(r.get(7)?),
        group_id: r.get::<_, Option<[u8; 16]>>(8)?.map(Uuid::from_bytes),
        reply_to: r.get::<_, Option<[u8; 16]>>(9)?.map(Uuid::from_bytes),
        friend_id: r.get(10)?,
        outgoing: r.get(11)?,
        by_system: r.get(12)?,
        remote_authors: refs(r.get(13)?),
        remote_recipients: refs(r.get(14)?),
        system: match (
            r.get::<_, Option<String>>(15)?,
            r.get::<_, Option<String>>(16)?,
        ) {
            (Some(key), Some(name)) => Some(SystemTag { key, name }),
            _ => None,
        },
    })
}

fn get(conn: &Connection, id: Uuid) -> Result<Message> {
    conn.query_row(
        &format!("SELECT {} FROM message m WHERE m.id = ?1", columns()),
        [id.as_bytes()],
        from_row,
    )
    .optional()?
    .ok_or(CoreError::NotFound)
}

fn dedup(list: &[Uuid]) -> Vec<Uuid> {
    let mut out = Vec::with_capacity(list.len());
    for u in list {
        if !out.contains(u) {
            out.push(*u);
        }
    }
    out
}

/// Valide le texte et les personnes ; renvoie le texte nettoyé.
fn check(conn: &Connection, input: &NewMessage) -> Result<String> {
    // On garde le texte tel quel (mise en forme voulue), seuls les blancs aux bords partent.
    let body = input.body.trim().to_owned();
    if body.is_empty() || body.chars().count() > MAX_BODY_CHARS {
        return Err(CoreError::Invalid("body"));
    }
    let people: Vec<Uuid> = input
        .author_ids
        .iter()
        .chain(&input.recipient_ids)
        .copied()
        .collect();
    members::ensure_active(conn, &people)?;
    if input.by_system && !input.author_ids.is_empty() {
        return Err(CoreError::Invalid("authors"));
    }
    Ok(body)
}

/// Ami concerné par un nouveau message : celui du groupe, ou celui demandé.
/// Avec un ami, les destinataires sont chez lui ; sans, il n'y en a pas chez lui.
fn friend_of(conn: &Connection, input: &NewMessage) -> Result<Option<String>> {
    let from_group = match input.group_id {
        Some(g) => crate::groups::get(conn, g)?.friend_id,
        None => None,
    };
    let friend = match (from_group, &input.friend_id) {
        (Some(g), Some(f)) if &g != f => return Err(CoreError::Invalid("friend")),
        (Some(g), _) => Some(g),
        (None, Some(_)) if input.group_id.is_some() => return Err(CoreError::Invalid("friend")),
        (None, f) => f.clone(),
    };
    match &friend {
        Some(_) if !input.recipient_ids.is_empty() => Err(CoreError::Invalid("recipients")),
        None if !input.remote_recipient_refs.is_empty() || input.by_system => {
            Err(CoreError::Invalid("recipients"))
        }
        _ => Ok(friend),
    }
}

fn friend_and_dir(conn: &Connection, id: Uuid) -> Result<(Option<String>, bool, Option<Uuid>)> {
    conn.query_row(
        "SELECT friend_id, outgoing, group_id FROM message WHERE id = ?1",
        [id.as_bytes()],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get::<_, Option<[u8; 16]>>(2)?.map(Uuid::from_bytes),
            ))
        },
    )
    .optional()?
    .ok_or(CoreError::NotFound)
}

/// Groupe partagé avec plusieurs systèmes : les destinataires y sont tout le monde.
fn multi_system(conn: &Connection, group: Option<Uuid>) -> Result<bool> {
    Ok(match group {
        Some(g) => conn
            .prepare_cached("SELECT 1 FROM group_peer WHERE group_id = ?1")?
            .exists([g.as_bytes()])?,
        None => false,
    })
}

/// Destinataires chez l'ami : validés d'après sa fiche, enregistrés, et
/// renvoyés sous forme d'identifiants pour l'enveloppe.
fn remote_to(conn: &Connection, id: Uuid, friend: &str, refs: &[String]) -> Result<Vec<Uuid>> {
    friends::ensure_writable(conn, friend)?;
    let remote = friends::friend_refs(conn, friend, refs)?;
    friends::write_message_remote(conn, id, 1, &remote)?;
    remote
        .iter()
        .map(|r| Uuid::try_parse(&r.reference).map_err(|_| CoreError::Invalid("recipients")))
        .collect()
}

fn write_people(conn: &Connection, id: Uuid, input: &NewMessage) -> Result<()> {
    let id = id.as_bytes();
    conn.execute("DELETE FROM message_author WHERE message_id = ?1", [id])?;
    conn.execute("DELETE FROM message_recipient WHERE message_id = ?1", [id])?;
    let mut st =
        conn.prepare_cached("INSERT INTO message_author (message_id, member_id) VALUES (?1, ?2)")?;
    for a in dedup(&input.author_ids) {
        st.execute(params![id, a.as_bytes()])?;
    }
    let mut st = conn
        .prepare_cached("INSERT INTO message_recipient (message_id, member_id) VALUES (?1, ?2)")?;
    for r in dedup(&input.recipient_ids) {
        st.execute(params![id, r.as_bytes()])?;
    }
    Ok(())
}

pub fn post(conn: &mut Connection, input: &NewMessage) -> Result<Message> {
    let tx = conn.transaction()?;
    let body = check(&tx, input)?;
    let friend = friend_of(&tx, input)?;
    if let Some(parent) = input.reply_to {
        let p = get(&tx, parent)?;
        // Dans un groupe, le message cité a pu passer par un autre ami.
        let ok = match input.group_id {
            Some(_) => p.group_id == input.group_id,
            None => p.group_id.is_none() && p.friend_id == friend,
        };
        if !ok {
            return Err(CoreError::Invalid("reply_to"));
        }
    }
    if multi_system(&tx, input.group_id)? && !input.remote_recipient_refs.is_empty() {
        return Err(CoreError::Invalid("recipients"));
    }
    let id = Uuid::now_v7();
    let now = crate::time::now_ms()?;
    // `author_id` garde le premier auteur pour les appareils pas encore à jour ;
    // `recipient_id` reste vide : sinon supprimer ce destinataire effacerait le message.
    tx.execute(
        "INSERT INTO message (id, author_id, recipient_id, body, created_at, group_id, reply_to,
                              friend_id, outgoing, by_system)
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id.as_bytes(),
            input.author_ids.first().map(|u| *u.as_bytes()),
            body,
            now,
            input.group_id.map(|u| *u.as_bytes()),
            input.reply_to.map(|u| *u.as_bytes()),
            friend,
            friend.is_some(),
            input.by_system,
        ],
    )?;
    write_people(&tx, id, input)?;
    if let Some(f) = friend.as_deref() {
        let to = remote_to(&tx, id, f, &input.remote_recipient_refs)?;
        let authors = friends::own_refs(&tx, &dedup(&input.author_ids))?;
        let env = Envelope::Msg {
            id,
            authors,
            by_system: input.by_system,
            to,
            body: body.clone(),
            at: now,
            group: input.group_id,
            reply: input.reply_to,
            from: None,
        };
        friends::queue(&tx, f, env.clone())?;
        if let Some(g) = input.group_id {
            crate::groups::forward(&tx, g, f, &env)?;
        }
    }
    let m = get(&tx, id)?;
    tx.commit()?;
    Ok(m)
}

/// Modifie le texte, les auteurs et les destinataires d'un message.
pub fn update(conn: &mut Connection, id: Uuid, input: &NewMessage) -> Result<Message> {
    let tx = conn.transaction()?;
    let body = check(&tx, input)?;
    let (friend, outgoing, group) = friend_and_dir(&tx, id)?;
    if friend.is_some() && !outgoing {
        return Err(CoreError::Invalid("incoming"));
    }
    if friend.is_some() && !input.recipient_ids.is_empty()
        || friend.is_none() && (!input.remote_recipient_refs.is_empty() || input.by_system)
        || multi_system(&tx, group)? && !input.remote_recipient_refs.is_empty()
    {
        return Err(CoreError::Invalid("recipients"));
    }
    let now = crate::time::now_ms()?;
    tx.execute(
        "UPDATE message SET body = ?2, author_id = ?3, recipient_id = NULL, edited_at = ?4,
                            by_system = ?5
         WHERE id = ?1",
        params![
            id.as_bytes(),
            body,
            input.author_ids.first().map(|u| *u.as_bytes()),
            now,
            input.by_system,
        ],
    )?;
    write_people(&tx, id, input)?;
    if let Some(f) = friend.as_deref() {
        let to = remote_to(&tx, id, f, &input.remote_recipient_refs)?;
        let authors = friends::own_refs(&tx, &dedup(&input.author_ids))?;
        let env = Envelope::Edit {
            id,
            authors,
            by_system: input.by_system,
            to,
            body: body.clone(),
            at: now,
        };
        friends::queue(&tx, f, env.clone())?;
        if let Some(g) = group {
            crate::groups::forward(&tx, g, f, &env)?;
        }
    }
    let m = get(&tx, id)?;
    tx.commit()?;
    Ok(m)
}

/// Tableau : épinglés d'abord, puis du plus récent au plus ancien.
/// Pagination par curseur sur l'identifiant (`UUIDv7`, strictement croissant dans le
/// temps) : contrairement à l'horodatage, il ne connaît pas d'égalité, donc rien n'est sauté.
///
/// `unread_only` : seulement les messages qu'un destinataire n'a pas encore lus
/// (pour un message à tout le système : que personne n'a lu). Les épinglés restent.
pub fn list(
    conn: &Connection,
    before: Option<Uuid>,
    limit: u32,
    unread_only: bool,
) -> Result<Vec<Message>> {
    let limit = limit.clamp(1, PAGE_MAX);
    let mut st = conn.prepare_cached(&format!(
        "SELECT {} FROM message m
         WHERE (?1 IS NULL OR (m.id < ?1 AND m.pinned = 0))
           AND (?3 = 0 OR m.pinned = 1
                OR m.outgoing = 0
                   AND (EXISTS (SELECT 1 FROM ({RECIPIENTS}) x
                                WHERE x.member_id NOT IN ({AUTHORS})
                                  AND NOT EXISTS (SELECT 1 FROM message_read r
                                                  WHERE r.message_id = m.id AND r.member_id = x.member_id))
                        OR (NOT EXISTS ({RECIPIENTS})
                            AND NOT EXISTS (SELECT 1 FROM message_read r WHERE r.message_id = m.id))))
         ORDER BY m.pinned DESC, m.id DESC
         LIMIT ?2",
        columns()
    ))?;
    Ok(st
        .query_map(
            params![before.map(|u| *u.as_bytes()), limit, unread_only],
            from_row,
        )?
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

/// Supprime un message. Envoyé à un ami encore présent, il est aussi retiré
/// chez lui ; reçu d'un ami, il ne disparaît que chez soi.
pub fn delete(conn: &Connection, id: Uuid) -> Result<()> {
    let (friend, outgoing, group) = friend_and_dir(conn, id)?;
    if let Some(f) = friend.as_deref().filter(|_| outgoing) {
        if friends::get(conn, f).is_ok() {
            friends::queue(conn, f, Envelope::Del { id })?;
        }
        if let Some(g) = group {
            crate::groups::forward(conn, g, f, &Envelope::Del { id })?;
        }
    }
    conn.execute("DELETE FROM message WHERE id = ?1", [id.as_bytes()])?;
    Ok(())
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

    // Retour au front en cours : ce qui commence après, il l'a vécu en étant là.
    let back_at: Option<i64> = conn.query_row(
        "SELECT min(started_at) FROM front_session WHERE member_id = ?1 AND ended_at IS NULL",
        [member_id.as_bytes()],
        |r| r.get(0),
    )?;

    // Un accusé postérieur au dernier passage masque ce qui a déjà été vu ;
    // seuls les passages commencés depuis restent à rattraper.
    let fronts = match last_seen {
        Some(since) => {
            let seen_until = acked_at.filter(|a| *a >= since);
            front::history(conn, since, i64::MAX)?
                .into_iter()
                .filter(|s| s.member_id != Some(member_id))
                .filter(|s| seen_until.is_none_or(|a| s.started_at > a))
                .filter(|s| back_at.is_none_or(|b| s.started_at < b))
                .collect()
        }
        None => Vec::new(),
    };

    let mut st = conn.prepare_cached(&format!(
        "SELECT {} FROM message m
         WHERE (NOT EXISTS ({RECIPIENTS}) OR ?1 IN ({RECIPIENTS}))
           AND ?1 NOT IN ({AUTHORS})
           AND m.outgoing = 0
           AND NOT EXISTS (SELECT 1 FROM message_read r WHERE r.message_id = m.id AND r.member_id = ?1)
         ORDER BY m.id",
        columns()
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
    insert_reads(&tx, &[member_id], ids, now)?;
    tx.execute(
        "INSERT INTO catchup_ack (member_id, acked_at) VALUES (?1, ?2)
         ON CONFLICT (member_id) DO UPDATE SET acked_at = max(acked_at, excluded.acked_at)",
        params![member_id.as_bytes(), now],
    )?;
    tx.commit()?;
    Ok(())
}

/// Marque un message comme lu par plusieurs membres, depuis le tableau
/// (sans toucher au « depuis ton dernier passage »).
pub fn mark_read_by(conn: &mut Connection, id: Uuid, members_ids: &[Uuid]) -> Result<Message> {
    let now = crate::time::now_ms()?;
    let tx = conn.transaction()?;
    members::ensure_active(&tx, members_ids)?;
    get(&tx, id)?;
    insert_reads(&tx, members_ids, &[id], now)?;
    let m = get(&tx, id)?;
    tx.commit()?;
    Ok(m)
}

fn insert_reads(conn: &Connection, members_ids: &[Uuid], ids: &[Uuid], now: i64) -> Result<()> {
    let mut st = conn.prepare_cached(
        "INSERT OR IGNORE INTO message_read (message_id, member_id, read_at)
         SELECT id, ?2, ?3 FROM message WHERE id = ?1",
    )?;
    for m in members_ids {
        for id in ids {
            st.execute(params![id.as_bytes(), m.as_bytes(), now])?;
        }
    }
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
        v.create().unwrap();
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
            author_ids: author.into_iter().collect(),
            recipient_ids: to.into_iter().collect(),
            body: body.into(),
            ..Default::default()
        }
    }

    #[test]
    fn post_validates() {
        let (_t, mut v, a, _) = setup();
        let c = v.conn_mut().unwrap();
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
        let (_t, mut v, a, _) = setup();
        let c = v.conn_mut().unwrap();
        let first = post(c, &msg(Some(a), None, "un")).unwrap();
        for body in ["deux", "trois"] {
            post(c, &msg(Some(a), None, body)).unwrap();
        }
        set_pinned(c, first.id, true).unwrap();
        let all = list(c, None, 50, false).unwrap();
        assert_eq!(all[0].id, first.id);
        assert_eq!(all.len(), 3);
        let page = list(c, None, 2, false).unwrap();
        assert_eq!(page.len(), 2);
        let next = list(c, Some(page[1].id), 2, false).unwrap();
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].body, "deux");
        delete(c, first.id).unwrap();
        assert!(matches!(delete(c, first.id), Err(CoreError::NotFound)));
    }

    #[test]
    fn several_people_edit_and_unread() {
        let (_t, mut v, mila, sacha) = setup();
        let c = v.conn_mut().unwrap();
        let both = NewMessage {
            author_ids: vec![mila, sacha, mila],
            recipient_ids: vec![],
            body: "à tous".into(),
            ..Default::default()
        };
        let shared = post(c, &both).unwrap();
        let mut authors = vec![mila, sacha];
        authors.sort_unstable();
        assert_eq!(shared.author_ids, authors);
        assert!(shared.recipient_ids.is_empty() && shared.edited_at.is_none());

        let to_both = post(
            c,
            &NewMessage {
                author_ids: vec![],
                recipient_ids: vec![mila, sacha],
                body: "pour vous deux".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(list(c, None, 50, true).unwrap().len(), 2);

        // Écrit à deux et adressé à soi-même : aucun des auteurs n'a à le lire.
        let between = post(
            c,
            &NewMessage {
                author_ids: vec![mila, sacha],
                recipient_ids: vec![mila, sacha],
                body: "entre nous".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(list(c, None, 50, true).unwrap().len(), 2);
        for who in [mila, sacha] {
            assert!(
                catch_up(c, who)
                    .unwrap()
                    .unread
                    .iter()
                    .all(|m| m.id != between.id)
            );
        }
        delete(c, between.id).unwrap();

        // Un message à plusieurs reste non lu tant que tous ne l'ont pas lu.
        let read = mark_read_by(c, to_both.id, &[mila]).unwrap();
        assert_eq!(read.read_by, vec![mila]);
        assert_eq!(list(c, None, 50, true).unwrap().len(), 2);
        mark_read_by(c, to_both.id, &[sacha]).unwrap();
        // Un message à tout le système est lu dès que quelqu'un l'a lu.
        mark_read_by(c, shared.id, &[sacha]).unwrap();
        assert!(list(c, None, 50, true).unwrap().is_empty());
        assert_eq!(list(c, None, 50, false).unwrap().len(), 2);

        let edited = update(
            c,
            shared.id,
            &NewMessage {
                author_ids: vec![sacha],
                recipient_ids: vec![mila],
                body: "corrigé".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            (edited.author_ids, edited.recipient_ids),
            (vec![sacha], vec![mila])
        );
        assert_eq!(edited.body, "corrigé");
        assert!(edited.edited_at.is_some());
        assert!(matches!(
            update(c, Uuid::now_v7(), &both),
            Err(CoreError::NotFound)
        ));

        // Supprimer un destinataire ne supprime pas le message.
        c.execute("DELETE FROM member WHERE id = ?1", [mila.as_bytes()])
            .unwrap();
        let after = list(c, None, 50, false).unwrap();
        assert_eq!(after.len(), 2);
        assert!(after.iter().all(|x| !x.recipient_ids.contains(&mila)));
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
                statuses: std::collections::HashMap::new(),
            },
        )
        .unwrap();
        front::change(
            c,
            &FrontChange {
                members: vec![sacha],
                blurry: false,
                statuses: std::collections::HashMap::new(),
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
                .all(|m| !m.author_ids.contains(&sacha))
        );
    }

    #[test]
    fn arrivals_while_present_are_not_missed() {
        let (_t, mut v, mila, sacha) = setup();
        let c = v.conn_mut().unwrap();
        let set = |c: &mut rusqlite::Connection, members: Vec<Uuid>, blurry: bool, at: i64| {
            let ch = FrontChange {
                members,
                blurry,
                statuses: std::collections::HashMap::new(),
            };
            front::change_at(c, &ch, at, front::Origin::Remote).unwrap();
        };
        set(c, vec![mila], false, 1_000);
        set(c, vec![sacha], false, 2_000); // Mila part
        set(c, vec![mila, sacha], false, 3_000); // Mila revient
        set(c, vec![mila, sacha], true, 4_000); // du flou arrive, Mila est là
        let cu = catch_up(c, mila).unwrap();
        let who: Vec<_> = cu.fronts.iter().map(|s| s.member_id).collect();
        assert_eq!(
            who,
            vec![Some(sacha)],
            "seul ce qui s'est passé en son absence"
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
