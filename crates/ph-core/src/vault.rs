//! Coffre : cycle de vie création / déverrouillage / verrouillage.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::keyfile::{Dek, KdfParams, KeyFile};
use crate::{CoreError, Result, migrations};

const KEYFILE: &str = "keyfile.json";
const DATABASE: &str = "pluralhome.db";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum VaultStatus {
    Uninitialized,
    Locked,
    Unlocked,
}

pub struct Vault {
    dir: PathBuf,
    kdf: KdfParams,
    conn: Option<Connection>,
}

impl Vault {
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            kdf: KdfParams::DEFAULT,
            conn: None,
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub fn with_kdf(mut self, kdf: KdfParams) -> Self {
        self.kdf = kdf;
        self
    }

    fn keyfile_path(&self) -> PathBuf {
        self.dir.join(KEYFILE)
    }

    #[must_use]
    pub fn status(&self) -> VaultStatus {
        if self.conn.is_some() {
            VaultStatus::Unlocked
        } else if self.keyfile_path().exists() {
            VaultStatus::Locked
        } else {
            VaultStatus::Uninitialized
        }
    }

    pub fn create(&mut self, passphrase: &str) -> Result<()> {
        if self.status() != VaultStatus::Uninitialized {
            return Err(CoreError::AlreadyInitialized);
        }
        create_private_dir(&self.dir)?;
        let (kf, dek) = KeyFile::create(passphrase, self.kdf)?;
        // La base d'abord : si on s'interrompt ici, aucun fichier de clé n'existe
        // et le coffre reste « non créé ». Une base orpheline est écrasée.
        let _ = fs::remove_file(self.dir.join(DATABASE));
        let conn = open_db(&self.dir.join(DATABASE), &dek)?;
        write_atomic(
            &self.keyfile_path(),
            &serde_json::to_vec_pretty(&kf).map_err(|_| CoreError::Crypto)?,
        )?;
        self.conn = Some(conn);
        tracing::info!("coffre créé");
        Ok(())
    }

    pub fn unlock(&mut self, passphrase: &str) -> Result<()> {
        match self.status() {
            VaultStatus::Uninitialized => return Err(CoreError::NotInitialized),
            VaultStatus::Unlocked => return Ok(()),
            VaultStatus::Locked => {}
        }
        let raw = fs::read(self.keyfile_path())?;
        let kf: KeyFile = serde_json::from_slice(&raw).map_err(|_| CoreError::CorruptKeyFile)?;
        let dek = kf.unwrap_dek(passphrase)?;
        self.conn = Some(open_db(&self.dir.join(DATABASE), &dek)?);
        tracing::info!("coffre déverrouillé");
        Ok(())
    }

    pub fn change_passphrase(&mut self, current: &str, next: &str) -> Result<()> {
        let raw = fs::read(self.keyfile_path())?;
        let kf: KeyFile = serde_json::from_slice(&raw).map_err(|_| CoreError::CorruptKeyFile)?;
        let dek = kf.unwrap_dek(current)?;
        let next_kf = KeyFile::wrap(next, self.kdf, &dek)?;
        write_atomic(
            &self.keyfile_path(),
            &serde_json::to_vec_pretty(&next_kf).map_err(|_| CoreError::Crypto)?,
        )?;
        tracing::info!("phrase secrète changée");
        Ok(())
    }

    /// Ferme la connexion : `SQLCipher` efface sa copie de la clé.
    pub fn lock(&mut self) {
        if self.conn.take().is_some() {
            tracing::info!("coffre verrouillé");
        }
    }

    pub fn conn(&self) -> Result<&Connection> {
        self.conn.as_ref().ok_or(CoreError::Locked)
    }
}

fn open_db(path: &Path, dek: &Dek) -> Result<Connection> {
    let mut conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let key = zeroize::Zeroizing::new(format!("x'{}'", hex::encode(dek.as_slice())));
    conn.pragma_update(None, "key", key.as_str())?;
    // Vérifie la clé avant toute autre chose (SQLCipher ne la valide qu'à la première lecture).
    conn.query_row("SELECT count(*) FROM sqlite_schema", [], |_| Ok(()))
        .map_err(|_| CoreError::BadPassphrase)?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA secure_delete = ON;
         PRAGMA temp_store = MEMORY;",
    )?;
    migrations::run(&mut conn)?;
    restrict(path)?;
    Ok(conn)
}

#[cfg(unix)]
fn restrict(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}
#[cfg(not(unix))]
fn restrict(_: &Path) -> Result<()> {
    Ok(())
}

fn create_private_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Écriture atomique : fichier temporaire, fsync, renommage, fsync du dossier.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    restrict(&tmp)?;
    fs::rename(&tmp, path)?;
    if let Some(parent) = path.parent() {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::members::{self, NewMember};

    fn vault(dir: &Path) -> Vault {
        Vault::new(dir).with_kdf(KdfParams::TEST)
    }

    #[test]
    fn full_lifecycle() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        assert_eq!(v.status(), VaultStatus::Uninitialized);
        v.create("phrase secrete longue").unwrap();
        members::create(
            v.conn().unwrap(),
            NewMember {
                name: " Mila ".into(),
                pronouns: Some("elle".into()),
                color: "#4F8583".into(),
            },
        )
        .unwrap();
        v.lock();
        assert_eq!(v.status(), VaultStatus::Locked);
        assert!(matches!(v.conn(), Err(CoreError::Locked)));

        let mut v = vault(tmp.path());
        assert!(matches!(
            v.unlock("mauvaise phrase"),
            Err(CoreError::BadPassphrase)
        ));
        v.unlock("phrase secrete longue").unwrap();
        let list = members::list(v.conn().unwrap()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Mila");
        assert_eq!(list[0].color, "#4f8583");
    }

    #[test]
    fn database_is_encrypted_at_rest() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        v.create("phrase secrete longue").unwrap();
        members::create(
            v.conn().unwrap(),
            NewMember {
                name: "Sacha-Marqueur".into(),
                pronouns: None,
                color: "#000000".into(),
            },
        )
        .unwrap();
        v.lock();
        let raw = fs::read(tmp.path().join(DATABASE)).unwrap();
        assert!(!raw.windows(6).any(|w| w == b"SQLite"));
        assert!(!raw.windows(14).any(|w| w == b"Sacha-Marqueur"));
    }

    #[test]
    fn change_passphrase_keeps_data() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        v.create("ancienne phrase").unwrap();
        members::create(
            v.conn().unwrap(),
            NewMember {
                name: "Lou".into(),
                pronouns: None,
                color: "#123456".into(),
            },
        )
        .unwrap();
        v.change_passphrase("ancienne phrase", "nouvelle phrase")
            .unwrap();
        v.lock();
        assert!(v.unlock("ancienne phrase").is_err());
        v.unlock("nouvelle phrase").unwrap();
        assert_eq!(members::list(v.conn().unwrap()).unwrap().len(), 1);
    }

    #[test]
    fn rejects_invalid_member() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        v.create("phrase secrete longue").unwrap();
        let bad = NewMember {
            name: "  ".into(),
            pronouns: None,
            color: "#000000".into(),
        };
        assert!(matches!(
            members::create(v.conn().unwrap(), bad),
            Err(CoreError::Invalid("name"))
        ));
        let bad = NewMember {
            name: "A".into(),
            pronouns: None,
            color: "red".into(),
        };
        assert!(matches!(
            members::create(v.conn().unwrap(), bad),
            Err(CoreError::Invalid("color"))
        ));
    }
}
