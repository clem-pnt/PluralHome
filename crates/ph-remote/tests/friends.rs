#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::many_single_char_names
)] // code de test
//! Amis entre deux systèmes, contre un serveur d'amis simulé en mémoire.

use std::collections::BTreeMap;
use std::sync::Mutex;

use ph_core::Vault;
use ph_core::keyfile::KdfParams;
use ph_core::members::{self, NewMember};
use ph_core::messages::{self, NewMessage};
use ph_remote::cloud::Auth;
use ph_remote::db::Db;
use ph_remote::friends::{
    self, FriendsApi, MailItem, MailPage, RemoteBlock, RemoteIdentity, RemoteIncoming,
    RemoteInvite, RemoteOutgoing, RemoteRequests, WireFriend,
};
use ph_remote::{RemoteError, Result};
use uuid::Uuid;
use zeroize::Zeroizing;

struct Invite {
    owner: String,
    card: String,
}
struct Request {
    from: String,
    to: String,
    card: String,
}
struct Friendship {
    a: String,
    b: String,
    cards: BTreeMap<String, String>,
}

#[derive(Default)]
struct State {
    idents: BTreeMap<String, (String, bool)>,
    invites: BTreeMap<String, Invite>,
    requests: BTreeMap<String, Request>,
    friends: BTreeMap<String, Friendship>,
    blocks: BTreeMap<String, (String, String)>,
    mail: Vec<(i64, String, String, String)>,
    seq: i64,
    at: i64,
}

#[derive(Default)]
struct Fake(Mutex<State>);

fn new_id() -> String {
    Uuid::now_v7().to_string()
}

impl Fake {
    fn st(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }
}

fn other<'a>(f: &'a Friendship, me: &str) -> &'a str {
    if f.a == me { &f.b } else { &f.a }
}

fn refused(code: &'static str) -> RemoteError {
    RemoteError::Friend(code)
}

