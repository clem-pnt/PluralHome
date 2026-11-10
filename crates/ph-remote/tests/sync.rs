#![allow(clippy::unwrap_used, clippy::expect_used)] // code de test
//! Synchro contre un PluralKit simulé en mémoire.

use std::sync::Mutex;

use ph_core::front::{self, FrontChange};
use ph_core::keyfile::KdfParams;
use ph_core::members::{self, NewMember};
use ph_core::{Vault, outbox};
use ph_remote::db::Db;
use ph_remote::pluralkit::{PkApi, PkMember, PkMemberPatch, PkSwitch, PkSystem};
use ph_remote::sync;
use ph_remote::{RemoteError, Result};
use uuid::Uuid;

#[derive(Default)]
struct State {
    members: Vec<PkMember>,
    switches: Vec<PkSwitch>, // du plus ancien au plus récent
    next: u32,
    offline: bool,
    writes: u32,
}

#[derive(Default)]
struct FakePk(Mutex<State>);

impl FakePk {
    fn st(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }
    fn add_member(&self, name: &str, pronouns: Option<&str>, color: Option<&str>) -> String {
        let mut s = self.st();
        s.next += 1;
        let id = format!("m{:04}", s.next);
        s.members.push(PkMember {
            id: id.clone(),
            uuid: format!("uuid-{id}"),
            name: name.into(),
            pronouns: pronouns.map(Into::into),
            color: color.map(Into::into),
        });
        id
    }
    fn remote_switch(&self, members: &[&str], ts: i64) {
        let mut s = self.st();
        s.next += 1;
        let id = format!("sw{}", s.next);
        s.switches.push(PkSwitch {
            id,
            timestamp: ts,
            members: members.iter().map(|m| (*m).to_owned()).collect(),
        });
    }
    fn member(&self, id: &str) -> PkMember {
        self.st()
            .members
            .iter()
            .find(|m| m.id == id)
            .unwrap()
            .clone()
    }
}

impl PkApi for FakePk {
    fn system(&self) -> Result<PkSystem> {
        Ok(PkSystem {
            id: "sysid".into(),
            uuid: "sys-uuid".into(),
            name: Some("Le système".into()),
        })
    }
    fn members(&self) -> Result<Vec<PkMember>> {
        let s = self.st();
        if s.offline {
            return Err(RemoteError::Network("hors ligne".into()));
        }
        Ok(s.members.clone())
    }
    fn create_member(&self, p: &PkMemberPatch) -> Result<PkMember> {
        let id = self.add_member(
            p.name.as_deref().unwrap(),
            p.pronouns.clone().flatten().as_deref(),
            p.color.as_deref(),
        );
        self.st().writes += 1;
        Ok(self.member(&id))
    }
    fn update_member(&self, id: &str, p: &PkMemberPatch) -> Result<PkMember> {
        let mut s = self.st();
        s.writes += 1;
        let m = s.members.iter_mut().find(|m| m.id == id).unwrap();
        if let Some(n) = &p.name {
            m.name.clone_from(n);
        }
        if let Some(pr) = &p.pronouns {
            m.pronouns.clone_from(pr);
        }
        if let Some(c) = &p.color {
            m.color = Some(c.clone());
        }
        Ok(m.clone())
    }
    fn switches(&self, limit: u32) -> Result<Vec<PkSwitch>> {
        let s = self.st();
        if s.offline {
            return Err(RemoteError::Network("hors ligne".into()));
        }
        Ok(s.switches
            .iter()
            .rev()
            .take(limit as usize)
            .cloned()
            .collect())
    }
    fn post_switch(&self, members: &[String], ts: i64) -> Result<PkSwitch> {
        let mut s = self.st();
        if s.offline {
            return Err(RemoteError::Network("hors ligne".into()));
        }
        s.next += 1;
        s.writes += 1;
        let sw = PkSwitch {
            id: format!("sw{}", s.next),
            timestamp: ts,
            members: members.to_vec(),
        };
        s.switches.push(sw.clone());
        Ok(sw)
    }
}

fn vault() -> (tempfile::TempDir, Mutex<Vault>) {
    let tmp = tempfile::tempdir().unwrap();
    let mut v = Vault::new(tmp.path()).with_kdf(KdfParams::MINIMUM);
    v.create("phrase secrete longue").unwrap();
    (tmp, Mutex::new(v))
}

fn local(db: &Mutex<Vault>, name: &str) -> Uuid {
    db.with(|c| {
        members::create(
            c,
            NewMember {
                name: name.into(),
                pronouns: None,
                color: "#4f8583".into(),
            },
        )
    })
    .unwrap()
    .id
}

