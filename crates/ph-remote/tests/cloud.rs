#![allow(clippy::unwrap_used, clippy::expect_used)] // code de test
//! Synchro chiffrée entre deux appareils, contre un serveur simulé en mémoire.

use std::collections::BTreeMap;
use std::sync::Mutex;

use ph_core::keyfile::KdfParams;
use ph_core::members::{self, NewMember};
use ph_core::Vault;
use ph_remote::cloud::{self, Auth, CloudApi, Item, Keyset, Page, PairPoll, PairRequest, RemoteState};
use ph_remote::db::Db;
use ph_remote::{RemoteError, Result};
use zeroize::Zeroizing;

#[derive(Default)]
struct State {
    keyset: Option<Keyset>,
    items: BTreeMap<String, (String, i64)>,
    seq: i64,
    pairs: Vec<(String, String, Option<String>)>,
    next: u32,
    reject_token: u32,
    pushes: u32,
}

#[derive(Default)]
struct Fake(Mutex<State>);

impl Fake {
    fn st(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }
    fn check(&self, token: &str) -> Result<()> {
        let mut s = self.st();
        if s.reject_token > 0 || token != "tok" {
            s.reject_token = s.reject_token.saturating_sub(1);
            return Err(RemoteError::Unauthorized);
        }
        Ok(())
    }
}

impl CloudApi for Fake {
    fn state(&self, t: &str) -> Result<RemoteState> {
        self.check(t)?;
        let s = self.st();
        Ok(RemoteState { keyset: s.keyset.clone(), seq: s.seq })
    }
    fn put_keyset(&self, t: &str, k: &Keyset) -> Result<()> {
        self.check(t)?;
        let mut s = self.st();
        if s.keyset.is_some() {
            return Err(RemoteError::Conflict);
        }
        s.keyset = Some(k.clone());
        Ok(())
    }
    fn changes(&self, t: &str, since: i64, limit: usize) -> Result<Page> {
        self.check(t)?;
        let s = self.st();
        let mut all: Vec<_> = s.items.iter().filter(|(_, (_, q))| *q > since).collect();
        all.sort_by_key(|(_, (_, q))| *q);
        let more = all.len() > limit;
        all.truncate(limit);
        let seq = all.last().map_or(since, |(_, (_, q))| *q);
        let items = all.into_iter().map(|(k, (v, _))| Item { k: k.clone(), v: v.clone() }).collect();
        Ok(Page { items, seq, more })
    }
    fn push(&self, t: &str, items: &[Item]) -> Result<i64> {
        self.check(t)?;
        let mut s = self.st();
        assert!(s.keyset.is_some());
        s.pushes += 1;
        for i in items {
            s.seq += 1;
            let q = s.seq;
            s.items.insert(i.k.clone(), (i.v.clone(), q));
        }
        Ok(s.seq)
    }
    fn delete_account(&self, t: &str) -> Result<()> {
        self.check(t)?;
        let mut s = self.st();
        s.keyset = None;
        s.items.clear();
        s.seq = 0;
        Ok(())
    }
    fn pair_create(&self, t: &str, public: &str) -> Result<String> {
        self.check(t)?;
        let mut s = self.st();
        s.next += 1;
        let id = format!("00000000-0000-4000-8000-{:012}", s.next);
        s.pairs.push((id.clone(), public.to_owned(), None));
        Ok(id)
    }
    fn pair_list(&self, t: &str) -> Result<Vec<PairRequest>> {
        self.check(t)?;
        Ok(self
            .st()
            .pairs
            .iter()
            .filter(|p| p.2.is_none())
            .map(|p| PairRequest { id: p.0.clone(), public: p.1.clone(), expires_at: 0 })
            .collect())
    }
    fn pair_seal(&self, t: &str, id: &str, sealed: &str) -> Result<()> {
        self.check(t)?;
        let mut s = self.st();
        let p = s.pairs.iter_mut().find(|p| p.0 == id).ok_or(RemoteError::Core(ph_core::CoreError::NotFound))?;
        p.2 = Some(sealed.to_owned());
        Ok(())
    }
    fn pair_take(&self, t: &str, id: &str) -> Result<Option<String>> {
        self.check(t)?;
        let mut s = self.st();
        let i = s.pairs.iter().position(|p| p.0 == id).ok_or(RemoteError::Core(ph_core::CoreError::NotFound))?;
        let sealed = s.pairs[i].2.clone();
        if sealed.is_some() {
            s.pairs.remove(i);
        }
        Ok(sealed)
    }
    fn pair_cancel(&self, t: &str, id: &str) -> Result<()> {
        self.check(t)?;
        self.st().pairs.retain(|p| p.0 != id);
        Ok(())
    }
}

