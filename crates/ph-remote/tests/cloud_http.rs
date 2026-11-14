#![allow(clippy::unwrap_used, clippy::expect_used)] // code de test
//! Même scénario que `cloud.rs`, mais contre le vrai serveur Node (`server/test/dev-server.mjs`).
//! Lancer : `node server/test/dev-server.mjs &` puis
//! `PLURALHOME_SYNC_URL=http://127.0.0.1:3191 cargo test -p ph-remote --test cloud_http -- --ignored`.

use std::sync::Mutex;

use ph_core::Vault;
use ph_core::keyfile::KdfParams;
use ph_core::members::{self, NewMember};
use ph_remote::cloud::{self, Auth, HttpCloud, PairPoll};
use ph_remote::{RemoteError, Result};
use zeroize::Zeroizing;

struct Tok(String);
impl Auth for Tok {
    fn token(&self) -> Result<Zeroizing<String>> {
        Ok(Zeroizing::new(self.0.clone()))
    }
    fn invalidate(&self) {}
}

fn tok() -> Tok {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    Tok(format!("compte-de-test-{n}"))
}

fn vault() -> (tempfile::TempDir, Mutex<Vault>) {
    let tmp = tempfile::tempdir().unwrap();
    let mut v = Vault::new(tmp.path()).with_kdf(KdfParams::MINIMUM);
    v.create().unwrap();
    (tmp, Mutex::new(v))
}

fn add(db: &Mutex<Vault>, name: &str) {
    use ph_remote::db::Db;
    db.with(|c| {
        members::create(
            c,
            NewMember {
                name: name.into(),
                pronouns: None,
                color: "#336699".into(),
            },
        )
    })
    .unwrap();
}

fn names(db: &Mutex<Vault>) -> Vec<String> {
    use ph_remote::db::Db;
    db.with(|c| members::list(c))
        .unwrap()
        .into_iter()
        .map(|m| m.name)
        .collect()
}

#[test]
#[ignore = "demande le serveur Node de dev"]
fn full_cycle_against_node_server() {
    let api = HttpCloud::new().unwrap();
    let t = tok();
    let (_a, a) = vault();
    let (_b, b) = vault();
    let (_c, c) = vault();
    add(&a, "Alix");

    assert_eq!(cloud::probe(&api, &t).unwrap(), cloud::Remote::Empty);
    let recovery = cloud::enable(&a, &api, &t).unwrap();
    assert!(matches!(
        cloud::enable(&b, &api, &t),
        Err(RemoteError::Conflict)
    ));
    assert!(cloud::sync(&a, &api, &t).unwrap().pushed > 0);

    // Clé de secours.
    assert!(cloud::join_with_recovery(&b, &api, &t, "AAAA-BBBB-CCCC-DDDD").is_err());
    cloud::join_with_recovery(&b, &api, &t, &recovery).unwrap();
    add(&b, "Bao");
    cloud::sync(&b, &api, &t).unwrap();
    cloud::sync(&a, &api, &t).unwrap();
    assert_eq!(names(&a), ["Alix", "Bao"]);

    // Appairage.
    let fp = cloud::pair_request(&c, &api, &t).unwrap();
    assert_eq!(cloud::pair_poll(&c, &api, &t).unwrap(), PairPoll::Waiting);
    let reqs = cloud::pair_requests(&a, &api, &t).unwrap();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].fingerprint, fp);
    cloud::pair_approve(&a, &api, &t, &reqs[0].id).unwrap();
    assert_eq!(cloud::pair_poll(&c, &api, &t).unwrap(), PairPoll::Joined);
    cloud::sync(&c, &api, &t).unwrap();
    assert_eq!(names(&c), ["Alix", "Bao"]);

    // Refus.
    let (_d, d) = vault();
    cloud::pair_request(&d, &api, &t).unwrap();
    let id = cloud::pair_requests(&a, &api, &t).unwrap()[0].id.clone();
    cloud::pair_reject(&api, &t, &id).unwrap();
    assert_eq!(cloud::pair_poll(&d, &api, &t).unwrap(), PairPoll::Gone);

    // Suppression côté serveur.
    cloud::delete_remote(&a, &api, &t).unwrap();
    assert!(matches!(
        cloud::sync(&b, &api, &t),
        Err(RemoteError::NotLinked)
    ));
    assert_eq!(names(&b), ["Alix", "Bao"]);
    assert_eq!(cloud::probe(&api, &t).unwrap(), cloud::Remote::Empty);
}

#[test]
#[ignore = "demande le serveur Node de dev"]
fn bad_token_is_unauthorized() {
    let api = HttpCloud::new().unwrap();
    assert!(matches!(
        cloud::probe(&api, &Tok("court".into())),
        Err(RemoteError::Unauthorized)
    ));
}
