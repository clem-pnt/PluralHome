//! Catalogue de rôles propre au système (hôte, protecteur… ou tout autre libellé
//! qu'il se donne). Partagé entre appareils ; les rôles portés par chaque membre
//! restent dans son profil, et suivent les renommages et suppressions faits ici.

use rusqlite::{Connection, params};

use crate::members::MAX_ROLE;
use crate::{CoreError, Result, time};

pub const MAX_CATALOG: usize = 100;

fn clean(name: &str) -> Result<String> {
    let name = name.trim().to_owned();
    if !(1..=MAX_ROLE).contains(&name.chars().count()) {
        return Err(CoreError::Invalid("role"));
    }
    Ok(name)
}

/// Rôles du catalogue, par ordre alphabétique.
pub fn list(conn: &Connection) -> Result<Vec<String>> {
    let mut st = conn.prepare_cached("SELECT name FROM role ORDER BY name COLLATE NOCASE")?;
    let rows = st.query_map([], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Nom déjà présent dans le catalogue, à la casse près.
fn existing(conn: &Connection, name: &str) -> Result<Option<String>> {
    let lower = name.to_lowercase();
    Ok(list(conn)?.into_iter().find(|r| r.to_lowercase() == lower))
}

/// Ajoute un rôle (sans effet s'il existe déjà, à la casse près) et renvoie le catalogue.
pub fn add(conn: &Connection, name: &str) -> Result<Vec<String>> {
    let name = clean(name)?;
    if existing(conn, &name)?.is_none() {
        if list(conn)?.len() >= MAX_CATALOG {
            return Err(CoreError::Invalid("roles"));
        }
        conn.execute(
            "INSERT OR IGNORE INTO role (name, created_at) VALUES (?1, ?2)",
            params![name, time::now_ms()?],
        )?;
    }
    list(conn)
}

/// Réécrit les rôles de chaque membre : `to = None` retire le rôle.
fn rewrite_members(conn: &Connection, from: &str, to: Option<&str>) -> Result<()> {
    let rows: Vec<(Vec<u8>, String)> = conn
        .prepare("SELECT member_id, roles FROM member_profile")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (id, raw) in rows {
        let roles: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
        if !roles.iter().any(|r| r == from) {
            continue;
        }
        let mut next: Vec<String> = Vec::with_capacity(roles.len());
        for r in roles {
            let r = if r == from {
                match to {
                    Some(t) => t.to_owned(),
                    None => continue,
                }
            } else {
                r
            };
            if !next.iter().any(|x| x.to_lowercase() == r.to_lowercase()) {
                next.push(r);
            }
        }
        let json = serde_json::to_string(&next).map_err(|_| CoreError::Invalid("roles"))?;
        conn.execute(
            "UPDATE member_profile SET roles = ?1 WHERE member_id = ?2",
            params![json, id],
        )?;
    }
    Ok(())
}

/// Renomme un rôle, dans le catalogue et chez tous les membres qui le portent.
/// Vers un nom déjà pris, les deux rôles fusionnent.
pub fn rename(conn: &mut Connection, from: &str, to: &str) -> Result<Vec<String>> {
    let to = clean(to)?;
    let tx = conn.transaction()?;
    let target = existing(&tx, &to)?.filter(|t| t != from).unwrap_or(to);
    if target != from {
        let created: Option<i64> = tx
            .query_row("SELECT created_at FROM role WHERE name = ?1", [from], |r| {
                r.get(0)
            })
            .ok();
        tx.execute("DELETE FROM role WHERE name = ?1", [from])?;
        tx.execute(
            "INSERT OR IGNORE INTO role (name, created_at) VALUES (?1, ?2)",
            params![target, match created { Some(t) => t, None => time::now_ms()? }],
        )?;
        rewrite_members(&tx, from, Some(&target))?;
    }
    tx.commit()?;
    list(conn)
}

/// Supprime un rôle du catalogue et le retire à tous les membres.
pub fn remove(conn: &mut Connection, name: &str) -> Result<Vec<String>> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM role WHERE name = ?1", [name])?;
    rewrite_members(&tx, name, None)?;
    tx.commit()?;
    list(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::members::{self, AvatarEdit, NewMember, Profile};

    fn db() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        crate::migrations::run(&mut c).unwrap();
        c
    }

    fn member(c: &Connection, name: &str, roles: &[&str]) -> uuid::Uuid {
        let m = members::create(
            c,
            NewMember {
                name: name.into(),
                pronouns: None,
                color: "#4f8583".into(),
            },
        )
        .unwrap();
        members::set_profile(
            c,
            m.id,
            Profile {
                description: None,
                roles: roles.iter().map(|r| (*r).to_owned()).collect(),
                avatar: AvatarEdit::Keep,
            },
        )
        .unwrap();
        m.id
    }

    #[test]
    fn add_ignores_case_duplicates_and_validates() {
        let c = db();
        assert_eq!(add(&c, " Gardien·ne ").unwrap(), vec!["Gardien·ne"]);
        assert_eq!(add(&c, "gardien·ne").unwrap(), vec!["Gardien·ne"]);
        assert!(add(&c, "   ").is_err());
        assert!(add(&c, &"x".repeat(MAX_ROLE + 1)).is_err());
    }

    #[test]
    fn rename_and_remove_follow_members() {
        let mut c = db();
        let a = member(&c, "Mila", &["Soignant", "Hôte"]);
        let b = member(&c, "Noé", &["Soignant"]);
        add(&c, "Soignant").unwrap();
        add(&c, "Hôte").unwrap();

        rename(&mut c, "Soignant", "Soin").unwrap();
        assert_eq!(list(&c).unwrap(), vec!["Hôte", "Soin"]);
        assert_eq!(members::get(&c, a).unwrap().roles, vec!["Soin", "Hôte"]);
        assert_eq!(members::get(&c, b).unwrap().roles, vec!["Soin"]);

        // Vers un nom existant : fusion, sans doublon chez le membre.
        rename(&mut c, "Soin", "hôte").unwrap();
        assert_eq!(list(&c).unwrap(), vec!["Hôte"]);
        assert_eq!(members::get(&c, a).unwrap().roles, vec!["Hôte"]);
        assert_eq!(members::get(&c, b).unwrap().roles, vec!["Hôte"]);

        remove(&mut c, "Hôte").unwrap();
        assert!(list(&c).unwrap().is_empty());
        assert!(members::get(&c, a).unwrap().roles.is_empty());
    }
}