fn names(db: &Mutex<Vault>) -> Vec<String> {
    let mut v: Vec<String> = db
        .with(|c| members::list(c))
        .unwrap()
        .into_iter()
        .map(|m| m.name)
        .collect();
    v.sort();
    v
}

fn fronting(db: &Mutex<Vault>) -> Vec<Option<Uuid>> {
    db.with(|c| front::current(c))
        .unwrap()
        .into_iter()
        .map(|s| s.member_id)
        .collect()
}

fn now() -> i64 {
    i64::try_from(time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000).unwrap()
}

#[test]
fn link_merges_members_both_ways() {
    let (_t, db) = vault();
    let pk = FakePk::default();
    pk.add_member("Mila", Some("elle"), Some("c8643b"));
    pk.add_member("Sacha", None, None);
    let lou = local(&db, "Lou");
    local(&db, "mila"); // même nom, casse différente

    sync::link(&db, &pk, "jeton").unwrap();

    assert_eq!(
        names(&db),
        vec!["Lou", "Mila", "Sacha"],
        "Mila rapprochée, Sacha importée"
    );
    let mila = db
        .with(|c| members::list(c))
        .unwrap()
        .into_iter()
        .find(|m| m.name == "Mila")
        .unwrap();
    assert_eq!(
        (mila.pronouns.as_deref(), mila.color.as_str()),
        (Some("elle"), "#c8643b"),
        "PluralKit fait foi à la liaison"
    );
    let remote: Vec<String> = pk.st().members.iter().map(|m| m.name.clone()).collect();
    assert!(
        remote.contains(&"Lou".to_owned()),
        "Lou créée sur PluralKit"
    );
    assert_eq!(remote.len(), 3);
    let _ = lou;

    // Une seconde passe ne fait rien.
    let writes = pk.st().writes;
    assert_eq!(sync::sync_members(&db, &pk).unwrap(), (0, 0));
    assert_eq!(pk.st().writes, writes);
}

#[test]
fn three_way_merge_per_field() {
    let (_t, db) = vault();
    let pk = FakePk::default();
    let hid = pk.add_member("Mila", Some("elle"), Some("4f8583"));
    sync::link(&db, &pk, "jeton").unwrap();
    let id = db.with(|c| members::list(c)).unwrap()[0].id;

    // Renommée sur Discord, pronoms changés ici.
    pk.st().members[0].name = "Mila R.".into();
    db.with(|c| {
        members::update(
            c,
            id,
            NewMember {
                name: "Mila".into(),
                pronouns: Some("iel".into()),
                color: "#4f8583".into(),
            },
        )
    })
    .unwrap();
    assert_eq!(sync::sync_members(&db, &pk).unwrap(), (1, 1));
    let here = db.with(|c| members::get(c, id)).unwrap();
    let there = pk.member(&hid);
    assert_eq!(
        (here.name.as_str(), here.pronouns.as_deref()),
        ("Mila R.", Some("iel"))
    );
    assert_eq!(
        (there.name.as_str(), there.pronouns.as_deref()),
        ("Mila R.", Some("iel"))
    );

    // Conflit sur le même champ : le local gagne.
    pk.st().members[0].pronouns = Some("elle/iel".into());
    db.with(|c| {
        members::update(
            c,
            id,
            NewMember {
                name: "Mila R.".into(),
                pronouns: Some("ille".into()),
                color: "#4f8583".into(),
            },
        )
    })
    .unwrap();
    sync::sync_members(&db, &pk).unwrap();
    assert_eq!(pk.member(&hid).pronouns.as_deref(), Some("ille"));
    assert_eq!(
        db.with(|c| members::get(c, id))
            .unwrap()
            .pronouns
            .as_deref(),
        Some("ille")
    );
}

#[test]
fn remote_deletion_only_unlinks() {
    let (_t, db) = vault();
    let pk = FakePk::default();
    pk.add_member("Mila", None, None);
    sync::link(&db, &pk, "jeton").unwrap();
    pk.st().members.clear();
    sync::sync_members(&db, &pk).unwrap();
    assert_eq!(names(&db), vec!["Mila"], "rien n'est effacé ici");
}

