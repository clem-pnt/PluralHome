//! Synchronisation bidirectionnelle avec PluralKit.
//!
//! Règles (voir aussi docs/pluralkit.md) :
//! - Membres : fusion à trois voies, champ par champ, contre le dernier état
//!   synchronisé (`pk_member.snapshot`). Un champ changé d'un seul côté est
//!   propagé ; changé des deux côtés, la version locale l'emporte. À la liaison,
//!   un membre présent des deux côtés sous le même nom prend les valeurs de
//!   PluralKit. Un membre supprimé sur PluralKit est délié, jamais effacé ici.
//! - Switchs : les changements locaux partent par la file d'envoi, dans l'ordre ;
//!   les switchs PluralKit plus récents que le dernier changement local sont
//!   appliqués ici. Chaque switch connu est mémorisé par son identifiant : aucun
//!   écho, aucune boucle. Le « flou » n'existe pas sur PluralKit : il y part
//!   comme « personne au front ».

use std::collections::{HashMap, HashSet};

use ph_core::front::{self, FrontChange, Origin, PK_SWITCH_TOPIC, PendingSwitch};
use ph_core::members::{self, Member, NewMember};
use ph_core::rusqlite::{Connection, params};
use ph_core::{kv, outbox};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::db::Db;
use crate::pluralkit::{PkApi, PkMember, PkMemberPatch, PkSwitch, PkSystem};
use crate::{RemoteError, Result};

pub const K_ENABLED: &str = "pluralkit.enabled";
pub const K_TOKEN: &str = "pluralkit.token";
const K_SYSTEM: &str = "pluralkit.system";
const K_LINKED_AT: &str = "pluralkit.linked_at";
const K_LAST_SYNC: &str = "pluralkit.last_sync";
const K_LAST_ERROR: &str = "pluralkit.last_error";

