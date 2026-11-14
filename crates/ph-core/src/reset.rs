//! Repartir de zéro quand le coffre ne s'ouvre plus (clé perdue du trousseau,
//! par exemple). L'ancien dossier est mis de côté, jamais effacé : si la clé
//! réapparaît, il reste récupérable à la main.

use std::fs;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::vault::{Vault, VaultStatus};

fn sibling(dir: &Path, suffix: &str) -> PathBuf {
    let mut name = dir.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    dir.with_file_name(name)
}

impl Vault {
    /// Met le coffre actuel de côté et en crée un neuf, vide et ouvert.
    pub fn start_over(&mut self) -> Result<()> {
        self.lock();
        if self.dir.exists() {
            let stamp = crate::time::now_ms()?;
            fs::rename(
                &self.dir,
                sibling(&self.dir, &format!(".mis-de-cote-{stamp}")),
            )?;
        }
        debug_assert_eq!(self.status(), VaultStatus::Uninitialized);
        self.create()?;
        tracing::info!("coffre recréé, l'ancien est mis de côté");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::keyfile::KdfParams;
    use crate::members::{self, NewMember};
    use crate::vault::{Vault, VaultStatus};

    #[test]
    fn start_over_keeps_previous_vault_aside() {
        let root = tempfile::tempdir().unwrap();
        let mut v = Vault::new(root.path().join("vault")).with_kdf(KdfParams::TEST);
        v.create().unwrap();
        members::create(
            v.conn().unwrap(),
            NewMember {
                name: "Mila".into(),
                pronouns: None,
                color: "#4f8583".into(),
            },
        )
        .unwrap();

        v.start_over().unwrap();
        assert_eq!(v.status(), VaultStatus::Unlocked);
        assert!(members::list(v.conn().unwrap()).unwrap().is_empty());
        let kept = std::fs::read_dir(root.path())
            .unwrap()
            .filter_map(std::result::Result::ok)
            .any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("vault.mis-de-cote-")
            });
        assert!(kept, "l'ancien coffre est conservé");
    }
}
