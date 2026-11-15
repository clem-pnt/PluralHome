//! Dossiers pour ranger les membres. Un membre peut être dans plusieurs dossiers
//! (ou aucun) et reste toujours dans la liste de tous les membres. Partagés entre
//! appareils ; rien n'en part vers PluralKit. Un dossier peut en contenir d'autres
//! (« Âge » → « Enfants », « Ados »…).

use std::collections::HashMap;

use rusqlite::{Connection, params};
use serde::Serialize;
use uuid::Uuid;

use crate::{CoreError, Result, time};

pub const MAX_FOLDER_NAME: usize = 60;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Folder {
    pub id: Uuid,
    pub name: String,
    /// Dossier parent ; `None` à la racine.
    pub parent_id: Option<Uuid>,
    /// Membres rangés ici (archivés exclus).
    pub members: Vec<Uuid>,
}

fn clean(name: &str) -> Result<String> {
    let name = name.trim().to_owned();
    if !(1..=MAX_FOLDER_NAME).contains(&name.chars().count()) {
        return Err(CoreError::Invalid("folder"));
    }
    Ok(name)
}

fn uuid(bytes: &[u8]) -> rusqlite::Result<Uuid> {
    Uuid::from_slice(bytes).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e))
    })
}

/// Dossiers par ordre alphabétique, avec leurs membres.
pub fn list(conn: &Connection) -> Result<Vec<Folder>> {
    let mut folders: Vec<Folder> = conn
        .prepare_cached(
            "SELECT id, name, parent_id FROM folder ORDER BY name COLLATE NOCASE, created_at",
        )?
        .query_map([], |r| {
            Ok(Folder {
                id: uuid(&r.get::<_, Vec<u8>>(0)?)?,
                name: r.get(1)?,
                parent_id: r
                    .get::<_, Option<Vec<u8>>>(2)?
                    .map(|b| uuid(&b))
                    .transpose()?,
                members: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let index: HashMap<Uuid, usize> = folders.iter().enumerate().map(|(i, f)| (f.id, i)).collect();
    let rows: Vec<(Vec<u8>, Vec<u8>)> = conn
        .prepare_cached(
            "SELECT fm.folder_id, fm.member_id FROM folder_member fm
             JOIN member m ON m.id = fm.member_id
             WHERE m.archived_at IS NULL ORDER BY m.name COLLATE NOCASE",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (f, m) in rows {
        if let Some(&i) = index.get(&uuid(&f)?) {
            folders[i].members.push(uuid(&m)?);
        }
    }
    Ok(folders)
}

fn exists(conn: &Connection, id: Uuid) -> Result<()> {
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM folder WHERE id = ?1",
        [id.as_bytes()],
        |r| r.get(0),
    )?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

pub fn create(conn: &Connection, name: &str, parent: Option<Uuid>) -> Result<Vec<Folder>> {
    let name = clean(name)?;
    if let Some(p) = parent {
        exists(conn, p)?;
    }
    conn.execute(
        "INSERT INTO folder (id, name, parent_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            Uuid::now_v7().as_bytes(),
            name,
            parent.map(|p| *p.as_bytes()),
            time::now_ms()?
        ],
    )?;
    list(conn)
}

/// Déplace un dossier (avec ce qu'il contient) sous un autre, ou à la racine.
/// Refuse de le ranger dans lui-même ou dans un de ses sous-dossiers.
pub fn move_to(conn: &Connection, id: Uuid, parent: Option<Uuid>) -> Result<Vec<Folder>> {
    exists(conn, id)?;
    if let Some(p) = parent {
        exists(conn, p)?;
        let cycle: i64 = conn.query_row(
            "WITH RECURSIVE up(id, n) AS (
                SELECT ?1, 0
                UNION SELECT f.parent_id, up.n + 1 FROM folder f JOIN up ON f.id = up.id
                WHERE f.parent_id IS NOT NULL AND up.n < 1000
            ) SELECT count(*) FROM up WHERE id = ?2",
            params![p.as_bytes(), id.as_bytes()],
            |r| r.get(0),
        )?;
        if cycle > 0 {
            return Err(CoreError::Invalid("folder_parent"));
        }
    }
    conn.execute(
        "UPDATE folder SET parent_id = ?2 WHERE id = ?1",
        params![id.as_bytes(), parent.map(|p| *p.as_bytes())],
    )?;
    list(conn)
}

pub fn rename(conn: &Connection, id: Uuid, name: &str) -> Result<Vec<Folder>> {
    let name = clean(name)?;
    let n = conn.execute(
        "UPDATE folder SET name = ?2 WHERE id = ?1",
        params![id.as_bytes(), name],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    list(conn)
}

/// Supprime le dossier et ses sous-dossiers ; les membres restent, simplement
/// plus rangés là.
pub fn remove(conn: &Connection, id: Uuid) -> Result<Vec<Folder>> {
    conn.execute("DELETE FROM folder WHERE id = ?1", [id.as_bytes()])?;
    list(conn)
}

/// Remplace le contenu d'un dossier.
pub fn set_members(conn: &mut Connection, id: Uuid, members: &[Uuid]) -> Result<Vec<Folder>> {
    let tx = conn.transaction()?;
    exists(&tx, id)?;
    let keep: Vec<&[u8]> = members.iter().map(|m| m.as_bytes().as_slice()).collect();
    let current: Vec<Vec<u8>> = tx
        .prepare("SELECT member_id FROM folder_member WHERE folder_id = ?1")?
        .query_map([id.as_bytes()], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for m in &current {
        if !keep.contains(&m.as_slice()) {
            tx.execute(
                "DELETE FROM folder_member WHERE folder_id = ?1 AND member_id = ?2",
                params![id.as_bytes(), m],
            )?;
        }
    }
    for m in members {
        add_one(&tx, id, *m)?;
    }
    tx.commit()?;
    list(conn)
}

/// Remplace les dossiers d'un membre.
pub fn set_for_member(
    conn: &mut Connection,
    member: Uuid,
    folders: &[Uuid],
) -> Result<Vec<Folder>> {
    let tx = conn.transaction()?;
    let keep: Vec<&[u8]> = folders.iter().map(|f| f.as_bytes().as_slice()).collect();
    let current: Vec<Vec<u8>> = tx
        .prepare("SELECT folder_id FROM folder_member WHERE member_id = ?1")?
        .query_map([member.as_bytes()], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for f in &current {
        if !keep.contains(&f.as_slice()) {
            tx.execute(
                "DELETE FROM folder_member WHERE folder_id = ?1 AND member_id = ?2",
                params![f, member.as_bytes()],
            )?;
        }
    }
    for f in folders {
        exists(&tx, *f)?;
        add_one(&tx, *f, member)?;
    }
    tx.commit()?;
    list(conn)
}

fn add_one(conn: &Connection, folder: Uuid, member: Uuid) -> Result<()> {
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM member WHERE id = ?1 AND archived_at IS NULL",
        [member.as_bytes()],
        |r| r.get(0),
    )?;
    if n == 0 {
        return Err(CoreError::NotFound);
    }
    conn.execute(
        "INSERT OR IGNORE INTO folder_member (folder_id, member_id) VALUES (?1, ?2)",
        params![folder.as_bytes(), member.as_bytes()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::members::{self, NewMember};

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
                color: "#4f8583".into(),
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn folders_hold_members_without_removing_them() {
        let mut c = db();
        let a = member(&c, "Mila");
        let b = member(&c, "Alex");
        let fs = create(&c, " Petits ", None).unwrap();
        assert_eq!(fs[0].name, "Petits");
        let id = fs[0].id;
        let fs = set_members(&mut c, id, &[a, b]).unwrap();
        assert_eq!(fs[0].members, vec![b, a]);
        create(&c, "Protecteurs", None).unwrap();
        let prot = list(&c).unwrap()[1].id;
        let fs = set_for_member(&mut c, a, &[prot]).unwrap();
        assert_eq!(fs[0].members, vec![b]);
        assert_eq!(fs[1].members, vec![a]);
        // Les membres restent dans la liste générale.
        assert_eq!(members::list(&c).unwrap().len(), 2);
        let fs = rename(&c, id, "Les petits").unwrap();
        assert_eq!(fs[0].name, "Les petits");
        let fs = remove(&c, id).unwrap();
        assert_eq!(fs.len(), 1);
        assert_eq!(members::list(&c).unwrap().len(), 2);
        assert!(create(&c, "  ", None).is_err());
        assert!(matches!(
            set_members(&mut c, id, &[a]),
            Err(CoreError::NotFound)
        ));
    }

    #[test]
    fn archived_members_are_hidden() {
        let mut c = db();
        let a = member(&c, "Mila");
        let id = create(&c, "Petits", None).unwrap()[0].id;
        set_members(&mut c, id, &[a]).unwrap();
        members::archive(&mut c, a).unwrap();
        assert!(list(&c).unwrap()[0].members.is_empty());
    }

    #[test]
    fn subfolders_nest_without_cycles() {
        let c = db();
        let age = create(&c, "Âge", None).unwrap()[0].id;
        let fs = create(&c, "Enfants", Some(age)).unwrap();
        let kids = fs.iter().find(|f| f.name == "Enfants").unwrap().id;
        assert_eq!(
            fs.iter().find(|f| f.id == kids).unwrap().parent_id,
            Some(age)
        );
        let small = create(&c, "Tout-petits", Some(kids)).unwrap();
        let small = small.iter().find(|f| f.name == "Tout-petits").unwrap().id;
        assert!(matches!(
            move_to(&c, age, Some(small)),
            Err(CoreError::Invalid("folder_parent"))
        ));
        assert!(matches!(
            move_to(&c, age, Some(age)),
            Err(CoreError::Invalid("folder_parent"))
        ));
        let fs = move_to(&c, small, None).unwrap();
        assert_eq!(fs.iter().find(|f| f.id == small).unwrap().parent_id, None);
        // Supprimer un parent emporte ses sous-dossiers.
        let fs = remove(&c, age).unwrap();
        assert_eq!(fs.len(), 1);
        assert!(create(&c, "X", Some(age)).is_err());
    }
}