struct Tok;
impl Auth for Tok {
    fn token(&self) -> Result<Zeroizing<String>> {
        Ok(Zeroizing::new("tok".into()))
    }
    fn invalidate(&self) {}
}

fn vault() -> (tempfile::TempDir, Mutex<Vault>) {
    let tmp = tempfile::tempdir().unwrap();
    let mut v = Vault::new(tmp.path()).with_kdf(KdfParams::MINIMUM);
    v.create().unwrap();
    (tmp, Mutex::new(v))
}

fn add(db: &Mutex<Vault>, name: &str) -> uuid::Uuid {
    db.with(|c| members::create(c, NewMember { name: name.into(), pronouns: None, color: "#336699".into() }))
        .unwrap()
        .id
}

fn names(db: &Mutex<Vault>) -> Vec<String> {
    db.with(|c| members::list(c)).unwrap().into_iter().map(|m| m.name).collect()
}

#[test]
fn enable_then_join_with_recovery_and_sync_both_ways() {
    let api = Fake::default();
    let (_a, a) = vault();
    let (_b, b) = vault();
    add(&a, "Alix");
    assert_eq!(cloud::probe(&api, &Tok).unwrap(), cloud::Remote::Empty);
    let recovery = cloud::enable(&a, &api, &Tok).unwrap();
    assert!(matches!(cloud::enable(&b, &api, &Tok), Err(RemoteError::Conflict)));
    assert!(!cloud::status(&b).unwrap().enabled);

    let r = cloud::sync(&a, &api, &Tok).unwrap();
    assert!(r.pushed > 0);
    assert_eq!(cloud::status(&a).unwrap().pending, 0);

    // Rien en clair côté serveur.
    for (k, (v, _)) in &api.st().items {
        assert_eq!(k.len(), 64);
        assert!(!v.contains("Alix"));
        let raw = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, v).unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("Alix"));
    }

    assert!(cloud::join_with_recovery(&b, &api, &Tok, "AAAA-BBBB-CCCC-DDDD").is_err());
    cloud::join_with_recovery(&b, &api, &Tok, &recovery).unwrap();
    add(&b, "Bao");
    cloud::sync(&b, &api, &Tok).unwrap();
    assert_eq!(names(&b), ["Alix", "Bao"]);
    cloud::sync(&a, &api, &Tok).unwrap();
    assert_eq!(names(&a), ["Alix", "Bao"]);

    // Modification puis suppression qui se propagent.
    let id = a.with(|c| members::list(c)).unwrap()[0].id;
    a.with(|c| members::update(c, id, NewMember { name: "Alix R".into(), pronouns: None, color: "#336699".into() }))
        .unwrap();
    cloud::sync(&a, &api, &Tok).unwrap();
    cloud::sync(&b, &api, &Tok).unwrap();
    assert_eq!(names(&b), ["Alix R", "Bao"]);

    // Un cycle sans changement n'envoie rien.
    let before = api.st().pushes;
    let r = cloud::sync(&a, &api, &Tok).unwrap();
    assert_eq!(r.pushed, 0);
    assert_eq!(api.st().pushes, before);
}

