//! Commandes exposées au front. Les types TypeScript sont générés depuis ces
//! signatures (`pnpm bindings`) : le contrat front/back ne peut pas dériver.

use std::sync::{Arc, Mutex, MutexGuard};

use ph_core::folders::{self, Folder};
use ph_core::front::{self, FrontChange, FrontSession};
use ph_core::journal::{self, Entry, NewEntry};
use ph_core::members::{self, Member, NewMember, Profile};
use ph_core::messages::{self, CatchUp, Message, NewMessage};
use ph_core::roles;
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

/// État du coffre ; l'ouvre au passage s'il est simplement fermé (la clé est
/// rangée sur l'appareil).
#[tauri::command]
#[specta::specta]
pub async fn vault_open(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
) -> IpcResult<VaultStatus> {
    let (before, after) = blocking(&state, |v| {
        let before = v.status();
        if before == VaultStatus::Locked {
            v.open()?;
        }
        Ok((before, v.status()))
    })
    .await?;
    if before != after {
        rem.wake(Wake::Full);
    }
    Ok(after)
}

#[tauri::command]
#[specta::specta]
pub async fn vault_create(state: State<'_, AppState>) -> IpcResult<()> {
    blocking(&state, Vault::create).await
}

/// Le coffre ne s'ouvre plus : on le met de côté et on repart d'un coffre vide.
#[tauri::command]
#[specta::specta]
pub async fn vault_start_over(state: State<'_, AppState>, rem: State<'_, Remote>) -> IpcResult<()> {
    blocking(&state, Vault::start_over).await?;
    rem.wake(Wake::Full);
    Ok(())
}

