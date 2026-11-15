//! Systèmes amis : identité, confidentialité, fiches et courrier chiffrés.
//!
//! Chaque système a une identité X25519 (`kv` `friends.identity`, synchronisée
//! entre ses appareils). Le serveur relaie des blobs scellés entre deux identités
//! et ne voit ni noms, ni membres, ni messages. Ce qu'un ami voit de nous tient
//! dans une « fiche » construite selon nos réglages de confidentialité.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::cloudkey::{FriendIdentity, InviteSecret, friend_code};
use crate::{CoreError, Result, front, kv, members, outbox, system};

const K_IDENTITY: &str = "friends.identity";
const K_PRIVACY: &str = "friends.privacy";
/// Empreinte de la dernière fiche publiée pour un ami (locale à l'appareil).
const K_CARD_HASH: &str = "friends.cardhash.";
pub const OUTBOX_TOPIC: &str = "friend";
const CARD: &str = "card";
const MAIL: &str = "mail";
pub const MAX_NICKNAME: usize = 64;
/// Au-delà, les photos des membres sautent de la fiche (le serveur plafonne à ~2,7 Mo).
const CARD_BUDGET: usize = 2_000_000;
const MAX_REF: usize = 64;

/// Ce qu'un ami peut voir de nous et faire avec nous.
#[allow(clippy::struct_excessive_bools)] // réglages indépendants, un interrupteur chacun
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Privacy {
    /// Liste des membres (sinon : seulement le nom et la photo du système).
    pub members: bool,
    pub avatars: bool,
    pub pronouns: bool,
    pub descriptions: bool,
    /// Qui est au front en ce moment.
    pub front: bool,
    /// Peut nous écrire.
    pub messages: bool,
    /// Membres jamais montrés, même si la liste l'est.
    pub hidden: Vec<Uuid>,
}

impl Default for Privacy {
    fn default() -> Self {
        Self {
            members: true,
            avatars: true,
            pronouns: true,
            descriptions: false,
            front: false,
            messages: true,
            hidden: Vec::new(),
        }
    }
}

/// Membre tel que montré à un ami. `id` est notre identifiant de membre.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardMember {
    pub id: String,
    pub name: String,
    pub color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pronouns: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
}

/// Fiche envoyée à un ami, scellée pour lui seul.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub v: u32,
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(default)]
    pub members: Vec<CardMember>,
    /// `None` : le front n'est pas partagé.
    #[serde(default)]
    pub front: Option<Vec<String>>,
    #[serde(default)]
    pub messages: bool,
}

/// Contenu d'une invitation : de quoi reconnaître le système et lui écrire.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteCard {
    #[serde(rename = "pub")]
    pub public: String,
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
}

/// Ami tel qu'affiché.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Friend {
    pub id: String,
    /// Nom choisi par l'ami pour son système.
    pub name: Option<String>,
    /// Surnom qu'on lui donne (prioritaire à l'affichage).
    pub nickname: Option<String>,
    pub avatar: Option<String>,
    pub members: Vec<CardMember>,
    pub front: Option<Vec<String>>,
    /// L'ami accepte nos messages.
    pub accepts_messages: bool,
    /// Code de vérification à comparer de vive voix.
    pub code: String,
    /// Réglages propres à cet ami ; `None` : réglages par défaut.
    pub privacy: Option<Privacy>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub card_at: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub created_at: i64,
}

/// Ami d'après le serveur.
#[derive(Debug, Clone)]
pub struct RemoteFriend {
    pub id: String,
    pub public: [u8; 32],
    /// Fiche scellée, en base64.
    pub card: Option<String>,
    pub card_at: Option<i64>,
    pub created_at: i64,
}

/// Membre d'un système ami cité dans un message ou un groupe, avec nom et
/// couleur gardés pour l'historique.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRef {
    #[serde(rename = "ref")]
    pub reference: String,
    pub name: String,
    pub color: String,
}

/// Ce qui circule entre deux systèmes amis, une fois déchiffré.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum Envelope {
    #[serde(rename_all = "camelCase")]
    Msg {
        id: Uuid,
        /// Auteurs chez l'expéditeur.
        authors: Vec<RemoteRef>,
        by_system: bool,
        /// Destinataires chez le destinataire ; vide : tout le système.
        to: Vec<Uuid>,
        body: String,
        at: i64,
        #[serde(default)]
        group: Option<Uuid>,
        #[serde(default)]
        reply: Option<Uuid>,
    },
    #[serde(rename_all = "camelCase")]
    Edit {
        id: Uuid,
        authors: Vec<RemoteRef>,
        by_system: bool,
        to: Vec<Uuid>,
        body: String,
        at: i64,
    },
    Del {
        id: Uuid,
    },
    /// Création ou mise à jour d'un groupe partagé.
    Group {
        id: Uuid,
        name: String,
        /// Membres de l'expéditeur ; vide : tout son système.
        mine: Vec<RemoteRef>,
        /// Nos membres ; vide : tout notre système.
        yours: Vec<Uuid>,
    },
}