/// Couleurs attribuées aux membres importés sans couleur (mêmes que l'interface).
const PALETTE: [&str; 10] = [
    "#4f8583", "#c8643b", "#5b7db1", "#b0607a", "#7a9a5b", "#c99a3c", "#8c7356", "#4c6b77",
    "#b5574a", "#6f8f86",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Snapshot {
    name: String,
    pronouns: Option<String>,
    color: Option<String>,
}

#[derive(Debug, Clone)]
struct Link {
    member_id: Uuid,
    pk_uuid: String,
    pk_hid: String,
    snapshot: Snapshot,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SyncReport {
    pub members_pulled: usize,
    pub members_pushed: usize,
    pub switches_pulled: usize,
    pub switches_pushed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub enabled: bool,
    pub system: Option<String>,
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
    pub pending: i64,
}

// ---------- stockage ----------

fn load_links(c: &Connection) -> ph_core::Result<Vec<Link>> {
    let mut st = c.prepare_cached("SELECT member_id, pk_uuid, pk_hid, snapshot FROM pk_member")?;
    let rows = st.query_map([], |r| {
        let id: [u8; 16] = r.get(0)?;
        let snap: String = r.get(3)?;
        Ok((
            Uuid::from_bytes(id),
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            snap,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (member_id, pk_uuid, pk_hid, snap) = row?;
        // Un instantané illisible est traité comme vide : la fusion repartira des valeurs actuelles.
        let snapshot = serde_json::from_str(&snap).unwrap_or(Snapshot {
            name: String::new(),
            pronouns: None,
            color: None,
        });
        out.push(Link {
            member_id,
            pk_uuid,
            pk_hid,
            snapshot,
        });
    }
    Ok(out)
}

fn save_link(c: &Connection, l: &Link) -> ph_core::Result<()> {
    let snap = serde_json::to_string(&l.snapshot).unwrap_or_default();
    c.execute(
        "INSERT INTO pk_member (member_id, pk_uuid, pk_hid, snapshot) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (member_id) DO UPDATE SET pk_uuid = excluded.pk_uuid,
           pk_hid = excluded.pk_hid, snapshot = excluded.snapshot",
        params![l.member_id.as_bytes(), l.pk_uuid, l.pk_hid, snap],
    )?;
    Ok(())
}

fn remember_switch(c: &Connection, id: &str, ts: i64) -> ph_core::Result<bool> {
    Ok(c.execute(
        "INSERT OR IGNORE INTO pk_switch (pk_id, timestamp) VALUES (?1, ?2)",
        params![id, ts],
    )? == 1)
}

fn int(c: &Connection, key: &str) -> ph_core::Result<Option<i64>> {
    Ok(kv::get(c, key)?.and_then(|v| v.parse().ok()))
}

// ---------- conversions ----------

fn pk_color(c: Option<&str>) -> Option<String> {
    let c = c?.trim().trim_start_matches('#').to_ascii_lowercase();
    (c.len() == 6 && c.bytes().all(|b| b.is_ascii_hexdigit())).then(|| format!("#{c}"))
}

fn default_color(seed: &str) -> &'static str {
    let h = seed
        .bytes()
        .fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
    PALETTE[h % PALETTE.len()]
}

fn snapshot_of_local(m: &Member) -> Snapshot {
    Snapshot {
        name: m.name.clone(),
        pronouns: m.pronouns.clone(),
        color: Some(m.color.clone()),
    }
}

fn snapshot_of_remote(r: &PkMember) -> Snapshot {
    Snapshot {
        name: r.name.clone(),
        pronouns: r.pronouns.clone().filter(|p| !p.trim().is_empty()),
        color: pk_color(r.color.as_deref()),
    }
}

#[derive(PartialEq, Eq)]
enum Side {
    Same,
    Pull,
    Push,
}

/// Fusion à trois voies d'un champ. Changé des deux côtés : le local gagne.
fn merge<T: PartialEq + Clone>(local: &T, remote: &T, snap: &T) -> (T, Side) {
    if local == remote {
        (local.clone(), Side::Same)
    } else if local == snap {
        (remote.clone(), Side::Pull)
    } else {
        (local.clone(), Side::Push)
    }
}

// ---------- état ----------

pub fn status(db: &impl Db) -> Result<Status> {
    Ok(db.with(|c| {
        Ok(Status {
            enabled: kv::flag(c, K_ENABLED)?,
            system: kv::get(c, K_SYSTEM)?,
            last_sync: int(c, K_LAST_SYNC)?,
            last_error: kv::get(c, K_LAST_ERROR)?,
            pending: outbox::count(c, PK_SWITCH_TOPIC)?,
        })
    })?)
}

pub fn token(db: &impl Db) -> Result<Option<Zeroizing<String>>> {
    Ok(db.with(|c| Ok(kv::get(c, K_TOKEN)?.map(Zeroizing::new)))?)
}

/// Lie un système PluralKit : vérifie le jeton, synchronise les membres et aligne
/// le front sur le plus récent des deux côtés.
pub fn link(db: &impl Db, api: &impl PkApi, token: &str) -> Result<PkSystem> {
    let system = api.system()?;
    let now = now()?;
    db.with(|c| {
        kv::set(c, K_TOKEN, token.trim())?;
        kv::set(c, K_SYSTEM, &system.id)?;
        kv::set(c, K_LINKED_AT, &now.to_string())?;
        kv::delete(c, K_LAST_ERROR)?;
        Ok(())
    })?;
    sync_members(db, api)?;

    // Front initial : le plus récent l'emporte.
    if let Some(latest) = api.switches(1)?.into_iter().next() {
        let local_last = db.with(|c| {
            remember_switch(c, &latest.id, latest.timestamp)?;
            front::last_change(c)
        })?;
        if local_last.is_none_or(|l| latest.timestamp > l) {
            apply_remote_switch(db, &latest, now)?;
        }
    }
    // Activé en dernier : aucun switch local n'est mis en file pendant la liaison.
    db.with(|c| kv::set(c, K_ENABLED, "1"))?;
    tracing::info!("PluralKit lié");
    Ok(system)
}

/// Délie : oublie le jeton et les correspondances. Aucune donnée n'est supprimée, ni ici ni sur PluralKit.
pub fn unlink(db: &impl Db) -> Result<()> {
    db.with(|c| {
        let tx = c.transaction()?;
        for k in [
            K_ENABLED,
            K_TOKEN,
            K_SYSTEM,
            K_LINKED_AT,
            K_LAST_SYNC,
            K_LAST_ERROR,
        ] {
            kv::delete(&tx, k)?;
        }
        tx.execute("DELETE FROM pk_member", [])?;
        tx.execute("DELETE FROM pk_switch", [])?;
        tx.execute("DELETE FROM outbox WHERE topic = ?1", [PK_SWITCH_TOPIC])?;
        tx.commit()?;
        Ok(())
    })?;
    tracing::info!("PluralKit délié");
    Ok(())
}

fn now() -> Result<i64> {
    let t = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    i64::try_from(t).map_err(|_| RemoteError::Decode)
}

// ---------- membres ----------

struct PushPlan {
    link: Link,
    patch: PkMemberPatch,
}

/// Rapproche un membre déjà lié : fusion à trois voies, application locale
/// immédiate, et retour de ce qu'il faut écrire sur PluralKit.
fn merge_linked(
    tx: &Connection,
    local: &Member,
    r: &PkMember,
    link: &Link,
) -> ph_core::Result<(bool, Option<PushPlan>)> {
    let theirs = snapshot_of_remote(r);
    let snap = &link.snapshot;
    let (name, s_name) = merge(&local.name, &theirs.name, &snap.name);
    let (pronouns, s_pro) = merge(&local.pronouns, &theirs.pronouns, &snap.pronouns);
    // Couleur absente côté PluralKit : pas d'avis, on garde la nôtre.
    let their_color = theirs.color.clone().or_else(|| snap.color.clone());
    let (color, s_col) = merge(&Some(local.color.clone()), &their_color, &snap.color);
    let color = color.unwrap_or_else(|| local.color.clone());

    let pulled = [&s_name, &s_pro, &s_col].contains(&&Side::Pull);
    if pulled {
        members::update(
            tx,
            local.id,
            NewMember {
                name: name.clone(),
                pronouns: pronouns.clone(),
                color: color.clone(),
            },
        )?;
    }
    let patch = PkMemberPatch {
        name: (s_name == Side::Push).then(|| name.clone()),
        pronouns: (s_pro == Side::Push).then(|| pronouns.clone()),
        color: (s_col == Side::Push).then(|| color.trim_start_matches('#').to_owned()),
    };
    let mut next = link.clone();
    next.pk_hid.clone_from(&r.id);
    next.snapshot = Snapshot {
        name,
        pronouns,
        color: Some(color),
    };
    if patch.is_empty() {
        save_link(tx, &next)?;
        Ok((pulled, None))
    } else {
        Ok((pulled, Some(PushPlan { link: next, patch })))
    }
}

/// Crée ici un membre venu de PluralKit (ou adopte celui de même nom) et le lie.
fn adopt_remote(tx: &Connection, r: &PkMember, existing: Option<&Member>) -> ph_core::Result<Uuid> {
    let theirs = snapshot_of_remote(r);
    let fallback = existing.map_or_else(|| default_color(&r.uuid).to_owned(), |m| m.color.clone());
    let color = theirs.color.clone().unwrap_or(fallback);
    let input = NewMember {
        name: theirs.name.clone(),
        pronouns: theirs.pronouns.clone(),
        color: color.clone(),
    };
    let member_id = match existing {
        // Même nom des deux côtés à la liaison : PluralKit fait foi.
        Some(m) => members::update(tx, m.id, input)?.id,
        None => members::create(tx, input)?.id,
    };
    save_link(
        tx,
        &Link {
            member_id,
            pk_uuid: r.uuid.clone(),
            pk_hid: r.id.clone(),
            snapshot: Snapshot {
                color: Some(color),
                ..theirs
            },
        },
    )?;
    Ok(member_id)
}

struct Plan {
    pulled: usize,
    pushes: Vec<PushPlan>,
    creates: Vec<Member>,
}

/// Sous verrou, sans réseau : rapproche les deux listes, applique tout ce qui
/// est local, et prépare les écritures distantes.
fn reconcile(c: &mut Connection, remote: &[PkMember]) -> ph_core::Result<Plan> {
    let tx = c.transaction()?;
    let links = load_links(&tx)?;
    let locals: HashMap<Uuid, Member> =
        members::list(&tx)?.into_iter().map(|m| (m.id, m)).collect();
    let by_uuid: HashMap<&str, &Link> = links.iter().map(|l| (l.pk_uuid.as_str(), l)).collect();
    let mut linked: HashSet<Uuid> = links.iter().map(|l| l.member_id).collect();
    let mut by_name: HashMap<String, Uuid> = locals
        .values()
        .filter(|m| !linked.contains(&m.id))
        .map(|m| (m.name.trim().to_lowercase(), m.id))
        .collect();

    let mut plan = Plan {
        pulled: 0,
        pushes: Vec::new(),
        creates: Vec::new(),
    };
    for r in remote {
        if let Some(link) = by_uuid.get(r.uuid.as_str()) {
            // Archivé ici : on ne touche à rien.
            let Some(local) = locals.get(&link.member_id) else {
                continue;
            };
            let (pulled, push) = merge_linked(&tx, local, r, link)?;
            plan.pulled += usize::from(pulled);
            plan.pushes.extend(push);
        } else {
            let existing = by_name
                .remove(&r.name.trim().to_lowercase())
                .and_then(|id| locals.get(&id));
            linked.insert(adopt_remote(&tx, r, existing)?);
            plan.pulled += 1;
        }
    }

    // Supprimés sur PluralKit : on délie, on garde le membre ici.
    let remote_uuids: HashSet<&str> = remote.iter().map(|r| r.uuid.as_str()).collect();
    for l in links
        .iter()
        .filter(|l| !remote_uuids.contains(l.pk_uuid.as_str()))
    {
        tx.execute(
            "DELETE FROM pk_member WHERE member_id = ?1",
            [l.member_id.as_bytes()],
        )?;
    }

    plan.creates = locals
        .into_values()
        .filter(|m| !linked.contains(&m.id))
        .collect();
    tx.commit()?;
    Ok(plan)
}

pub fn sync_members(db: &impl Db, api: &impl PkApi) -> Result<(usize, usize)> {
    let remote = api.members()?;
    let plan = db.with(|c| reconcile(c, &remote))?;

    // Hors verrou : écrire sur PluralKit, puis enregistrer chaque succès.
    let mut written = 0;
    for p in plan.pushes {
        api.update_member(&p.link.pk_hid, &p.patch)?;
        db.with(|c| save_link(c, &p.link))?;
        written += 1;
    }
    for m in plan.creates {
        let made = api.create_member(&PkMemberPatch {
            name: Some(m.name.clone()),
            pronouns: m.pronouns.clone().map(Some),
            color: Some(m.color.trim_start_matches('#').to_owned()),
        })?;
        let link = Link {
            member_id: m.id,
            pk_uuid: made.uuid,
            pk_hid: made.id,
            snapshot: snapshot_of_local(&m),
        };
        db.with(|c| save_link(c, &link))?;
        written += 1;
    }
    Ok((plan.pulled, written))
}

// ---------- switchs ----------

fn hid_map(c: &Connection) -> ph_core::Result<HashMap<String, Uuid>> {
    Ok(load_links(c)?
        .into_iter()
        .map(|l| (l.pk_hid, l.member_id))
        .collect())
}

fn apply_remote_switch(db: &impl Db, sw: &PkSwitch, at: i64) -> Result<()> {
    db.with(|c| {
        let map = hid_map(c)?;
        let members: Vec<Uuid> = sw
            .members
            .iter()
            .filter_map(|h| map.get(h).copied())
            .collect();
        front::change_at(
            c,
            &FrontChange {
                members,
                blurry: false,
            },
            at,
            Origin::Remote,
        )?;
        Ok(())
    })?;
    Ok(())
}

/// Importe les switchs faits ailleurs (Discord, autre appli) depuis la liaison.
pub fn pull_switches(db: &impl Db, api: &impl PkApi) -> Result<usize> {
    let mut recent = api.switches(20)?;
    recent.reverse(); // du plus ancien au plus récent
    let list = recent;

    let unknown_member = db.with(|c| {
        let map = hid_map(c)?;
        Ok(list
            .iter()
            .flat_map(|s| &s.members)
            .any(|h| !map.contains_key(h)))
    })?;
    if unknown_member {
        sync_members(db, api)?;
    }

    let mut applied = 0;
    for sw in &list {
        let fresh = db.with(|c| {
            let new = remember_switch(c, &sw.id, sw.timestamp)?;
            let linked_at = int(c, K_LINKED_AT)?.unwrap_or(i64::MAX);
            let latest_local = front::last_change(c)?;
            Ok(new && sw.timestamp > linked_at && latest_local.is_none_or(|l| sw.timestamp > l))
        })?;
        if fresh {
            apply_remote_switch(db, sw, sw.timestamp)?;
            applied += 1;
        }
    }
    Ok(applied)
}

/// Envoie les switchs saisis ici, dans l'ordre. S'arrête à la première erreur
/// réseau pour préserver l'ordre ; une requête refusée (400) est abandonnée.
pub fn push_switches(db: &impl Db, api: &impl PkApi) -> Result<usize> {
    let mut sent = 0;
    let mut members_synced = false;
    loop {
        let items = db.with(|c| outbox::pending(c, PK_SWITCH_TOPIC, 20))?;
        if items.is_empty() {
            return Ok(sent);
        }
        for item in items {
            let Ok(p) = serde_json::from_str::<PendingSwitch>(&item.payload) else {
                db.with(|c| outbox::done(c, item.id))?;
                continue;
            };
            let lookup = || -> Result<(Vec<String>, bool)> {
                Ok(db.with(|c| {
                    let links = load_links(c)?;
                    let by_member: HashMap<Uuid, String> =
                        links.into_iter().map(|l| (l.member_id, l.pk_hid)).collect();
                    let ids: Vec<String> = p
                        .members
                        .iter()
                        .filter_map(|m| by_member.get(m).cloned())
                        .collect();
                    Ok((ids.clone(), ids.len() == p.members.len()))
                })?)
            };
            let (mut ids, complete) = lookup()?;
            if !complete && !members_synced {
                sync_members(db, api)?;
                members_synced = true;
                ids = lookup()?.0;
            }
            match api.post_switch(&ids, p.at) {
                Ok(sw) => {
                    db.with(|c| {
                        remember_switch(c, &sw.id, sw.timestamp)?;
                        outbox::done(c, item.id)
                    })?;
                    sent += 1;
                }
                Err(RemoteError::Http {
                    status: 400,
                    message,
                }) => {
                    tracing::warn!("switch refusé par PluralKit, abandonné : {message}");
                    db.with(|c| outbox::done(c, item.id))?;
                }
                Err(e) => {
                    let msg = e.to_string();
                    db.with(|c| outbox::failed(c, item.id, &msg))?;
                    return Err(e);
                }
            }
        }
    }
}

/// Un cycle complet. `with_members` : aussi la fusion des membres (plus coûteuse).
pub fn sync(db: &impl Db, api: &impl PkApi, with_members: bool) -> Result<SyncReport> {
    let run = || -> Result<SyncReport> {
        let mut report = SyncReport::default();
        if with_members {
            (report.members_pulled, report.members_pushed) = sync_members(db, api)?;
        }
        report.switches_pushed = push_switches(db, api)?;
        report.switches_pulled = pull_switches(db, api)?;
        Ok(report)
    };
    let result = run();
    let stamp = now()?;
    db.with(|c| {
        match &result {
            Ok(_) => {
                kv::set(c, K_LAST_SYNC, &stamp.to_string())?;
                kv::delete(c, K_LAST_ERROR)?;
            }
            Err(e) => kv::set(c, K_LAST_ERROR, e.code())?,
        }
        Ok(())
    })?;
    result
}
