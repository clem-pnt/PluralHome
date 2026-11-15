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
    /// Photos servies par adresse ; une adresse absente répond 404.
    files: std::collections::HashMap<String, Vec<u8>>,
    fetches: u32,
    files_down: bool,
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
            avatar_url: None,
            description: None,
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
        let mut s = self.st();
        s.writes += 1;
        if let Some(d) = &p.description {
            s.members.last_mut().unwrap().description.clone_from(d);
        }
        drop(s);
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
        if let Some(d) = &p.description {
            m.description.clone_from(d);
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
    fn fetch_avatar(&self, url: &str) -> Result<Vec<u8>> {
        let mut s = self.st();
        s.fetches += 1;
        if s.files_down {
            return Err(RemoteError::Network("hors ligne".into()));
        }
        s.files.get(url).cloned().ok_or(RemoteError::Http {
            status: 404,
            message: String::new(),
        })
    }
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        w,
        h,
        image::Rgb([90, 120, 200]),
    ));
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

fn avatar_of(db: &Mutex<Vault>, name: &str) -> Option<String> {
    db.with(|c| members::list(c))
        .unwrap()
        .into_iter()
        .find(|m| m.name == name)
        .unwrap()
        .avatar
}

fn vault() -> (tempfile::TempDir, Mutex<Vault>) {
    let tmp = tempfile::tempdir().unwrap();
    let mut v = Vault::new(tmp.path()).with_kdf(KdfParams::MINIMUM);
    v.create().unwrap();
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

#[test]
fn avatars_imported_once_never_over_local() {
    let (_t, db) = vault();
    let pk = FakePk::default();
    for (name, url) in [
        ("Mila", "https://cdn/m.png"),
        ("Sacha", "https://cdn/s.png"),
        ("Lou", "https://cdn/l.png"),
        ("Noé", "https://cdn/absent.png"),
    ] {
        let hid = pk.add_member(name, None, None);
        let mut s = pk.st();
        s.members
            .iter_mut()
            .find(|m| m.id == hid)
            .unwrap()
            .avatar_url = Some(url.into());
        s.files.insert(url.into(), png(400, 300));
    }
    pk.st().files.remove("https://cdn/absent.png");
    // Lou a déjà une photo ici : elle la garde.
    let lou = local(&db, "Lou");
    let mine = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(png(8, 8))
    };
    db.with(|c| {
        members::set_profile(
            c,
            lou,
            members::Profile {
                description: Some("ici".into()),
                roles: vec![],
                avatar: members::AvatarEdit::Set { data: mine.clone() },
            },
        )
    })
    .unwrap();
    pk.st().files_down = true;

    // Photos injoignables à la liaison : rien d'importé, nouvel essai plus tard.
    sync::link(&db, &pk, "jeton").unwrap();
    assert!(avatar_of(&db, "Mila").is_none());
    pk.st().files_down = false;

    let (pulled, _) = sync::sync_members(&db, &pk).unwrap();
    assert_eq!(pulled, 2, "Mila et Sacha");
    let mila = avatar_of(&db, "Mila").unwrap();
    assert!(mila.starts_with("data:image/jpeg;base64,"));
    assert!(avatar_of(&db, "Sacha").is_some());
    assert!(avatar_of(&db, "Noé").is_none(), "404 : écartée");
    assert!(
        avatar_of(&db, "Lou").unwrap().ends_with(&mine),
        "photo locale intacte"
    );
    let lou_desc = db.with(|c| members::get(c, lou)).unwrap().description;
    assert_eq!(lou_desc.as_deref(), Some("ici"));

    // Une seule tentative par adresse, même après retrait de la photo ici.
    let id = db
        .with(|c| members::list(c))
        .unwrap()
        .into_iter()
        .find(|m| m.name == "Mila")
        .unwrap()
        .id;
    db.with(|c| {
        members::set_profile(
            c,
            id,
            members::Profile {
                description: None,
                roles: vec![],
                avatar: members::AvatarEdit::Remove,
            },
        )
    })
    .unwrap();
    let fetches = pk.st().fetches;
    assert_eq!(sync::sync_members(&db, &pk).unwrap(), (0, 0));
    assert_eq!(
        pk.st().fetches,
        fetches,
        "ni Mila ni Noé ne sont redemandées"
    );
    assert!(avatar_of(&db, "Mila").is_none());

    // Nouvelle photo sur PluralKit : importée.
    {
        let mut s = pk.st();
        s.members
            .iter_mut()
            .find(|m| m.name == "Mila")
            .unwrap()
            .avatar_url = Some("https://cdn/m2.png".into());
        let bytes = png(50, 80);
        s.files.insert("https://cdn/m2.png".into(), bytes);
    }
    assert_eq!(sync::sync_members(&db, &pk).unwrap().0, 1);
    assert!(avatar_of(&db, "Mila").is_some());
    assert_eq!(pk.st().writes, 0, "rien n'est écrit sur PluralKit");
}