#[test]
fn switches_flow_both_ways_without_echo() {
    let (_t, db) = vault();
    let pk = FakePk::default();
    let a = pk.add_member("Mila", None, None);
    let b = pk.add_member("Sacha", None, None);
    sync::link(&db, &pk, "jeton").unwrap();
    let ids: std::collections::HashMap<String, Uuid> = db
        .with(|c| members::list(c))
        .unwrap()
        .into_iter()
        .map(|m| (m.name, m.id))
        .collect();

    // Switch fait sur Discord.
    pk.remote_switch(&[&a, &b], now() + 1_000);
    assert_eq!(sync::pull_switches(&db, &pk).unwrap(), 1);
    assert_eq!(fronting(&db).len(), 2);
    assert_eq!(sync::pull_switches(&db, &pk).unwrap(), 0, "déjà appliqué");

    // Switch fait ici : part sur PluralKit, ne revient pas.
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    db.with(|c| {
        front::change(
            c,
            &FrontChange {
                members: vec![ids["Sacha"]],
                blurry: false,
            },
        )
    })
    .unwrap();
    assert_eq!(sync::push_switches(&db, &pk).unwrap(), 1);
    assert_eq!(pk.st().switches.last().unwrap().members, vec![b.clone()]);
    assert_eq!(sync::pull_switches(&db, &pk).unwrap(), 0, "pas d'écho");
    assert_eq!(fronting(&db), vec![Some(ids["Sacha"])]);

    // Le flou part comme « personne ».
    db.with(|c| {
        front::change(
            c,
            &FrontChange {
                members: vec![],
                blurry: true,
            },
        )
    })
    .unwrap();
    sync::push_switches(&db, &pk).unwrap();
    assert!(pk.st().switches.last().unwrap().members.is_empty());
    assert_eq!(fronting(&db), vec![None], "le flou reste ici");
}

#[test]
fn older_remote_switch_never_overrides_local() {
    let (_t, db) = vault();
    let pk = FakePk::default();
    let a = pk.add_member("Mila", None, None);
    sync::link(&db, &pk, "jeton").unwrap();
    let start = now();
    db.with(|c| {
        front::change(
            c,
            &FrontChange {
                members: vec![],
                blurry: true,
            },
        )
    })
    .unwrap();
    pk.remote_switch(&[&a], start - 60_000);
    assert_eq!(sync::pull_switches(&db, &pk).unwrap(), 0);
    assert_eq!(fronting(&db), vec![None]);
}

#[test]
fn offline_keeps_queue_in_order() {
    let (_t, db) = vault();
    let pk = FakePk::default();
    pk.add_member("Mila", None, None);
    pk.add_member("Sacha", None, None);
    sync::link(&db, &pk, "jeton").unwrap();
    let ids: Vec<Uuid> = db
        .with(|c| members::list(c))
        .unwrap()
        .into_iter()
        .map(|m| m.id)
        .collect();

    pk.st().offline = true;
    db.with(|c| {
        front::change(
            c,
            &FrontChange {
                members: vec![ids[0]],
                blurry: false,
            },
        )
    })
    .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    db.with(|c| {
        front::change(
            c,
            &FrontChange {
                members: vec![ids[1]],
                blurry: false,
            },
        )
    })
    .unwrap();
    assert!(sync::sync(&db, &pk, false).is_err());
    assert_eq!(
        db.with(|c| outbox::count(c, front::PK_SWITCH_TOPIC))
            .unwrap(),
        2,
        "rien de perdu"
    );
    assert_eq!(
        sync::status(&db).unwrap().last_error.as_deref(),
        Some("remote_network")
    );

    pk.st().offline = false;
    let report = sync::sync(&db, &pk, false).unwrap();
    assert_eq!(report.switches_pushed, 2);
    let sws = pk.st().switches.clone();
    assert!(
        sws[sws.len() - 2].timestamp < sws[sws.len() - 1].timestamp,
        "ordre préservé"
    );
    assert_eq!(sync::status(&db).unwrap().last_error, None);
}

#[test]
fn unlink_forgets_everything_but_data() {
    let (_t, db) = vault();
    let pk = FakePk::default();
    pk.add_member("Mila", None, None);
    sync::link(&db, &pk, "jeton").unwrap();
    sync::unlink(&db).unwrap();
    assert!(sync::token(&db).unwrap().is_none());
    assert!(!sync::status(&db).unwrap().enabled);
    assert_eq!(names(&db), vec!["Mila"]);
    // Plus rien n'est mis en file une fois délié.
    db.with(|c| {
        front::change(
            c,
            &FrontChange {
                members: vec![],
                blurry: true,
            },
        )
    })
    .unwrap();
    assert_eq!(
        db.with(|c| outbox::count(c, front::PK_SWITCH_TOPIC))
            .unwrap(),
        0
    );
}
