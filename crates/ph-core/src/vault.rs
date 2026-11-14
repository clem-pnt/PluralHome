//! Coffre : création, ouverture, migration depuis l'ancien mode à phrase secrète.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::keyfile::{Dek, KdfParams, KeyFile};
use crate::keystore::{DEVICE_FILE, DeviceKey};
use crate::{CoreError, Result, migrations};

/// Ancien fichier de clé (≤ 0.8) : la DEK enveloppée par la phrase secrète.
pub(crate) const KEYFILE: &str = "keyfile.json";
pub(crate) const DATABASE: &str = "pluralhome.db";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum VaultStatus {
    Uninitialized,
    /// Coffre d'avant 0.9 : la phrase secrète est demandée une dernière fois.
    Legacy,
    /// Coffre fermé, ouvrable sans saisie.
    Locked,
    Unlocked,
}

pub struct Vault {
    pub(crate) dir: PathBuf,
    pub(crate) kdf: KdfParams,
    pub(crate) os_keyring: bool,
    conn: Option<Connection>,
    /// Clé de données, gardée en mémoire (effacée à la libération) tant que le
    /// coffre est ouvert : SQLCipher la détient de toute façon, et elle permet
    /// des copies chiffrées cohérentes à chaud (sauvegardes).
    pub(crate) dek: Option<Dek>,
}

impl Vault {
    /// Coffre dont la clé est rangée dans un fichier protégé.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            kdf: KdfParams::DEFAULT,
            os_keyring: false,
            conn: None,
            dek: None,
        }
    }

    /// Range la clé dans le trousseau du système quand il est disponible.
    #[must_use]
    pub fn with_os_keyring(mut self) -> Self {
        self.os_keyring = true;
        self
    }

    #[doc(hidden)]
    #[must_use]
    pub fn with_kdf(mut self, kdf: KdfParams) -> Self {
        self.kdf = kdf;
        self
    }

    pub(crate) fn keyfile_path(&self) -> PathBuf {
        self.dir.join(KEYFILE)
    }

    #[must_use]
    pub fn status(&self) -> VaultStatus {
        if self.conn.is_some() {
            VaultStatus::Unlocked
        } else if self.dir.join(DEVICE_FILE).exists() {
            VaultStatus::Locked
        } else if self.keyfile_path().exists() {
            VaultStatus::Legacy
        } else {
            VaultStatus::Uninitialized
        }
    }

    pub fn create(&mut self) -> Result<()> {
        if self.status() != VaultStatus::Uninitialized {
            return Err(CoreError::AlreadyInitialized);
        }
        create_private_dir(&self.dir)?;
        let dek = crate::keyfile::new_dek()?;
        // La base d'abord : si on s'interrompt ici, aucune référence de clé n'existe
        // et le coffre reste « non créé ». Une base orpheline est écrasée.
        let _ = fs::remove_file(self.dir.join(DATABASE));
        let conn = open_db(&self.dir.join(DATABASE), &dek)?;
        DeviceKey::save(&self.dir, &dek, self.os_keyring)?;
        self.conn = Some(conn);
        self.dek = Some(dek);
        tracing::info!("coffre créé");
        Ok(())
    }

    /// Ouvre le coffre avec la clé rangée sur l'appareil.
    pub fn open(&mut self) -> Result<()> {
        match self.status() {
            VaultStatus::Unlocked => return Ok(()),
            VaultStatus::Uninitialized => return Err(CoreError::NotInitialized),
            VaultStatus::Legacy => return Err(CoreError::Locked),
            VaultStatus::Locked => {}
        }
        let dk = DeviceKey::read(&self.dir)?.ok_or(CoreError::KeyMissing)?;
        let dek = dk.load(&self.dir)?;
        let conn = open_db(&self.dir.join(DATABASE), &dek).map_err(|e| match e {
            CoreError::BadPassphrase => CoreError::KeyMissing,
            e => e,
        })?;
        self.conn = Some(conn);
        self.dek = Some(dek);
        tracing::info!("coffre ouvert");
        Ok(())
    }

    /// Passe un coffre d'avant 0.9 au rangement sur l'appareil : la phrase secrète
    /// sert une dernière fois, puis l'ancien fichier de clé est supprimé.
    pub fn upgrade(&mut self, passphrase: &str) -> Result<()> {
        if self.status() != VaultStatus::Legacy {
            return self.open();
        }
        let raw = fs::read(self.keyfile_path())?;
        let kf: KeyFile = serde_json::from_slice(&raw).map_err(|_| CoreError::CorruptKeyFile)?;
        let dek = kf.unwrap_dek(passphrase)?;
        let conn = open_db(&self.dir.join(DATABASE), &dek)?;
        DeviceKey::save(&self.dir, &dek, self.os_keyring)?;
        fs::remove_file(self.keyfile_path())?;
        self.conn = Some(conn);
        self.dek = Some(dek);
        tracing::info!("coffre migré : plus de phrase secrète");
        Ok(())
    }

    /// Ferme la connexion : `SQLCipher` efface sa copie de la clé.
    pub fn lock(&mut self) {
        self.dek = None;
        if self.conn.take().is_some() {
            tracing::info!("coffre fermé");
        }
    }

    pub fn conn(&self) -> Result<&Connection> {
        self.conn.as_ref().ok_or(CoreError::Locked)
    }

    /// Accès mutable, nécessaire aux opérations transactionnelles.
    pub fn conn_mut(&mut self) -> Result<&mut Connection> {
        self.conn.as_mut().ok_or(CoreError::Locked)
    }
}