/// Courrier prêt à partir.
#[derive(Debug, Clone)]
pub struct Outgoing {
    pub outbox_id: i64,
    pub friend: String,
    /// Scellé, en base64.
    pub item: String,
}

fn b64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

fn unb64(s: &str) -> Result<Vec<u8>> {
    STANDARD
        .decode(s.as_bytes())
        .map_err(|_| CoreError::Invalid("base64"))
}

fn check_ref(r: &str) -> Result<()> {
    if r.is_empty() || r.len() > MAX_REF {
        return Err(CoreError::Invalid("ref"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Identité et réglages
// ---------------------------------------------------------------------------

/// Identité du système, créée au premier besoin.
pub fn identity(conn: &Connection) -> Result<FriendIdentity> {
    if let Some(hex) = kv::get(conn, K_IDENTITY)? {
        return FriendIdentity::from_hex(&hex);
    }
    let id = FriendIdentity::generate()?;
    kv::set(conn, K_IDENTITY, &id.to_hex())?;
    Ok(id)
}

pub fn has_identity(conn: &Connection) -> Result<bool> {
    Ok(kv::get(conn, K_IDENTITY)?.is_some())
}

/// Nouvelle identité : les amitiés existantes ne pourront plus être lues.
pub fn reset_identity(conn: &Connection) -> Result<FriendIdentity> {
    let id = FriendIdentity::generate()?;
    kv::set(conn, K_IDENTITY, &id.to_hex())?;
    conn.execute("DELETE FROM friend", [])?;
    conn.execute(
        "DELETE FROM kv WHERE key >= ?1 AND key < ?2",
        params![K_CARD_HASH, "friends.cardhash/"],
    )?;
    Ok(id)
}

pub fn privacy(conn: &Connection) -> Result<Privacy> {
    Ok(kv::get(conn, K_PRIVACY)?
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default())
}

pub fn set_privacy(conn: &Connection, p: &Privacy) -> Result<()> {
    let json = serde_json::to_string(p).map_err(|_| CoreError::Invalid("privacy"))?;
    kv::set(conn, K_PRIVACY, &json)
}

type Pref = (Option<String>, Option<Privacy>);

fn pref(conn: &Connection, id: &str) -> Result<Pref> {
    let row: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT nickname, privacy FROM friend_pref WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(row.map_or((None, None), |(n, p)| {
        (n, p.and_then(|p| serde_json::from_str(&p).ok()))
    }))
}

/// Surnom et réglages propres à un ami (`privacy` `None` : réglages par défaut).
pub fn set_pref(
    conn: &Connection,
    id: &str,
    nickname: Option<&str>,
    privacy: Option<&Privacy>,
) -> Result<()> {
    check_ref(id)?;
    let nickname = nickname.map(str::trim).filter(|n| !n.is_empty());
    if nickname.is_some_and(|n| n.chars().count() > MAX_NICKNAME) {
        return Err(CoreError::Invalid("nickname"));
    }
    let privacy = privacy
        .map(serde_json::to_string)
        .transpose()
        .map_err(|_| CoreError::Invalid("privacy"))?;
    if nickname.is_none() && privacy.is_none() {
        conn.execute("DELETE FROM friend_pref WHERE id = ?1", [id])?;
    } else {
        conn.execute(
            "INSERT INTO friend_pref (id, nickname, privacy) VALUES (?1, ?2, ?3)
             ON CONFLICT (id) DO UPDATE SET nickname = excluded.nickname, privacy = excluded.privacy",
            params![id, nickname, privacy],
        )?;
    }
    Ok(())
}

/// Réglages qui s'appliquent à cet ami.
pub fn privacy_for(conn: &Connection, id: &str) -> Result<Privacy> {
    match pref(conn, id)?.1 {
        Some(p) => Ok(p),
        None => privacy(conn),
    }
}

// ---------------------------------------------------------------------------
// Fiches
// ---------------------------------------------------------------------------

/// Fiche telle qu'un ami ayant ces réglages la verra.
pub fn build_card(conn: &Connection, p: &Privacy) -> Result<Card> {
    let sys = system::get(conn)?;
    let mut members_out = Vec::new();
    if p.members {
        for m in members::list(conn)? {
            if p.hidden.contains(&m.id) {
                continue;
            }
            members_out.push(CardMember {
                id: m.id.to_string(),
                name: m.name,
                color: m.color,
                pronouns: m.pronouns.filter(|_| p.pronouns),
                description: m.description.filter(|_| p.descriptions),
                avatar: m.avatar.filter(|_| p.avatars),
            });
        }
    }
    let front = if p.front {
        let visible: Vec<String> = front::current(conn)?
            .into_iter()
            .filter_map(|s| s.member_id)
            .filter(|id| !p.hidden.contains(id))
            .map(|id| id.to_string())
            .filter(|id| members_out.iter().any(|m| &m.id == id))
            .collect();
        Some(visible)
    } else {
        None
    };
    let mut card = Card {
        v: 1,
        name: sys.name,
        avatar: sys.avatar.filter(|_| p.avatars),
        members: members_out,
        front,
        messages: p.messages,
    };
    // Trop lourde : on retire les photos des membres, en commençant par les plus grosses.
    let mut size = card_size(&card);
    while size > CARD_BUDGET {
        let Some(biggest) = card
            .members
            .iter_mut()
            .filter(|m| m.avatar.is_some())
            .max_by_key(|m| m.avatar.as_ref().map_or(0, String::len))
        else {
            break;
        };
        size -= biggest.avatar.take().map_or(0, |a| a.len());
    }
    Ok(card)
}

fn card_size(card: &Card) -> usize {
    serde_json::to_vec(card).map_or(usize::MAX, |v| v.len())
}

fn card_json(card: &Card) -> Result<String> {
    serde_json::to_string(card).map_err(|_| CoreError::Invalid("card"))
}

/// Fiche par défaut scellée pour un système pas encore ami (demande ou acceptation).
pub fn sealed_card_for(conn: &Connection, their: &[u8; 32]) -> Result<String> {
    let json = card_json(&build_card(conn, &privacy(conn)?)?)?;
    Ok(b64(&identity(conn)?.seal_for(
        their,
        CARD,
        json.as_bytes(),
    )?))
}

/// Ouvre la fiche qu'un système nous a scellée.
pub fn open_card(conn: &Connection, their: &[u8; 32], sealed: &str) -> Result<Card> {
    let plain = identity(conn)?.open_from(their, CARD, &unb64(sealed)?)?;
    let card: Card = serde_json::from_slice(&plain).map_err(|_| CoreError::Invalid("card"))?;
    Ok(sanitize(card))
}

/// Une fiche vient d'ailleurs : on borne ce qu'on en garde.
fn sanitize(mut card: Card) -> Card {
    let image = |a: &Option<String>| a.as_deref().is_some_and(|a| a.starts_with("data:image/"));
    if !image(&card.avatar) {
        card.avatar = None;
    }
    card.name = card
        .name
        .map(|n| n.chars().take(members::MAX_NAME).collect());
    card.members.retain(|m| check_ref(&m.id).is_ok());
    card.members.truncate(2000);
    for m in &mut card.members {
        m.name = m.name.chars().take(members::MAX_NAME).collect();
        if !is_color(&m.color) {
            m.color = "#808080".into();
        }
        if !image(&m.avatar) {
            m.avatar = None;
        }
    }
    card
}

fn is_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Invitation : notre clé publique, notre nom et notre photo, scellés avec le secret
/// du lien (le serveur garde le blob mais ne peut pas le lire).
pub fn invite_card(conn: &Connection, secret: &InviteSecret) -> Result<String> {
    let p = privacy(conn)?;
    let sys = system::get(conn)?;
    let card = InviteCard {
        public: hex::encode(identity(conn)?.public()),
        name: sys.name,
        avatar: sys.avatar.filter(|_| p.avatars),
    };
    let json = serde_json::to_vec(&card).map_err(|_| CoreError::Invalid("card"))?;
    Ok(b64(&secret.seal(&json)?))
}

pub fn open_invite(secret: &InviteSecret, sealed: &str) -> Result<(InviteCard, [u8; 32])> {
    let plain = secret.open(&unb64(sealed)?)?;
    let mut card: InviteCard =
        serde_json::from_slice(&plain).map_err(|_| CoreError::Invalid("card"))?;
    let mut public = [0u8; 32];
    hex::decode_to_slice(&card.public, &mut public).map_err(|_| CoreError::Invalid("card"))?;
    if !card
        .avatar
        .as_deref()
        .is_some_and(|a| a.starts_with("data:image/"))
    {
        card.avatar = None;
    }
    card.name = card
        .name
        .map(|n| n.chars().take(members::MAX_NAME).collect());
    Ok((card, public))
}

/// Code de vérification entre nous et ce système.
pub fn code_with(conn: &Connection, their: &[u8; 32]) -> Result<String> {
    Ok(friend_code(&identity(conn)?.public(), their))
}

// ---------------------------------------------------------------------------
// Liste des amis (cache local de la liste du serveur)
// ---------------------------------------------------------------------------

/// Remplace le cache par la liste du serveur ; les fiches illisibles sont ignorées.
pub fn reconcile(conn: &mut Connection, remote: &[RemoteFriend]) -> Result<()> {
    let tx = conn.transaction()?;
    let me = identity(&tx)?;
    let keep: Vec<&str> = remote.iter().map(|f| f.id.as_str()).collect();
    let existing: Vec<String> = tx
        .prepare("SELECT id FROM friend")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for id in existing.iter().filter(|id| !keep.contains(&id.as_str())) {
        tx.execute("DELETE FROM friend WHERE id = ?1", [id])?;
        kv::delete(&tx, &format!("{K_CARD_HASH}{id}"))?;
    }
    for f in remote {
        if check_ref(&f.id).is_err() {
            continue;
        }
        let card = f
            .card
            .as_deref()
            .and_then(|c| unb64(c).ok())
            .and_then(|c| me.open_from(&f.public, CARD, &c).ok())
            .and_then(|plain| serde_json::from_slice::<Card>(&plain).ok())
            .map(sanitize)
            .and_then(|c| card_json(&c).ok());
        // Clé publique changée : l'ami a réinitialisé son identité, on republiera.
        let old_pub: Option<Vec<u8>> = tx
            .query_row("SELECT pub FROM friend WHERE id = ?1", [&f.id], |r| {
                r.get(0)
            })
            .optional()?;
        if old_pub.is_some_and(|p| p != f.public) {
            kv::delete(&tx, &format!("{K_CARD_HASH}{}", f.id))?;
        }
        tx.execute(
            "INSERT INTO friend (id, pub, card, card_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (id) DO UPDATE SET pub = excluded.pub,
                card = coalesce(excluded.card, friend.card), card_at = excluded.card_at",
            params![f.id, f.public, card, f.card_at, f.created_at],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn friend_pub(conn: &Connection, id: &str) -> Result<Option<[u8; 32]>> {
    Ok(conn
        .query_row("SELECT pub FROM friend WHERE id = ?1", [id], |r| {
            r.get::<_, [u8; 32]>(0)
        })
        .optional()?)
}

fn cached_card(conn: &Connection, id: &str) -> Result<Option<Card>> {
    let json: Option<Option<String>> = conn
        .query_row("SELECT card FROM friend WHERE id = ?1", [id], |r| r.get(0))
        .optional()?;
    Ok(json
        .flatten()
        .and_then(|j| serde_json::from_str::<Card>(&j).ok()))
}

type FriendRow = (String, [u8; 32], Option<String>, Option<i64>, i64);

pub fn list(conn: &Connection) -> Result<Vec<Friend>> {
    let me = identity(conn)?.public();
    let rows: Vec<FriendRow> = conn
        .prepare("SELECT id, pub, card, card_at, created_at FROM friend")?
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::with_capacity(rows.len());
    for (id, public, card, card_at, created_at) in rows {
        let card = card.and_then(|j| serde_json::from_str::<Card>(&j).ok());
        let (nickname, privacy) = pref(conn, &id)?;
        let card = card.unwrap_or(Card {
            v: 1,
            name: None,
            avatar: None,
            members: Vec::new(),
            front: None,
            messages: false,
        });
        out.push(Friend {
            code: friend_code(&me, &public),
            id,
            name: card.name,
            nickname,
            avatar: card.avatar,
            members: card.members,
            front: card.front,
            accepts_messages: card.messages,
            privacy,
            card_at,
            created_at,
        });
    }
    out.sort_by_key(|f| {
        f.nickname
            .clone()
            .or_else(|| f.name.clone())
            .unwrap_or_default()
            .to_lowercase()
    });
    Ok(out)
}

pub fn get(conn: &Connection, id: &str) -> Result<Friend> {
    list(conn)?
        .into_iter()
        .find(|f| f.id == id)
        .ok_or(CoreError::NotFound)
}

/// Oublie un ami localement (après l'avoir retiré ou bloqué sur le serveur).
/// Les messages échangés restent, rattachés à un ami disparu.
pub fn forget(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM friend WHERE id = ?1", [id])?;
    conn.execute("DELETE FROM friend_pref WHERE id = ?1", [id])?;
    conn.execute(
        "DELETE FROM outbox WHERE topic = ?1 AND json_extract(payload, '$.friend') = ?2",
        params![OUTBOX_TOPIC, id],
    )?;
    kv::delete(conn, &format!("{K_CARD_HASH}{id}"))
}

/// Fiches à (re)publier : celles dont le contenu a changé depuis le dernier envoi.
/// Renvoie (ami, fiche scellée, empreinte à confirmer avec [`card_published`]).
pub fn cards_to_publish(conn: &Connection) -> Result<Vec<(String, String, String)>> {
    let me = identity(conn)?;
    let rows: Vec<(String, [u8; 32])> = conn
        .prepare("SELECT id, pub FROM friend")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut out = Vec::new();
    for (id, public) in rows {
        let json = card_json(&build_card(conn, &privacy_for(conn, &id)?)?)?;
        let mut h = Sha256::new();
        h.update(public);
        h.update(json.as_bytes());
        let hash = hex::encode(h.finalize());
        if kv::get(conn, &format!("{K_CARD_HASH}{id}"))?.as_deref() == Some(hash.as_str()) {
            continue;
        }
        let sealed = b64(&me.seal_for(&public, CARD, json.as_bytes())?);
        out.push((id, sealed, hash));
    }
    Ok(out)
}

pub fn card_published(conn: &Connection, id: &str, hash: &str) -> Result<()> {
    kv::set(conn, &format!("{K_CARD_HASH}{id}"), hash)
}

// ---------------------------------------------------------------------------
// Courrier
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct Queued {
    friend: String,
    env: Envelope,
}

/// Met une enveloppe en file d'envoi (dans la transaction de l'appelant).
pub(crate) fn queue(conn: &Connection, friend: &str, env: Envelope) -> Result<()> {
    let payload = serde_json::to_string(&Queued {
        friend: friend.to_owned(),
        env,
    })
    .map_err(|_| CoreError::Invalid("envelope"))?;
    outbox::push(conn, OUTBOX_TOPIC, &payload, crate::time::now_ms()?)
}

/// Courrier en attente, scellé pour son destinataire. Ce qui vise un ami disparu
/// est abandonné.
pub fn outgoing(conn: &Connection, limit: u32) -> Result<Vec<Outgoing>> {
    let me = identity(conn)?;
    let mut out = Vec::new();
    for item in outbox::pending(conn, OUTBOX_TOPIC, limit)? {
        let Ok(q) = serde_json::from_str::<Queued>(&item.payload) else {
            outbox::done(conn, item.id)?;
            continue;
        };
        let Some(public) = friend_pub(conn, &q.friend)? else {
            outbox::done(conn, item.id)?;
            continue;
        };
        let json = serde_json::to_vec(&q.env).map_err(|_| CoreError::Invalid("envelope"))?;
        out.push(Outgoing {
            outbox_id: item.id,
            friend: q.friend,
            item: b64(&me.seal_for(&public, MAIL, &json)?),
        });
    }
    Ok(out)
}

/// Applique un courrier reçu. Ce qui est illisible, d'un inconnu ou refusé par
/// nos réglages est ignoré sans erreur : l'appelant peut l'acquitter.
pub fn receive(conn: &mut Connection, friend: &str, sealed: &str) -> Result<bool> {
    let Some(public) = friend_pub(conn, friend)? else {
        return Ok(false);
    };
    let Ok(raw) = unb64(sealed) else {
        return Ok(false);
    };
    let Ok(plain) = identity(conn)?.open_from(&public, MAIL, &raw) else {
        return Ok(false);
    };
    let Ok(env) = serde_json::from_slice::<Envelope>(&plain) else {
        return Ok(false);
    };
    if !privacy_for(conn, friend)?.messages {
        return Ok(false);
    }
    let tx = conn.transaction()?;
    let applied = apply(&tx, friend, env)?;
    tx.commit()?;
    Ok(applied)
}

fn clean_refs(list: Vec<RemoteRef>) -> Vec<RemoteRef> {
    let mut out: Vec<RemoteRef> = Vec::new();
    for mut r in list {
        if check_ref(&r.reference).is_err() || out.iter().any(|o| o.reference == r.reference) {
            continue;
        }
        r.name = r.name.chars().take(members::MAX_NAME).collect();
        if !is_color(&r.color) {
            r.color = "#808080".into();
        }
        out.push(r);
        if out.len() >= 500 {
            break;
        }
    }
    out
}

/// Garde nos membres existants (archivés compris : l'historique reste lisible).
fn own_members(conn: &Connection, list: &[Uuid]) -> Result<Vec<Uuid>> {
    let mut out = Vec::new();
    let mut st = conn.prepare_cached("SELECT 1 FROM member WHERE id = ?1")?;
    for id in list {
        if !out.contains(id) && st.exists([id.as_bytes()])? {
            out.push(*id);
        }
    }
    Ok(out)
}

fn write_remote(conn: &Connection, message: Uuid, role: i64, refs: &[RemoteRef]) -> Result<()> {
    conn.execute(
        "DELETE FROM message_remote WHERE message_id = ?1 AND role = ?2",
        params![message.as_bytes(), role],
    )?;
    let mut st = conn.prepare_cached(
        "INSERT OR IGNORE INTO message_remote (message_id, role, ref, name, color)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for r in refs {
        st.execute(params![
            message.as_bytes(),
            role,
            r.reference,
            r.name,
            r.color
        ])?;
    }
    Ok(())
}

pub(crate) fn write_message_remote(
    conn: &Connection,
    message: Uuid,
    role: i64,
    refs: &[RemoteRef],
) -> Result<()> {
    write_remote(conn, message, role, refs)
}

fn write_recipients(conn: &Connection, message: Uuid, to: &[Uuid]) -> Result<()> {
    conn.execute(
        "DELETE FROM message_recipient WHERE message_id = ?1",
        [message.as_bytes()],
    )?;
    let mut st = conn
        .prepare_cached("INSERT INTO message_recipient (message_id, member_id) VALUES (?1, ?2)")?;
    for m in to {
        st.execute(params![message.as_bytes(), m.as_bytes()])?;
    }
    Ok(())
}

/// Le message existe-t-il, reçu de cet ami ?
fn incoming_from(conn: &Connection, id: Uuid, friend: &str) -> Result<Option<bool>> {
    Ok(conn
        .query_row(
            "SELECT friend_id = ?2 AND outgoing = 0 FROM message WHERE id = ?1",
            params![id.as_bytes(), friend],
            |r| r.get::<_, Option<bool>>(0),
        )
        .optional()?
        .map(|v| v.unwrap_or(false)))
}

#[allow(clippy::too_many_lines)] // un bras par type d'enveloppe
fn apply(conn: &Connection, friend: &str, env: Envelope) -> Result<bool> {
    match env {
        Envelope::Msg {
            id,
            authors,
            by_system,
            to,
            body,
            at,
            group,
            reply,
        } => {
            let body = body.trim().to_owned();
            if body.is_empty() || body.chars().count() > crate::messages::MAX_BODY_CHARS {
                return Ok(false);
            }
            if incoming_from(conn, id, friend)?.is_some() {
                return Ok(false);
            }
            // Le groupe et la réponse ne tiennent que s'ils relèvent de cet ami.
            let group = match group {
                Some(g) => conn
                    .query_row(
                        "SELECT 1 FROM chat_group WHERE id = ?1 AND friend_id = ?2",
                        params![g.as_bytes(), friend],
                        |_| Ok(()),
                    )
                    .optional()?
                    .map(|()| g),
                None => None,
            };
            let reply = match reply {
                Some(r) => conn
                    .query_row(
                        "SELECT 1 FROM message WHERE id = ?1 AND friend_id = ?2",
                        params![r.as_bytes(), friend],
                        |_| Ok(()),
                    )
                    .optional()?
                    .map(|()| r),
                None => None,
            };
            let to = own_members(conn, &to)?;
            let now = crate::time::now_ms()?;
            let at = if at > 0 && at <= now + 300_000 {
                at
            } else {
                now
            };
            conn.execute(
                "INSERT INTO message (id, author_id, recipient_id, body, created_at, group_id,
                                      reply_to, friend_id, outgoing, by_system)
                 VALUES (?1, NULL, NULL, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
                params![
                    id.as_bytes(),
                    body,
                    at,
                    group.map(|u| *u.as_bytes()),
                    reply.map(|u| *u.as_bytes()),
                    friend,
                    by_system,
                ],
            )?;
            write_remote(conn, id, 0, &clean_refs(authors))?;
            write_recipients(conn, id, &to)?;
            Ok(true)
        }
        Envelope::Edit {
            id,
            authors,
            by_system,
            to,
            body,
            at,
        } => {
            let body = body.trim().to_owned();
            if body.is_empty() || body.chars().count() > crate::messages::MAX_BODY_CHARS {
                return Ok(false);
            }
            if incoming_from(conn, id, friend)? != Some(true) {
                return Ok(false);
            }
            let to = own_members(conn, &to)?;
            conn.execute(
                "UPDATE message SET body = ?2, edited_at = ?3, by_system = ?4 WHERE id = ?1",
                params![
                    id.as_bytes(),
                    body,
                    at.min(crate::time::now_ms()? + 300_000),
                    by_system
                ],
            )?;
            write_remote(conn, id, 0, &clean_refs(authors))?;
            write_recipients(conn, id, &to)?;
            Ok(true)
        }
        Envelope::Del { id } => {
            if incoming_from(conn, id, friend)? != Some(true) {
                return Ok(false);
            }
            conn.execute("DELETE FROM message WHERE id = ?1", [id.as_bytes()])?;
            Ok(true)
        }
        Envelope::Group {
            id,
            name,
            mine,
            yours,
        } => {
            let name: String = name.trim().chars().take(members::MAX_NAME).collect();
            if name.is_empty() {
                return Ok(false);
            }
            let owner: Option<Option<String>> = conn
                .query_row(
                    "SELECT friend_id FROM chat_group WHERE id = ?1",
                    [id.as_bytes()],
                    |r| r.get(0),
                )
                .optional()?;
            match owner {
                Some(Some(f)) if f == friend => {
                    conn.execute(
                        "UPDATE chat_group SET name = ?2 WHERE id = ?1",
                        params![id.as_bytes(), name],
                    )?;
                }
                Some(_) => return Ok(false),
                None => {
                    conn.execute(
                        "INSERT INTO chat_group (id, name, avatar, created_at, friend_id)
                         VALUES (?1, ?2, NULL, ?3, ?4)",
                        params![id.as_bytes(), name, crate::time::now_ms()?, friend],
                    )?;
                }
            }
            let yours = own_members(conn, &yours)?;
            crate::groups::write_members(conn, id, &yours)?;
            crate::groups::write_remote(conn, id, &clean_refs(mine))?;
            Ok(true)
        }
    }
}

/// Nos membres tels que cités chez l'ami (nom et couleur actuels).
pub(crate) fn own_refs(conn: &Connection, ids: &[Uuid]) -> Result<Vec<RemoteRef>> {
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let m = members::get(conn, *id)?;
        out.push(RemoteRef {
            reference: m.id.to_string(),
            name: m.name,
            color: m.color,
        });
    }
    Ok(out)
}

/// Membres de l'ami désignés par leur identifiant chez lui, d'après sa fiche.
/// Un membre absent de la fiche (masqué ou retiré) est refusé.
pub(crate) fn friend_refs(
    conn: &Connection,
    friend: &str,
    refs: &[String],
) -> Result<Vec<RemoteRef>> {
    if refs.is_empty() {
        return Ok(Vec::new());
    }
    let card = cached_card(conn, friend)?.ok_or(CoreError::Invalid("recipients"))?;
    let mut out: Vec<RemoteRef> = Vec::new();
    for r in refs {
        if out.iter().any(|o| &o.reference == r) {
            continue;
        }
        let m = card
            .members
            .iter()
            .find(|m| &m.id == r)
            .ok_or(CoreError::Invalid("recipients"))?;
        out.push(RemoteRef {
            reference: m.id.clone(),
            name: m.name.clone(),
            color: m.color.clone(),
        });
    }
    Ok(out)
}

/// L'ami existe et accepte nos messages.
pub(crate) fn ensure_writable(conn: &Connection, friend: &str) -> Result<()> {
    let card = cached_card(conn, friend)?;
    if friend_pub(conn, friend)?.is_none() {
        return Err(CoreError::NotFound);
    }
    if !card.is_some_and(|c| c.messages) {
        return Err(CoreError::Invalid("friend_closed"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::members::NewMember;

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
                pronouns: Some("iel".into()),
                color: "#4f8583".into(),
            },
        )
        .unwrap()
        .id
    }

    /// Deux systèmes amis, chacun avec sa base, reliés comme le ferait le serveur.
    fn pair() -> (Connection, Connection) {
        let mut a = db();
        let mut b = db();
        let pa = identity(&a).unwrap().public();
        let pb = identity(&b).unwrap().public();
        let card_a = sealed_card_for(&a, &pb).unwrap();
        let card_b = sealed_card_for(&b, &pa).unwrap();
        reconcile(
            &mut a,
            &[RemoteFriend {
                id: "f-1".into(),
                public: pb,
                card: Some(card_b),
                card_at: Some(1),
                created_at: 1,
            }],
        )
        .unwrap();
        reconcile(
            &mut b,
            &[RemoteFriend {
                id: "f-1".into(),
                public: pa,
                card: Some(card_a),
                card_at: Some(1),
                created_at: 1,
            }],
        )
        .unwrap();
        (a, b)
    }

    fn deliver(from: &Connection, to: &mut Connection) -> usize {
        let mut n = 0;
        for o in outgoing(from, 50).unwrap() {
            if receive(to, &o.friend, &o.item).unwrap() {
                n += 1;
            }
            outbox::done(from, o.outbox_id).unwrap();
        }
        n
    }

    #[test]
    fn card_respects_privacy() {
        let c = db();
        let mila = member(&c, "Mila");
        let sacha = member(&c, "Sacha");
        let mut p = Privacy::default();
        p.hidden.push(sacha);
        let card = build_card(&c, &p).unwrap();
        assert_eq!(card.members.len(), 1);
        assert_eq!(card.members[0].id, mila.to_string());
        assert_eq!(card.members[0].pronouns.as_deref(), Some("iel"));
        assert!(card.front.is_none());
        p.members = false;
        assert!(build_card(&c, &p).unwrap().members.is_empty());
        p.members = true;
        p.pronouns = false;
        assert!(build_card(&c, &p).unwrap().members[0].pronouns.is_none());
    }

    #[test]
    fn friends_see_each_other_and_share_a_code() {
        let (a, b) = pair();
        member(&a, "Mila");
        let fa = list(&a).unwrap();
        let fb = list(&b).unwrap();
        assert_eq!(fa.len(), 1);
        assert_eq!(fa[0].code, fb[0].code);
        assert!(fa[0].accepts_messages);
        // Fiche à publier, puis plus rien tant qu'elle ne change pas.
        let pubs = cards_to_publish(&a).unwrap();
        assert_eq!(pubs.len(), 1);
        card_published(&a, &pubs[0].0, &pubs[0].2).unwrap();
        assert!(cards_to_publish(&a).unwrap().is_empty());
        member(&a, "Sacha");
        assert_eq!(cards_to_publish(&a).unwrap().len(), 1);
    }

    #[test]
    fn invite_roundtrip() {
        let c = db();
        let s = InviteSecret::generate().unwrap();
        let sealed = invite_card(&c, &s).unwrap();
        let (card, public) = open_invite(&s, &sealed).unwrap();
        assert_eq!(public, identity(&c).unwrap().public());
        assert_eq!(card.public, hex::encode(public));
        let other = InviteSecret::generate().unwrap();
        assert!(open_invite(&other, &sealed).is_err());
    }

    #[test]
    fn mail_is_applied_once_and_only_from_its_sender() {
        let (a, mut b) = pair();
        let bob = member(&b, "Bob");
        let env = Envelope::Msg {
            id: Uuid::now_v7(),
            authors: vec![RemoteRef {
                reference: "x".into(),
                name: "Mila".into(),
                color: "#112233".into(),
            }],
            by_system: false,
            to: vec![bob, Uuid::now_v7()],
            body: "coucou".into(),
            at: 5,
            group: None,
            reply: None,
        };
        queue(&a, "f-1", env.clone()).unwrap();
        queue(&a, "f-1", env).unwrap();
        assert_eq!(deliver(&a, &mut b), 1);
        let (to, remote): (i64, i64) = b
            .query_row(
                "SELECT (SELECT count(*) FROM message_recipient),
                        (SELECT count(*) FROM message_remote WHERE role = 0)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((to, remote), (1, 1));

        // Fermé aux messages : ignoré.
        let p = Privacy {
            messages: false,
            ..Privacy::default()
        };
        set_privacy(&b, &p).unwrap();
        queue(
            &a,
            "f-1",
            Envelope::Msg {
                id: Uuid::now_v7(),
                authors: vec![],
                by_system: true,
                to: vec![],
                body: "encore".into(),
                at: 6,
                group: None,
                reply: None,
            },
        )
        .unwrap();
        assert_eq!(deliver(&a, &mut b), 0);
    }

    #[test]
    fn unknown_or_tampered_mail_is_ignored() {
        let (_a, mut b) = pair();
        assert!(!receive(&mut b, "inconnu", "AAAA").unwrap());
        assert!(!receive(&mut b, "f-1", "pas du base64 !").unwrap());
        assert!(!receive(&mut b, "f-1", &b64(&[0u8; 80])).unwrap());
    }

    #[test]
    fn groups_are_shared() {
        let (a, mut b) = pair();
        let bob = member(&b, "Bob");
        let gid = Uuid::now_v7();
        queue(
            &a,
            "f-1",
            Envelope::Group {
                id: gid,
                name: "Thé".into(),
                mine: vec![],
                yours: vec![bob],
            },
        )
        .unwrap();
        assert_eq!(deliver(&a, &mut b), 1);
        let g = crate::groups::get(&b, gid).unwrap();
        assert_eq!(g.friend_id.as_deref(), Some("f-1"));
        assert_eq!(g.member_ids, vec![bob]);
    }

    fn refresh(from: &Connection, to: &mut Connection) {
        let card = sealed_card_for(from, &identity(to).unwrap().public()).unwrap();
        reconcile(
            to,
            &[RemoteFriend {
                id: "f-1".into(),
                public: identity(from).unwrap().public(),
                card: Some(card),
                card_at: Some(2),
                created_at: 1,
            }],
        )
        .unwrap();
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn conversation_between_systems() {
        use crate::groups::{self, GroupInput};
        use crate::messages::{self, NewMessage};
        let (mut a, mut b) = pair();
        let mila = member(&a, "Mila");
        let bob = member(&b, "Bob");
        // Les fiches doivent connaître les nouveaux membres.
        refresh(&a, &mut b);
        refresh(&b, &mut a);
        // Un destinataire local dans une conversation avec un ami : refusé.
        assert!(
            messages::post(
                &mut a,
                &NewMessage {
                    author_ids: vec![mila],
                    recipient_ids: vec![mila],
                    body: "x".into(),
                    friend_id: Some("f-1".into()),
                    ..Default::default()
                },
            )
            .is_err()
        );
        let sent = messages::post(
            &mut a,
            &NewMessage {
                author_ids: vec![mila],
                body: "salut Bob".into(),
                friend_id: Some("f-1".into()),
                remote_recipient_refs: vec![bob.to_string()],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(sent.outgoing);
        assert_eq!(sent.remote_recipients[0].name, "Bob");
        // Envoyé : pas « non lu » chez soi.
        assert!(messages::list(&a, None, 50, true).unwrap().is_empty());
        assert_eq!(deliver(&a, &mut b), 1);
        let got = &messages::list(&b, None, 50, false).unwrap()[0];
        assert!(!got.outgoing);
        assert_eq!(got.recipient_ids, vec![bob]);
        assert_eq!(got.remote_authors[0].name, "Mila");
        assert_eq!(messages::list(&b, None, 50, true).unwrap().len(), 1);
        // Un message reçu ne se modifie pas.
        assert!(
            messages::update(
                &mut b,
                got.id,
                &NewMessage {
                    body: "non".into(),
                    ..Default::default()
                }
            )
            .is_err()
        );

        // Signé par le système, pour tout le système de l'ami.
        messages::update(
            &mut a,
            sent.id,
            &NewMessage {
                body: "salut à vous".into(),
                by_system: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(deliver(&a, &mut b), 1);
        let got = messages::list(&b, None, 50, false).unwrap().remove(0);
        assert!(got.by_system && got.recipient_ids.is_empty() && got.remote_authors.is_empty());
        assert_eq!(got.body, "salut à vous");

        messages::delete(&a, sent.id).unwrap();
        assert_eq!(deliver(&a, &mut b), 1);
        assert!(messages::list(&b, None, 50, false).unwrap().is_empty());

        // Groupe partagé : tout le système d'en face, puis un message dedans.
        let g = groups::create(
            &mut a,
            &GroupInput {
                name: "Thé".into(),
                member_ids: vec![],
                friend_id: Some("f-1".into()),
                ..Default::default()
            },
        )
        .unwrap();
        messages::post(
            &mut a,
            &NewMessage {
                author_ids: vec![mila],
                body: "dans le groupe".into(),
                group_id: Some(g.id),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(deliver(&a, &mut b), 2);
        let gb = groups::get(&b, g.id).unwrap();
        assert_eq!(gb.friend_id.as_deref(), Some("f-1"));
        let in_group = messages::list(&b, None, 50, false).unwrap().remove(0);
        assert_eq!(in_group.group_id, Some(g.id));
        // Répondre depuis l'autre côté, dans le même groupe.
        messages::post(
            &mut b,
            &NewMessage {
                body: "réponse".into(),
                by_system: true,
                group_id: Some(g.id),
                reply_to: Some(in_group.id),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(deliver(&b, &mut a), 1);
        let back = messages::list(&a, None, 50, false).unwrap().remove(0);
        assert_eq!(
            (back.group_id, back.reply_to),
            (Some(g.id), Some(in_group.id))
        );
    }
}