#[test]
fn pairing_transfers_the_key() {
    let api = Fake::default();
    let (_a, a) = vault();
    let (_b, b) = vault();
    add(&a, "Alix");
    cloud::enable(&a, &api, &Tok).unwrap();
    cloud::sync(&a, &api, &Tok).unwrap();

    let fp = cloud::pair_request(&b, &api, &Tok).unwrap();
    assert_eq!(cloud::status(&b).unwrap().pairing.as_deref(), Some(fp.as_str()));
    assert_eq!(cloud::pair_poll(&b, &api, &Tok).unwrap(), PairPoll::Waiting);
    // L'appareil demandeur ne voit pas sa propre demande.
    assert!(cloud::pair_requests(&b, &api, &Tok).unwrap().is_empty());

    let reqs = cloud::pair_requests(&a, &api, &Tok).unwrap();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].fingerprint, fp);
    cloud::pair_approve(&a, &api, &Tok, &reqs[0].id).unwrap();

    assert_eq!(cloud::pair_poll(&b, &api, &Tok).unwrap(), PairPoll::Joined);
    assert!(cloud::status(&b).unwrap().enabled);
    assert!(cloud::status(&b).unwrap().pairing.is_none());
    cloud::sync(&b, &api, &Tok).unwrap();
    assert_eq!(names(&b), ["Alix"]);
}

#[test]
fn rejected_pairing_is_gone() {
    let api = Fake::default();
    let (_a, a) = vault();
    let (_b, b) = vault();
    cloud::enable(&a, &api, &Tok).unwrap();
    cloud::pair_request(&b, &api, &Tok).unwrap();
    let id = cloud::pair_requests(&a, &api, &Tok).unwrap()[0].id.clone();
    cloud::pair_reject(&api, &Tok, &id).unwrap();
    assert_eq!(cloud::pair_poll(&b, &api, &Tok).unwrap(), PairPoll::Gone);
    assert!(cloud::status(&b).unwrap().pairing.is_none());
}

#[test]
fn deleted_remote_unlinks_other_devices() {
    let api = Fake::default();
    let (_a, a) = vault();
    let (_b, b) = vault();
    add(&a, "Alix");
    let rec = cloud::enable(&a, &api, &Tok).unwrap();
    cloud::sync(&a, &api, &Tok).unwrap();
    cloud::join_with_recovery(&b, &api, &Tok, &rec).unwrap();
    cloud::sync(&b, &api, &Tok).unwrap();

    cloud::delete_remote(&a, &api, &Tok).unwrap();
    assert!(api.st().items.is_empty());
    assert!(!cloud::status(&a).unwrap().enabled);
    assert!(matches!(cloud::sync(&b, &api, &Tok), Err(RemoteError::NotLinked)));
    assert!(!cloud::status(&b).unwrap().enabled);
    // Les données locales restent.
    assert_eq!(names(&b), ["Alix"]);
}

#[test]
fn retries_once_after_refused_token_and_records_errors() {
    let api = Fake::default();
    let (_a, a) = vault();
    cloud::enable(&a, &api, &Tok).unwrap();
    api.st().reject_token = 1;
    cloud::sync(&a, &api, &Tok).unwrap();
    api.st().reject_token = 5;
    assert!(cloud::sync(&a, &api, &Tok).is_err());
    assert_eq!(cloud::status(&a).unwrap().last_error.as_deref(), Some("remote_unauthorized"));
    api.st().reject_token = 0;
    cloud::sync(&a, &api, &Tok).unwrap();
    let st = cloud::status(&a).unwrap();
    assert!(st.last_error.is_none());
    assert!(st.last_sync.is_some());
}

#[test]
fn tampered_or_moved_items_are_ignored() {
    let api = Fake::default();
    let (_a, a) = vault();
    let (_b, b) = vault();
    add(&a, "Alix");
    add(&a, "Bao");
    let rec = cloud::enable(&a, &api, &Tok).unwrap();
    cloud::sync(&a, &api, &Tok).unwrap();
    {
        let mut s = api.st();
        let keys: Vec<String> = s.items.keys().cloned().collect();
        // Échange des contenus de deux éléments : chacun est rejeté.
        let (v0, q0) = s.items[&keys[0]].clone();
        let (v1, q1) = s.items[&keys[1]].clone();
        s.items.insert(keys[0].clone(), (v1, q0));
        s.items.insert(keys[1].clone(), (v0, q1));
    }
    cloud::join_with_recovery(&b, &api, &Tok, &rec).unwrap();
    cloud::sync(&b, &api, &Tok).unwrap();
    assert!(names(&b).len() < 2);
}