pub(crate) fn open_db(path: &Path, dek: &Dek) -> Result<Connection> {
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

pub(crate) fn create_private_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Écriture atomique : fichier temporaire, fsync, renommage, fsync du dossier.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
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
        v.create().unwrap();
        assert!(matches!(v.create(), Err(CoreError::AlreadyInitialized)));
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
        v.open().unwrap();
        let list = members::list(v.conn().unwrap()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Mila");
        assert_eq!(list[0].color, "#4f8583");
    }

    #[test]
    fn database_is_encrypted_at_rest() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        v.create().unwrap();
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
    fn missing_or_wrong_key_is_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        v.create().unwrap();
        v.lock();
        fs::write(tmp.path().join("device.key"), hex::encode([1u8; 32])).unwrap();
        assert!(matches!(v.open(), Err(CoreError::KeyMissing)));
        fs::remove_file(tmp.path().join("device.key")).unwrap();
        assert!(matches!(v.open(), Err(CoreError::KeyMissing)));
    }

    /// Reproduit un coffre créé par la 0.8 (phrase secrète).
    fn legacy_vault(dir: &Path, passphrase: &str) {
        create_private_dir(dir).unwrap();
        let (kf, dek) = KeyFile::create(passphrase, KdfParams::TEST).unwrap();
        let conn = open_db(&dir.join(DATABASE), &dek).unwrap();
        members::create(
            &conn,
            NewMember {
                name: "Lou".into(),
                pronouns: None,
                color: "#123456".into(),
            },
        )
        .unwrap();
        drop(conn);
        write_atomic(&dir.join(KEYFILE), &serde_json::to_vec(&kf).unwrap()).unwrap();
    }

    #[test]
    fn upgrades_passphrase_vault_once() {
        let tmp = tempfile::tempdir().unwrap();
        legacy_vault(tmp.path(), "ancienne phrase");
        let mut v = vault(tmp.path());
        assert_eq!(v.status(), VaultStatus::Legacy);
        assert!(matches!(v.open(), Err(CoreError::Locked)));
        assert!(matches!(
            v.upgrade("mauvaise phrase"),
            Err(CoreError::BadPassphrase)
        ));
        assert_eq!(v.status(), VaultStatus::Legacy, "échec sans effet de bord");
        v.upgrade("ancienne phrase").unwrap();
        assert!(!tmp.path().join(KEYFILE).exists());
        v.lock();

        let mut v = vault(tmp.path());
        assert_eq!(v.status(), VaultStatus::Locked);
        v.open().unwrap();
        assert_eq!(members::list(v.conn().unwrap()).unwrap()[0].name, "Lou");
    }

    #[test]
    fn update_and_archive_member() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        v.create().unwrap();
        let id = members::create(
            v.conn().unwrap(),
            NewMember {
                name: "Lou".into(),
                pronouns: None,
                color: "#123456".into(),
            },
        )
        .unwrap()
        .id;
        let up = members::update(
            v.conn().unwrap(),
            id,
            NewMember {
                name: "Louise".into(),
                pronouns: Some("elle".into()),
                color: "#ABCDEF".into(),
            },
        )
        .unwrap();
        assert_eq!(
            (up.name.as_str(), up.pronouns.as_deref(), up.color.as_str()),
            ("Louise", Some("elle"), "#abcdef")
        );

        let c = v.conn_mut().unwrap();
        crate::front::change(
            c,
            &crate::front::FrontChange {
                members: vec![id],
                blurry: false,
            },
        )
        .unwrap();
        members::archive(c, id).unwrap();
        assert!(members::list(c).unwrap().is_empty());
        assert!(
            crate::front::current(c).unwrap().is_empty(),
            "un membre archivé quitte le front"
        );
        assert!(matches!(members::archive(c, id), Err(CoreError::NotFound)));
        assert!(matches!(
            members::update(
                c,
                id,
                NewMember {
                    name: "X".into(),
                    pronouns: None,
                    color: "#000000".into()
                }
            ),
            Err(CoreError::NotFound)
        ));
    }

    #[test]
    fn rejects_invalid_member() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = vault(tmp.path());
        v.create().unwrap();
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
