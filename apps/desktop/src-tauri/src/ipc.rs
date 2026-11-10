//! Commandes exposées au front. Les types TypeScript sont générés depuis ces
//! signatures (`pnpm bindings`) : le contrat front/back ne peut pas dériver.

use std::sync::{Arc, Mutex, MutexGuard};

use ph_core::members::{self, Member, NewMember};
use ph_core::{CoreError, Vault, VaultStatus};
use serde::Serialize;
use tauri::State;

#[derive(Clone)]
pub struct AppState {
    vault: Arc<Mutex<Vault>>,
}

impl AppState {
    pub fn new(vault: Vault) -> Self {
        Self {
            vault: Arc::new(Mutex::new(vault)),
        }
    }

    /// Un verrou empoisonné signifie qu'un panic a eu lieu pendant une
    /// opération : on verrouille le coffre par sécurité et on continue.
    fn lock(&self) -> MutexGuard<'_, Vault> {
        self.vault.lock().unwrap_or_else(|poisoned| {
            let mut v = poisoned.into_inner();
            v.lock();
            v
        })
    }

    pub fn lock_vault(&self) {
        self.lock().lock();
    }
}

/// Erreur telle que vue par le front : un code stable à traduire, jamais de détail brut.
#[derive(Debug, Serialize, specta::Type)]
pub struct IpcError {
    pub code: String,
    pub field: Option<String>,
}

impl From<CoreError> for IpcError {
    fn from(e: CoreError) -> Self {
        match &e {
            CoreError::Storage(_) | CoreError::Io(_) | CoreError::Crypto => {
                tracing::error!(code = e.code(), "erreur interne : {e}");
            }
            _ => tracing::debug!(code = e.code(), "erreur métier"),
        }
        Self {
            code: e.code().to_owned(),
            field: match e {
                CoreError::Invalid(f) => Some(f.to_owned()),
                _ => None,
            },
        }
    }
}

type IpcResult<T> = Result<T, IpcError>;

/// Exécute une opération sur le coffre hors du thread d'interface
/// (Argon2id prend volontairement plusieurs centaines de millisecondes).
async fn blocking<T: Send + 'static>(
    state: &AppState,
    f: impl FnOnce(&mut Vault) -> ph_core::Result<T> + Send + 'static,
) -> IpcResult<T> {
    let state = state.clone();
    tauri::async_runtime::spawn_blocking(move || f(&mut state.lock()))
        .await
        .map_err(|_| IpcError {
            code: "internal".into(),
            field: None,
        })?
        .map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn vault_status(state: State<'_, AppState>) -> IpcResult<VaultStatus> {
    blocking(&state, |v| Ok(v.status())).await
}

#[tauri::command]
#[specta::specta]
pub async fn vault_create(state: State<'_, AppState>, passphrase: String) -> IpcResult<()> {
    blocking(&state, move |v| {
        v.create(&zeroize::Zeroizing::new(passphrase))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn vault_unlock(state: State<'_, AppState>, passphrase: String) -> IpcResult<()> {
    blocking(&state, move |v| {
        v.unlock(&zeroize::Zeroizing::new(passphrase))
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn vault_lock(state: State<'_, AppState>) -> IpcResult<()> {
    blocking(&state, |v| {
        v.lock();
        Ok(())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn members_list(state: State<'_, AppState>) -> IpcResult<Vec<Member>> {
    blocking(&state, |v| members::list(v.conn()?)).await
}

#[tauri::command]
#[specta::specta]
pub async fn members_create(state: State<'_, AppState>, input: NewMember) -> IpcResult<Member> {
    blocking(&state, move |v| members::create(v.conn()?, input)).await
}

pub fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        vault_status,
        vault_create,
        vault_unlock,
        vault_lock,
        members_list,
        members_create,
    ])
}
