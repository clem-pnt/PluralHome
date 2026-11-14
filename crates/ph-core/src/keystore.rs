//! Rangement de la clé du coffre sur l'appareil, sans phrase secrète.
//!
//! La clé de données (DEK) est confiée au trousseau du système (Windows
//! Credential Manager, trousseau macOS, Secret Service sous Linux), protégé par la
//! session de l'utilisateur. À défaut de trousseau utilisable, elle est écrite
//! dans `device.key`, lisible seulement par l'utilisateur. `device.json` indique
//! où la retrouver ; il ne contient aucun secret.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::keyfile::{DEK_LEN, Dek};
use crate::vault::write_atomic;
use crate::{CoreError, Result};

pub(crate) const DEVICE_FILE: &str = "device.json";
const KEY_FILE: &str = "device.key";
const VERSION: u32 = 2;
#[cfg(feature = "os-keyring")]
const SERVICE: &str = "PluralHome";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Store {
    Keyring,
    File,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DeviceKey {
    version: u32,
    id: Uuid,
    pub(crate) store: Store,
}

impl DeviceKey {
    /// Référence de clé du coffre `dir`, si elle existe.
    pub(crate) fn read(dir: &Path) -> Result<Option<Self>> {
        let path = dir.join(DEVICE_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let dk: Self =
            serde_json::from_slice(&fs::read(path)?).map_err(|_| CoreError::CorruptKeyFile)?;
        if dk.version != VERSION {
            return Err(CoreError::CorruptKeyFile);
        }
        Ok(Some(dk))
    }

    /// Range `dek` (trousseau si `os_keyring`, sinon fichier), vérifie qu'elle se
    /// relit à l'identique, puis écrit `device.json`. Rien n'est écrit si le
    /// rangement échoue.
    pub(crate) fn save(dir: &Path, dek: &Dek, os_keyring: bool) -> Result<Self> {
        let id = Uuid::now_v7();
        let store = if os_keyring && keyring_save(id, dek) {
            Store::Keyring
        } else {
            write_atomic(&dir.join(KEY_FILE), hex::encode(dek.as_slice()).as_bytes())?;
            Store::File
        };
        let dk = Self {
            version: VERSION,
            id,
            store,
        };
        if *dk.load(dir)? != **dek {
            return Err(CoreError::KeyMissing);
        }
        write_atomic(
            &dir.join(DEVICE_FILE),
            &serde_json::to_vec_pretty(&dk).map_err(|_| CoreError::Crypto)?,
        )?;
        tracing::info!(?store, "clé du coffre rangée");
        Ok(dk)
    }

    pub(crate) fn load(&self, dir: &Path) -> Result<Dek> {
        let bytes = match self.store {
            Store::Keyring => keyring_load(self.id)?,
            Store::File => {
                let hex_key = Zeroizing::new(
                    fs::read_to_string(dir.join(KEY_FILE)).map_err(|_| CoreError::KeyMissing)?,
                );
                Zeroizing::new(hex::decode(hex_key.trim()).map_err(|_| CoreError::KeyMissing)?)
            }
        };
        let arr: [u8; DEK_LEN] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| CoreError::KeyMissing)?;
        Ok(Zeroizing::new(arr))
    }
}

#[cfg(feature = "os-keyring")]
fn keyring_save(id: Uuid, dek: &Dek) -> bool {
    match keyring::Entry::new(SERVICE, &id.to_string()).and_then(|e| e.set_secret(dek.as_slice())) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("trousseau du système indisponible, repli sur un fichier : {e}");
            false
        }
    }
}

#[cfg(not(feature = "os-keyring"))]
fn keyring_save(_: Uuid, _: &Dek) -> bool {
    false
}

#[cfg(feature = "os-keyring")]
fn keyring_load(id: Uuid) -> Result<Zeroizing<Vec<u8>>> {
    keyring::Entry::new(SERVICE, &id.to_string())
        .and_then(|e| e.get_secret())
        .map(Zeroizing::new)
        .map_err(|e| {
            tracing::error!("clé introuvable dans le trousseau : {e}");
            CoreError::KeyMissing
        })
}

#[cfg(not(feature = "os-keyring"))]
fn keyring_load(_: Uuid) -> Result<Zeroizing<Vec<u8>>> {
    Err(CoreError::KeyMissing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_store_round_trips_and_detects_loss() {
        let tmp = tempfile::tempdir().unwrap();
        let dek: Dek = Zeroizing::new([7u8; DEK_LEN]);
        assert!(DeviceKey::read(tmp.path()).unwrap().is_none());
        let dk = DeviceKey::save(tmp.path(), &dek, false).unwrap();
        assert_eq!(dk.store, Store::File);
        let again = DeviceKey::read(tmp.path()).unwrap().unwrap();
        assert_eq!(*again.load(tmp.path()).unwrap(), *dek);
        let meta = fs::read_to_string(tmp.path().join(DEVICE_FILE)).unwrap();
        assert!(
            !meta.contains(&hex::encode(*dek)),
            "aucun secret dans device.json"
        );

        fs::remove_file(tmp.path().join(KEY_FILE)).unwrap();
        assert!(matches!(again.load(tmp.path()), Err(CoreError::KeyMissing)));
    }
}