impl FriendsApi for Fake {
    fn identity(&self, t: &str) -> Result<RemoteIdentity> {
        let s = self.st();
        Ok(s.idents.get(t).map_or(
            RemoteIdentity {
                public: None,
                open: true,
            },
            |(p, o)| RemoteIdentity {
                public: Some(p.clone()),
                open: *o,
            },
        ))
    }
    fn put_identity(&self, t: &str, public: &str, replace: bool) -> Result<()> {
        let mut s = self.st();
        if s.idents.contains_key(t) && !replace {
            return Err(refused("friend_identity_conflict"));
        }
        if replace {
            s.friends.retain(|_, f| f.a != t && f.b != t);
        }
        s.idents.insert(t.into(), (public.into(), true));
        Ok(())
    }
    fn set_open(&self, t: &str, open: bool) -> Result<()> {
        self.st().idents.get_mut(t).unwrap().1 = open;
        Ok(())
    }
    fn invites(&self, t: &str) -> Result<Vec<RemoteInvite>> {
        let s = self.st();
        Ok(s.invites
            .iter()
            .filter(|(_, i)| i.owner == t)
            .map(|(id, _)| RemoteInvite {
                id: id.clone(),
                once: false,
                expires_at: None,
                created_at: 1,
            })
            .collect())
    }
    fn invite_create(&self, t: &str, card: &str, _: Option<i64>, _: bool) -> Result<String> {
        let id = new_id();
        self.st().invites.insert(
            id.clone(),
            Invite {
                owner: t.into(),
                card: card.into(),
            },
        );
        Ok(id)
    }
    fn invite_read(&self, _: &str, id: &str) -> Result<String> {
        let s = self.st();
        let i = s.invites.get(id).ok_or(refused("friend_invite_invalid"))?;
        if !s.idents[&i.owner].1 {
            return Err(refused("friend_invite_invalid"));
        }
        Ok(i.card.clone())
    }
    fn invite_revoke(&self, _: &str, id: &str) -> Result<()> {
        self.st().invites.remove(id);
        Ok(())
    }
    fn requests(&self, t: &str) -> Result<RemoteRequests> {
        let s = self.st();
        let mut out = RemoteRequests::default();
        for (id, r) in &s.requests {
            if r.to == t {
                out.incoming.push(RemoteIncoming {
                    id: id.clone(),
                    public: s.idents[&r.from].0.clone(),
                    card: r.card.clone(),
                    created_at: 1,
                });
            } else if r.from == t {
                out.outgoing.push(RemoteOutgoing {
                    id: id.clone(),
                    created_at: 1,
                });
            }
        }
        Ok(out)
    }
    fn request_send(&self, t: &str, invite: &str, card: &str) -> Result<String> {
        let mut s = self.st();
        let to = s
            .invites
            .get(invite)
            .ok_or(refused("friend_invite_invalid"))?
            .owner
            .clone();
        if s.blocks.values().any(|(a, b)| a == &to && b == t) {
            return Err(refused("friend_blocked"));
        }
        if s.friends
            .values()
            .any(|f| (f.a == t && f.b == to) || (f.b == t && f.a == to))
        {
            return Err(refused("friend_already"));
        }
        let id = new_id();
        s.requests.insert(
            id.clone(),
            Request {
                from: t.into(),
                to,
                card: card.into(),
            },
        );
        Ok(id)
    }
    fn request_accept(&self, t: &str, id: &str, card: &str) -> Result<String> {
        let mut s = self.st();
        let r = s
            .requests
            .remove(id)
            .filter(|r| r.to == t)
            .ok_or(refused("friend_request_gone"))?;
        let mut cards = BTreeMap::new();
        // Fiche lisible par `to` (envoyée par `from`), et inversement.
        cards.insert(r.from.clone(), r.card);
        cards.insert(t.to_owned(), card.into());
        let fid = new_id();
        s.friends.insert(
            fid.clone(),
            Friendship {
                a: r.from,
                b: t.into(),
                cards,
            },
        );
        Ok(fid)
    }
    fn request_block(&self, t: &str, id: &str) -> Result<String> {
        let mut s = self.st();
        let r = s
            .requests
            .remove(id)
            .ok_or(refused("friend_request_gone"))?;
        let b = new_id();
        s.blocks.insert(b.clone(), (t.into(), r.from));
        Ok(b)
    }
    fn request_drop(&self, _: &str, id: &str) -> Result<()> {
        self.st().requests.remove(id);
        Ok(())
    }
    fn friends(&self, t: &str) -> Result<Vec<WireFriend>> {
        let mut s = self.st();
        s.at += 1;
        let s = &*s;
        Ok(s.friends
            .iter()
            .filter(|(_, f)| f.a == t || f.b == t)
            .map(|(id, f)| {
                let o = other(f, t);
                WireFriend {
                    id: id.clone(),
                    public: s.idents[o].0.clone(),
                    card: f.cards.get(o).cloned(),
                    card_at: f.cards.get(o).map(|c| i64::try_from(c.len()).unwrap()),
                    created_at: 1,
                }
            })
            .collect())
    }
    fn put_card(&self, t: &str, id: &str, card: &str) -> Result<()> {
        let mut s = self.st();
        let f = s.friends.get_mut(id).ok_or(refused("friend_gone"))?;
        f.cards.insert(t.into(), card.into());
        Ok(())
    }
    fn mail_send(&self, t: &str, id: &str, items: &[String]) -> Result<()> {
        let mut s = self.st();
        let f = s.friends.get(id).ok_or(refused("friend_gone"))?;
        let to = other(f, t).to_owned();
        for v in items {
            s.seq += 1;
            let seq = s.seq;
            s.mail.push((seq, to.clone(), id.into(), v.clone()));
        }
        Ok(())
    }
    fn block_friend(&self, t: &str, id: &str) -> Result<String> {
        let mut s = self.st();
        let f = s.friends.remove(id).ok_or(refused("friend_gone"))?;
        let b = new_id();
        let o = other(&f, t).to_owned();
        s.blocks.insert(b.clone(), (t.into(), o));
        Ok(b)
    }
    fn unfriend(&self, _: &str, id: &str) -> Result<()> {
        self.st().friends.remove(id);
        Ok(())
    }
    fn blocks(&self, t: &str) -> Result<Vec<RemoteBlock>> {
        Ok(self
            .st()
            .blocks
            .iter()
            .filter(|(_, (by, _))| by == t)
            .map(|(id, _)| RemoteBlock {
                id: id.clone(),
                created_at: 1,
            })
            .collect())
    }
    fn unblock(&self, _: &str, id: &str) -> Result<()> {
        self.st().blocks.remove(id);
        Ok(())
    }
    fn mail(&self, t: &str, after: i64) -> Result<MailPage> {
        let s = self.st();
        let mine: Vec<_> = s
            .mail
            .iter()
            .filter(|(q, to, ..)| to == t && *q > after)
            .collect();
        let more = mine.len() > 2;
        Ok(MailPage {
            items: mine
                .into_iter()
                .take(2)
                .map(|(q, _, f, v)| MailItem {
                    s: *q,
                    friend: f.clone(),
                    v: v.clone(),
                })
                .collect(),
            more,
        })
    }
    fn mail_ack(&self, t: &str, up_to: i64) -> Result<()> {
        self.st()
            .mail
            .retain(|(q, to, ..)| !(to == t && *q <= up_to));
        Ok(())
    }
}