fn description_of(db: &Mutex<Vault>, id: Uuid) -> Option<String> {
    db.with(|c| members::get(c, id)).unwrap().description
}

fn describe(db: &Mutex<Vault>, id: Uuid, d: &str) {
    db.with(|c| members::set_description(c, id, Some(d)))
        .unwrap();
}

#[test]
fn descriptions_only_with_the_option() {
    let (_t, db) = vault();
    let pk = FakePk::default();
    let hid = pk.add_member("Mila", None, None);
    pk.st().members[0].description = Some("Venue de Discord".into());
    let lou = local(&db, "Lou");
    describe(&db, lou, "Protège le système");

    sync::link(&db, &pk, "jeton").unwrap();
    let mila = db
        .with(|c| members::list(c))
        .unwrap()
        .into_iter()
        .find(|m| m.name == "Mila")
        .unwrap()
        .id;
    assert_eq!(
        description_of(&db, mila),
        None,
        "option coupée : rien ne bouge"
    );
    let lou_pk = pk
        .st()
        .members
        .iter()
        .find(|m| m.name == "Lou")
        .unwrap()
        .id
        .clone();
    assert_eq!(pk.member(&lou_pk).description, None);
    assert!(!sync::status(&db).unwrap().sync_description);

    // Option activée : premier contact, chaque description vide prend l'autre.
    sync::set_sync_description(&db, true).unwrap();
    assert!(sync::status(&db).unwrap().sync_description);
    sync::sync_members(&db, &pk).unwrap();
    assert_eq!(
        description_of(&db, mila).as_deref(),
        Some("Venue de Discord")
    );
    assert_eq!(
        pk.member(&lou_pk).description.as_deref(),
        Some("Protège le système")
    );

    // Ensuite, fusion à trois voies classique.
    pk.st().members[0].description = Some("Modifiée sur Discord".into());
    describe(&db, lou, "Modifiée ici");
    sync::sync_members(&db, &pk).unwrap();
    assert_eq!(
        description_of(&db, mila).as_deref(),
        Some("Modifiée sur Discord")
    );
    assert_eq!(
        pk.member(&lou_pk).description.as_deref(),
        Some("Modifiée ici")
    );

    // Effacée ici : effacée là-bas.
    db.with(|c| members::set_description(c, lou, None)).unwrap();
    sync::sync_members(&db, &pk).unwrap();
    assert_eq!(pk.member(&lou_pk).description, None);

    // Trop longue pour PluralKit : reste ici, rien n'est envoyé.
    describe(&db, mila, &"x".repeat(1200));
    let writes = pk.st().writes;
    sync::sync_members(&db, &pk).unwrap();
    assert_eq!(pk.st().writes, writes);
    assert_eq!(
        pk.member(&hid).description.as_deref(),
        Some("Modifiée sur Discord")
    );
    assert_eq!(description_of(&db, mila).unwrap().len(), 1200);

    // Une passe de plus ne fait rien.
    sync::sync_members(&db, &pk).unwrap();
    assert_eq!(pk.st().writes, writes);
}

#[test]
fn new_member_carries_its_description() {
    let (_t, db) = vault();
    let pk = FakePk::default();
    sync::link(&db, &pk, "jeton").unwrap();
    sync::set_sync_description(&db, true).unwrap();
    let lou = local(&db, "Lou");
    describe(&db, lou, "Arrivée récemment");
    sync::sync_members(&db, &pk).unwrap();
    assert_eq!(
        pk.st().members[0].description.as_deref(),
        Some("Arrivée récemment")
    );
    let writes = pk.st().writes;
    sync::sync_members(&db, &pk).unwrap();
    assert_eq!(pk.st().writes, writes, "pas de renvoi");
}
