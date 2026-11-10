//! Accès à la base sous verrou, le temps strictement nécessaire.

use std::sync::Mutex;

use ph_core::Vault;
use ph_core::rusqlite::Connection;

pub trait Db: Sync {
    fn with<T>(&self, f: impl FnOnce(&mut Connection) -> ph_core::Result<T>) -> ph_core::Result<T>;
}

impl Db for Mutex<Vault> {
    fn with<T>(&self, f: impl FnOnce(&mut Connection) -> ph_core::Result<T>) -> ph_core::Result<T> {
        let mut vault = self
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(vault.conn_mut()?)
    }
}