/// Le jeton sert aussi d'identifiant de compte côté serveur simulé.
struct Tok(&'static str);
impl Auth for Tok {
    fn token(&self) -> Result<Zeroizing<String>> {
        Ok(Zeroizing::new(self.0.into()))
    }
    fn invalidate(&self) {}
}

fn vault() -> (tempfile::TempDir, Mutex<Vault>) {
    let tmp = tempfile::tempdir().unwrap();
    let mut v = Vault::new(tmp.path()).with_kdf(KdfParams::MINIMUM);
    v.create().unwrap();
    (tmp, Mutex::new(v))
}

fn add(db: &Mutex<Vault>, name: &str) -> Uuid {
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
    .unwrap()
    .id
}

fn befriend(api: &Fake, a: &Mutex<Vault>, b: &Mutex<Vault>) -> String {
    let (ta, tb) = (Tok("a"), Tok("b"));
    let inv = friends::create_invite(a, api, &ta, Some(24 * 3_600_000), false).unwrap();
    let link = inv.link.unwrap();
    let preview = friends::preview_invite(b, api, &tb, &format!("tiens : {link} !")).unwrap();
    assert!(!preview.own && !preview.already);
    let req = friends::send_request(b, api, &tb, &link).unwrap();
    let pending = friends::requests(a, api, &ta).unwrap();
    assert_eq!(pending.incoming.len(), 1);
    assert_eq!(pending.incoming[0].code, preview.code);
    assert!(pending.incoming[0].members.is_some());
    let out = friends::requests(b, api, &tb).unwrap();
    assert_eq!(out.outgoing[0].id, req);
    assert_eq!(out.outgoing[0].code.as_deref(), Some(preview.code.as_str()));
    let fid = friends::accept(a, api, &ta, &req).unwrap();
    let rb = friends::sync(b, api, &tb).unwrap();
    assert!(rb.changed);
    fid
}

#[test]
fn invite_request_accept_then_talk() {
    let api = Fake::default();
    let (_a, a) = vault();
    let (_b, b) = vault();
    let (ta, tb) = (Tok("a"), Tok("b"));
    let mila = add(&a, "Mila");
    let bob = add(&b, "Bob");
    let fid = befriend(&api, &a, &b);

    let fa = a.with(|c| ph_core::friends::list(c)).unwrap();
    let fb = b.with(|c| ph_core::friends::list(c)).unwrap();
    assert_eq!(fa.len(), 1);
    assert_eq!(fa[0].id, fid);
    assert_eq!(fa[0].code, fb[0].code);
    assert_eq!(fb[0].members.len(), 1);
    assert_eq!(fb[0].members[0].name, "Mila");

    // Le lien ne sert plus à rien entre amis.
    let link = friends::invites(&a, &api, &ta).unwrap()[0]
        .link
        .clone()
        .unwrap();
    assert!(
        friends::preview_invite(&b, &api, &tb, &link)
            .unwrap()
            .already
    );
    assert!(matches!(
        friends::send_request(&a, &api, &ta, &link),
        Err(RemoteError::Friend("friend_self"))
    ));

    // Trois messages de Mila vers tout le système de Bob (relève paginée).
    for n in 0..3 {
        a.with(|c| {
            messages::post(
                c,
                &NewMessage {
                    author_ids: vec![mila],
                    body: format!("coucou {n}"),
                    friend_id: Some(fid.clone()),
                    ..NewMessage::default()
                },
            )
        })
        .unwrap();
    }
    let ra = friends::sync(&a, &api, &ta).unwrap();
    assert_eq!(ra.sent, 3);
    let rb = friends::sync(&b, &api, &tb).unwrap();
    assert_eq!(rb.received, 3);
    assert!(api.st().mail.is_empty());
    let got = b.with(|c| messages::list(c, None, 50, false)).unwrap();
    assert_eq!(got.len(), 3);
    assert!(
        got.iter()
            .all(|m| !m.outgoing && m.friend_id.as_deref() == Some(&fid))
    );
    assert_eq!(got[0].remote_authors[0].name, "Mila");

    // Réponse de Bob.
    b.with(|c| {
        messages::post(
            c,
            &NewMessage {
                author_ids: vec![bob],
                body: "salut".into(),
                friend_id: Some(fid.clone()),
                ..NewMessage::default()
            },
        )
    })
    .unwrap();
    friends::sync(&b, &api, &tb).unwrap();
    assert_eq!(friends::sync(&a, &api, &ta).unwrap().received, 1);
    // Rien de neuf : pas de changement signalé.
    assert_eq!(
        friends::sync(&a, &api, &ta).unwrap(),
        friends::FriendsReport::default()
    );
}

#[test]
fn closed_profile_and_block() {
    let api = Fake::default();
    let (_a, a) = vault();
    let (_b, b) = vault();
    let (ta, tb) = (Tok("a"), Tok("b"));
    add(&a, "Mila");
    let inv = friends::create_invite(&a, &api, &ta, None, false).unwrap();
    let link = inv.link.unwrap();
    friends::set_open(&a, &api, &ta, false).unwrap();
    assert!(!friends::settings(&a, &api, &ta).unwrap().open);
    assert!(matches!(
        friends::preview_invite(&b, &api, &tb, &link),
        Err(RemoteError::Friend("friend_invite_invalid"))
    ));
    friends::set_open(&a, &api, &ta, true).unwrap();
    let req = friends::send_request(&b, &api, &tb, &link).unwrap();
    friends::block_request(&a, &api, &ta, &req, Some("Bob")).unwrap();
    let blocked = friends::blocks(&a, &api, &ta).unwrap();
    assert_eq!(blocked[0].name.as_deref(), Some("Bob"));
    assert!(matches!(
        friends::send_request(&b, &api, &tb, &link),
        Err(RemoteError::Friend("friend_blocked"))
    ));
    friends::unblock(&a, &api, &ta, &blocked[0].id).unwrap();
    assert!(friends::blocks(&a, &api, &ta).unwrap().is_empty());
    friends::send_request(&b, &api, &tb, &link).unwrap();
}

#[test]
fn unfriend_removes_on_both_sides() {
    let api = Fake::default();
    let (_a, a) = vault();
    let (_b, b) = vault();
    add(&a, "Mila");
    let fid = befriend(&api, &a, &b);
    friends::unfriend(&a, &api, &Tok("a"), &fid).unwrap();
    assert!(a.with(|c| ph_core::friends::list(c)).unwrap().is_empty());
    assert!(friends::sync(&b, &api, &Tok("b")).unwrap().changed);
    assert!(b.with(|c| ph_core::friends::list(c)).unwrap().is_empty());
}

#[test]
fn identity_conflict_then_take_over() {
    let api = Fake::default();
    let (_a, a) = vault();
    let (_a2, a2) = vault();
    let (_b, b) = vault();
    add(&a, "Mila");
    befriend(&api, &a, &b);
    // Autre appareil du même compte, sans les données : conflit.
    assert!(matches!(
        friends::sync(&a2, &api, &Tok("a")),
        Err(RemoteError::Friend("friend_identity_conflict"))
    ));
    friends::take_over_identity(&a2, &api, &Tok("a")).unwrap();
    friends::sync(&a2, &api, &Tok("a")).unwrap();
    assert!(friends::sync(&b, &api, &Tok("b")).unwrap().changed);
    assert!(b.with(|c| ph_core::friends::list(c)).unwrap().is_empty());
}

#[test]
fn links_are_parsed_leniently() {
    let id = Uuid::now_v7().to_string();
    let secret = "0123456789abcdef0123456789abcdef";
    let text = format!("Rejoins-nous\npluralhome://ami/{id}/{secret}\nà bientôt");
    let (got, s) = friends::parse_link(&text).unwrap();
    assert_eq!(got, id);
    assert_eq!(s.to_hex(), secret);
    assert!(friends::parse_link("pluralhome://ami/pas-un-id/0123").is_err());
    assert!(friends::parse_link("rien").is_err());
}
