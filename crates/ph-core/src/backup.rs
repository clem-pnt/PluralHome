//! Sauvegardes : un fichier unique, autoportant, chiffré.
//!
//! Format (version 1) :
//!
//! ```text
//! "PLURALHOME-BACKUP\n" | longueur de l'en-tête (u32 LE) | en-tête JSON | base SQLCipher
//! ```
//!
//! L'en-tête embarque le fichier de clé du moment : la sauvegarde s'ouvre avec la
//! phrase secrète en vigueur lors de l'export, sur n'importe quelle installation.
//! Rien n'y est en clair : la base est chiffrée page à page et authentifiée (HMAC),
//! la clé est enveloppée par Argon2id + XChaCha20-Poly1305.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use rusqlite::backup::Backup;
use serde::{Deserialize, Serialize};

use crate::keyfile::KeyFile;
use crate::vault::{
    DATABASE, KEYFILE, Vault, VaultStatus, create_private_dir, open_db, write_atomic,
};
use crate::{CoreError, Result};

const MAGIC: &[u8] = b"PLURALHOME-BACKUP\n";
const FORMAT: u32 = 1;
const MAX_HEADER: usize = 64 * 1024;
/// Garde-fou contre un fichier absurde chargé en mémoire.
const MAX_BACKUP: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
struct Header {
    format: u32,
    created_at: i64,
    app_version: String,
    keyfile: KeyFile,
}

