//! Commandes exposées au front. Les types TypeScript sont générés depuis ces
//! signatures (`pnpm bindings`) : le contrat front/back ne peut pas dériver.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use ph_core::front::{self, FrontChange, FrontSession};
use ph_core::members::{self, Member, NewMember};
use ph_core::messages::{self, CatchUp, Message, NewMessage};
use ph_core::safety::{self, SafetyPlan};
use ph_core::{CoreError, Vault, VaultStatus};
use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use crate::remote::{self, Remote, Wake};

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

    pub fn vault(&self) -> Arc<Mutex<Vault>> {
        Arc::clone(&self.vault)
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
pub async fn vault_unlock(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    passphrase: String,
) -> IpcResult<()> {
    blocking(&state, move |v| {
        v.unlock(&zeroize::Zeroizing::new(passphrase))
    })
    .await?;
    rem.wake(Wake::Full);
    Ok(())
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
pub async fn members_create(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    input: NewMember,
) -> IpcResult<Member> {
    let m = blocking(&state, move |v| members::create(v.conn()?, input)).await?;
    rem.wake(Wake::Full);
    Ok(m)
}

#[tauri::command]
#[specta::specta]
pub async fn members_update(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    id: Uuid,
    input: NewMember,
) -> IpcResult<Member> {
    let m = blocking(&state, move |v| members::update(v.conn()?, id, input)).await?;
    rem.wake(Wake::Full);
    Ok(m)
}

/// Archive : le membre disparaît des listes, son historique reste.
#[tauri::command]
#[specta::specta]
pub async fn members_archive(state: State<'_, AppState>, id: Uuid) -> IpcResult<()> {
    blocking(&state, move |v| members::archive(v.conn_mut()?, id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn front_current(state: State<'_, AppState>) -> IpcResult<Vec<FrontSession>> {
    blocking(&state, |v| front::current(v.conn()?)).await
}

#[tauri::command]
#[specta::specta]
pub async fn front_change(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    change: FrontChange,
) -> IpcResult<Vec<FrontSession>> {
    let current = blocking(&state, move |v| front::change(v.conn_mut()?, &change)).await?;
    rem.wake(Wake::Switches);
    Ok(current)
}

/// Bornes en millisecondes Unix, `[from, to)`.
#[tauri::command]
#[specta::specta]
pub async fn front_history(
    state: State<'_, AppState>,
    from: f64,
    to: f64,
) -> IpcResult<Vec<FrontSession>> {
    #[allow(clippy::cast_possible_truncation)]
    let (from, to) = (from as i64, to as i64);
    blocking(&state, move |v| front::history(v.conn()?, from, to)).await
}

#[tauri::command]
#[specta::specta]
pub async fn messages_list(
    state: State<'_, AppState>,
    before: Option<Uuid>,
    limit: u32,
) -> IpcResult<Vec<Message>> {
    blocking(&state, move |v| messages::list(v.conn()?, before, limit)).await
}

#[tauri::command]
#[specta::specta]
pub async fn messages_post(state: State<'_, AppState>, input: NewMessage) -> IpcResult<Message> {
    blocking(&state, move |v| messages::post(v.conn()?, &input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn messages_set_pinned(
    state: State<'_, AppState>,
    id: Uuid,
    pinned: bool,
) -> IpcResult<()> {
    blocking(&state, move |v| messages::set_pinned(v.conn()?, id, pinned)).await
}

#[tauri::command]
#[specta::specta]
pub async fn messages_delete(state: State<'_, AppState>, id: Uuid) -> IpcResult<()> {
    blocking(&state, move |v| messages::delete(v.conn()?, id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn catch_up(state: State<'_, AppState>, member_id: Uuid) -> IpcResult<CatchUp> {
    blocking(&state, move |v| messages::catch_up(v.conn()?, member_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn messages_mark_read(
    state: State<'_, AppState>,
    member_id: Uuid,
    ids: Vec<Uuid>,
) -> IpcResult<()> {
    blocking(&state, move |v| {
        messages::mark_read(v.conn_mut()?, member_id, &ids)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn safety_get(state: State<'_, AppState>) -> IpcResult<SafetyPlan> {
    blocking(&state, |v| safety::get(v.conn()?)).await
}

#[tauri::command]
#[specta::specta]
pub async fn safety_set(state: State<'_, AppState>, plan: SafetyPlan) -> IpcResult<SafetyPlan> {
    blocking(&state, move |v| safety::set(v.conn()?, plan)).await
}

/// Les chemins viennent des boîtes de dialogue natives ; on impose l'extension
/// pour éviter d'écraser par erreur un fichier qui n'est pas une sauvegarde.
fn backup_path(path: String) -> IpcResult<PathBuf> {
    let p = PathBuf::from(path);
    if p.is_absolute() && p.extension().is_some_and(|e| e == "phbackup") {
        Ok(p)
    } else {
        Err(IpcError {
            code: "invalid".into(),
            field: Some("path".into()),
        })
    }
}

#[tauri::command]
#[specta::specta]
pub async fn backup_export(state: State<'_, AppState>, path: String) -> IpcResult<()> {
    let path = backup_path(path)?;
    blocking(&state, move |v| v.export_backup(&path)).await
}

#[tauri::command]
#[specta::specta]
pub async fn backup_restore(
    state: State<'_, AppState>,
    path: String,
    passphrase: String,
) -> IpcResult<()> {
    let path = backup_path(path)?;
    blocking(&state, move |v| {
        v.restore_backup(&path, &zeroize::Zeroizing::new(passphrase))
    })
    .await
}

pub fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        vault_status,
        vault_create,
        vault_unlock,
        vault_lock,
        members_list,
        members_create,
        members_update,
        members_archive,
        front_current,
        front_change,
        front_history,
        messages_list,
        messages_post,
        messages_set_pinned,
        messages_delete,
        catch_up,
        messages_mark_read,
        backup_export,
        backup_restore,
        safety_get,
        safety_set,
        remote::account_get,
        remote::account_login,
        remote::account_login_cancel,
        remote::account_refresh,
        remote::account_logout,
        remote::pk_status,
        remote::pk_link,
        remote::pk_unlink,
        remote::pk_sync_now,
    ])
}