/// Migration d'un coffre d'avant 0.9 : dernière saisie de la phrase secrète.
#[tauri::command]
#[specta::specta]
pub async fn vault_upgrade(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    passphrase: String,
) -> IpcResult<()> {
    blocking(&state, move |v| {
        v.upgrade(&zeroize::Zeroizing::new(passphrase))
    })
    .await?;
    rem.wake(Wake::Full);
    Ok(())
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

/// Profil détaillé (description, rôles, image). Seule la description peut
/// partir sur PluralKit, et seulement si l'option est activée.
#[tauri::command]
#[specta::specta]
pub async fn members_set_profile(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    id: Uuid,
    profile: Profile,
) -> IpcResult<Member> {
    let m = blocking(&state, move |v| {
        members::set_profile(v.conn()?, id, profile)
    })
    .await?;
    rem.wake(Wake::Full);
    Ok(m)
}

/// Catalogue de rôles du système.
#[tauri::command]
#[specta::specta]
pub async fn roles_list(state: State<'_, AppState>) -> IpcResult<Vec<String>> {
    blocking(&state, |v| roles::list(v.conn()?)).await
}

#[tauri::command]
#[specta::specta]
pub async fn roles_add(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    name: String,
) -> IpcResult<Vec<String>> {
    let r = blocking(&state, move |v| roles::add(v.conn()?, &name)).await?;
    rem.wake(Wake::Full);
    Ok(r)
}

/// Renomme un rôle partout (catalogue et membres qui le portent).
#[tauri::command]
#[specta::specta]
pub async fn roles_rename(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    from: String,
    to: String,
) -> IpcResult<Vec<String>> {
    let r = blocking(&state, move |v| roles::rename(v.conn_mut()?, &from, &to)).await?;
    rem.wake(Wake::Full);
    Ok(r)
}

/// Supprime un rôle du catalogue et le retire à tous les membres.
#[tauri::command]
#[specta::specta]
pub async fn roles_remove(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    name: String,
) -> IpcResult<Vec<String>> {
    let r = blocking(&state, move |v| roles::remove(v.conn_mut()?, &name)).await?;
    rem.wake(Wake::Full);
    Ok(r)
}

/// Dossiers de rangement des membres.
#[tauri::command]
#[specta::specta]
pub async fn folders_list(state: State<'_, AppState>) -> IpcResult<Vec<Folder>> {
    blocking(&state, |v| folders::list(v.conn()?)).await
}

#[tauri::command]
#[specta::specta]
pub async fn folders_create(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    name: String,
    parent: Option<Uuid>,
) -> IpcResult<Vec<Folder>> {
    let r = blocking(&state, move |v| folders::create(v.conn()?, &name, parent)).await?;
    rem.wake(Wake::Full);
    Ok(r)
}

/// Range un dossier dans un autre (ou à la racine avec `None`).
#[tauri::command]
#[specta::specta]
pub async fn folders_move(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    id: Uuid,
    parent: Option<Uuid>,
) -> IpcResult<Vec<Folder>> {
    let r = blocking(&state, move |v| folders::move_to(v.conn()?, id, parent)).await?;
    rem.wake(Wake::Full);
    Ok(r)
}

#[tauri::command]
#[specta::specta]
pub async fn folders_rename(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    id: Uuid,
    name: String,
) -> IpcResult<Vec<Folder>> {
    let r = blocking(&state, move |v| folders::rename(v.conn()?, id, &name)).await?;
    rem.wake(Wake::Full);
    Ok(r)
}

/// Supprime le dossier ; les membres ne sont pas touchés.
#[tauri::command]
#[specta::specta]
pub async fn folders_remove(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    id: Uuid,
) -> IpcResult<Vec<Folder>> {
    let r = blocking(&state, move |v| folders::remove(v.conn()?, id)).await?;
    rem.wake(Wake::Full);
    Ok(r)
}

#[tauri::command]
#[specta::specta]
pub async fn folders_set_members(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    id: Uuid,
    members: Vec<Uuid>,
) -> IpcResult<Vec<Folder>> {
    let r = blocking(&state, move |v| {
        folders::set_members(v.conn_mut()?, id, &members)
    })
    .await?;
    rem.wake(Wake::Full);
    Ok(r)
}

/// Remplace les dossiers d'un membre.
#[tauri::command]
#[specta::specta]
pub async fn folders_set_for_member(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    member: Uuid,
    folders: Vec<Uuid>,
) -> IpcResult<Vec<Folder>> {
    let r = blocking(&state, move |v| {
        folders::set_for_member(v.conn_mut()?, member, &folders)
    })
    .await?;
    rem.wake(Wake::Full);
    Ok(r)
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

/// Membres du switch rapide : les favoris choisis, sinon les plus fréquents
/// des 90 derniers jours. Le booléen dit s'il s'agit de favoris choisis.
#[tauri::command]
#[specta::specta]
pub async fn front_quick(state: State<'_, AppState>) -> IpcResult<(Vec<Uuid>, bool)> {
    blocking(&state, |v| front::quick(v.conn()?)).await
}

#[tauri::command]
#[specta::specta]
pub async fn front_set_favorites(
    state: State<'_, AppState>,
    members: Vec<Uuid>,
) -> IpcResult<Vec<Uuid>> {
    blocking(&state, move |v| front::set_favorites(v.conn()?, &members)).await
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
    unread_only: bool,
) -> IpcResult<Vec<Message>> {
    blocking(&state, move |v| {
        messages::list(v.conn()?, before, limit, unread_only)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn messages_post(state: State<'_, AppState>, input: NewMessage) -> IpcResult<Message> {
    blocking(&state, move |v| messages::post(v.conn_mut()?, &input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn messages_update(
    state: State<'_, AppState>,
    id: Uuid,
    input: NewMessage,
) -> IpcResult<Message> {
    blocking(&state, move |v| messages::update(v.conn_mut()?, id, &input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn messages_mark_read_by(
    state: State<'_, AppState>,
    id: Uuid,
    members: Vec<Uuid>,
) -> IpcResult<Message> {
    blocking(&state, move |v| {
        messages::mark_read_by(v.conn_mut()?, id, &members)
    })
    .await
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

#[tauri::command]
#[specta::specta]
pub async fn journal_list(
    state: State<'_, AppState>,
    before: Option<Uuid>,
    author: Option<Uuid>,
    limit: u32,
) -> IpcResult<Vec<Entry>> {
    blocking(&state, move |v| {
        journal::list(v.conn()?, before, author, limit)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn journal_create(state: State<'_, AppState>, input: NewEntry) -> IpcResult<Entry> {
    blocking(&state, move |v| journal::create(v.conn_mut()?, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn journal_update(
    state: State<'_, AppState>,
    id: Uuid,
    input: NewEntry,
) -> IpcResult<Entry> {
    blocking(&state, move |v| journal::update(v.conn_mut()?, id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn journal_delete(state: State<'_, AppState>, id: Uuid) -> IpcResult<()> {
    blocking(&state, move |v| journal::delete(v.conn()?, id)).await
}

pub fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        vault_open,
        vault_create,
        vault_start_over,
        vault_upgrade,
        members_list,
        members_create,
        members_update,
        members_set_profile,
        members_archive,
        roles_list,
        roles_add,
        roles_rename,
        roles_remove,
        folders_list,
        folders_create,
        folders_rename,
        folders_move,
        folders_remove,
        folders_set_members,
        folders_set_for_member,
        front_current,
        front_change,
        front_history,
        front_quick,
        front_set_favorites,
        messages_list,
        messages_post,
        messages_update,
        messages_mark_read_by,
        messages_set_pinned,
        messages_delete,
        catch_up,
        messages_mark_read,
        journal_list,
        journal_create,
        journal_update,
        journal_delete,
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
        remote::pk_set_sync_description,
        remote::cloud_status,
        remote::cloud_probe,
        remote::cloud_enable,
        remote::cloud_join_recovery,
        remote::cloud_pair_request,
        remote::cloud_pair_poll,
        remote::cloud_pair_cancel,
        remote::cloud_pair_requests,
        remote::cloud_pair_approve,
        remote::cloud_pair_reject,
        remote::cloud_disable,
        remote::cloud_delete,
        remote::cloud_sync_now,
    ])
}