/// Dossier de travail privé, effacé à la libération quoi qu'il arrive.
struct Scratch(PathBuf);
impl Scratch {
    fn new(path: PathBuf) -> Result<Self> {
        let _ = fs::remove_dir_all(&path);
        create_private_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn sibling(dir: &Path, suffix: &str) -> PathBuf {
    let mut name = dir.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    dir.with_file_name(name)
}

impl Vault {
    /// Écrit une sauvegarde complète dans `dest`. Le coffre doit être ouvert.
    pub fn export_backup(&self, dest: &Path) -> Result<()> {
        let src = self.conn()?;
        let dek = self.dek.as_ref().ok_or(CoreError::Locked)?;
        let keyfile: KeyFile = serde_json::from_slice(&fs::read(self.keyfile_path())?)
            .map_err(|_| CoreError::CorruptKeyFile)?;

        // Copie cohérente à chaud, vers une base chiffrée avec la même clé.
        let scratch = Scratch::new(sibling(&self.dir, ".export-tmp"))?;
        let copy_path = scratch.0.join(DATABASE);
        {
            let mut copy = open_db(&copy_path, dek)?;
            Backup::new(src, &mut copy)?.run_to_completion(256, std::time::Duration::ZERO, None)?;
            // Tout replier dans le fichier principal avant de le lire.
            copy.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")?;
        }
        let db = fs::read(&copy_path)?;

        let header = serde_json::to_vec(&Header {
            format: FORMAT,
            created_at: crate::time::now_ms()?,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            keyfile,
        })
        .map_err(|_| CoreError::Crypto)?;
        let header_len = u32::try_from(header.len()).map_err(|_| CoreError::Crypto)?;

        let mut out = Vec::with_capacity(MAGIC.len() + 4 + header.len() + db.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&header_len.to_le_bytes());
        out.extend_from_slice(&header);
        out.extend_from_slice(&db);
        write_atomic(dest, &out)?;
        tracing::info!(bytes = out.len(), "sauvegarde exportée");
        Ok(())
    }

    /// Restaure une sauvegarde. Elle est intégralement vérifiée (phrase secrète,
    /// intégrité de chaque page, migrations) avant de toucher au coffre actuel,
    /// qui est conservé à côté et non supprimé.
    pub fn restore_backup(&mut self, src: &Path, passphrase: &str) -> Result<()> {
        if fs::metadata(src)?.len() > MAX_BACKUP {
            return Err(CoreError::Invalid("backup"));
        }
        let raw = fs::read(src)?;
        let (header, db) = parse(&raw)?;
        let dek = header.keyfile.unwrap_dek(passphrase)?;

        let staging = Scratch::new(sibling(&self.dir, ".restore-tmp"))?;
        let db_path = staging.0.join(DATABASE);
        fs::write(&db_path, db)?;
        let conn = open_db(&db_path, &dek)?; // clé + migrations
        verify_integrity(&conn)?;
        drop(conn);
        write_atomic(
            &staging.0.join(KEYFILE),
            &serde_json::to_vec_pretty(&header.keyfile).map_err(|_| CoreError::Crypto)?,
        )?;

        // Bascule : l'ancien coffre est mis de côté, jamais effacé.
        self.lock();
        if self.status() != VaultStatus::Uninitialized || self.dir.exists() {
            let stamp = crate::time::now_ms()?;
            fs::rename(
                &self.dir,
                sibling(&self.dir, &format!(".avant-restauration-{stamp}")),
            )?;
        }
        fs::rename(&staging.0, &self.dir)?;
        let conn = open_db(&self.dir.join(DATABASE), &dek)?;
        self.adopt(conn, dek);
        tracing::info!(format = header.format, "sauvegarde restaurée");
        Ok(())
    }
}

fn parse(raw: &[u8]) -> Result<(Header, &[u8])> {
    let bad = || CoreError::Invalid("backup");
    let rest = raw.strip_prefix(MAGIC).ok_or_else(bad)?;
    let (len, rest) = rest.split_first_chunk::<4>().ok_or_else(bad)?;
    let len = usize::try_from(u32::from_le_bytes(*len)).map_err(|_| bad())?;
    if len > MAX_HEADER || len > rest.len() {
        return Err(bad());
    }
    let (header, db) = rest.split_at(len);
    let header: Header = serde_json::from_slice(header).map_err(|_| bad())?;
    if header.format != FORMAT || db.is_empty() {
        return Err(bad());
    }
    Ok((header, db))
}

fn verify_integrity(conn: &Connection) -> Result<()> {
    // SQLCipher : vérifie le HMAC de chaque page. Aucune ligne = aucune erreur.
    let mut st = conn.prepare("PRAGMA cipher_integrity_check")?;
    if st.query([])?.next()?.is_some() {
        return Err(CoreError::Invalid("backup"));
    }
    let ok: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if ok != "ok" {
        return Err(CoreError::Invalid("backup"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyfile::KdfParams;
    use crate::members::{self, NewMember};

    fn vault_at(dir: &Path) -> Vault {
        Vault::new(dir).with_kdf(KdfParams::TEST)
    }

    fn seeded(root: &Path) -> Vault {
        let mut v = vault_at(&root.join("vault"));
        v.create("phrase de sauvegarde").unwrap();
        members::create(
            v.conn().unwrap(),
            NewMember {
                name: "Mila".into(),
                pronouns: None,
                color: "#4f8583".into(),
            },
        )
        .unwrap();
        v
    }

    #[test]
    fn export_then_restore_on_fresh_install() {
        let a = tempfile::tempdir().unwrap();
        let file = a.path().join("systeme.phbackup");
        seeded(a.path()).export_backup(&file).unwrap();

        let raw = fs::read(&file).unwrap();
        assert!(
            !raw.windows(4).any(|w| w == b"Mila"),
            "aucun contenu en clair"
        );

        let b = tempfile::tempdir().unwrap();
        let mut fresh = vault_at(&b.path().join("vault"));
        assert!(matches!(
            fresh.restore_backup(&file, "mauvaise phrase"),
            Err(CoreError::BadPassphrase)
        ));
        assert_eq!(
            fresh.status(),
            VaultStatus::Uninitialized,
            "échec sans effet de bord"
        );

        fresh.restore_backup(&file, "phrase de sauvegarde").unwrap();
        assert_eq!(fresh.status(), VaultStatus::Unlocked);
        assert_eq!(
            members::list(fresh.conn().unwrap()).unwrap()[0].name,
            "Mila"
        );

        // Et elle se rouvre normalement ensuite.
        fresh.lock();
        fresh.unlock("phrase de sauvegarde").unwrap();
    }

    #[test]
    fn restore_keeps_previous_vault_aside() {
        let a = tempfile::tempdir().unwrap();
        let file = a.path().join("s.phbackup");
        let mut v = seeded(a.path());
        v.export_backup(&file).unwrap();
        members::create(
            v.conn().unwrap(),
            NewMember {
                name: "Sacha".into(),
                pronouns: None,
                color: "#c8643b".into(),
            },
        )
        .unwrap();

        v.restore_backup(&file, "phrase de sauvegarde").unwrap();
        assert_eq!(members::list(v.conn().unwrap()).unwrap().len(), 1);
        let kept = fs::read_dir(a.path())
            .unwrap()
            .filter_map(std::result::Result::ok)
            .any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("vault.avant-restauration-")
            });
        assert!(kept, "l'ancien coffre est conservé");
    }

    #[test]
    fn rejects_tampered_or_foreign_files() {
        let a = tempfile::tempdir().unwrap();
        let file = a.path().join("s.phbackup");
        seeded(a.path()).export_backup(&file).unwrap();

        let mut raw = fs::read(&file).unwrap();
        let last = raw.len() - 100;
        raw[last] ^= 0xff; // une page altérée
        let tampered = a.path().join("t.phbackup");
        fs::write(&tampered, &raw).unwrap();

        let b = tempfile::tempdir().unwrap();
        let mut fresh = vault_at(&b.path().join("vault"));
        assert!(
            fresh
                .restore_backup(&tampered, "phrase de sauvegarde")
                .is_err()
        );
        assert_eq!(fresh.status(), VaultStatus::Uninitialized);

        let foreign = a.path().join("f.phbackup");
        fs::write(&foreign, b"ceci n'est pas une sauvegarde").unwrap();
        assert!(matches!(
            fresh.restore_backup(&foreign, "x"),
            Err(CoreError::Invalid("backup"))
        ));
    }
}
