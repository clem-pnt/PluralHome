//! Compte Aragonite et synchro PluralKit côté application : commandes et boucle
//! d'arrière-plan. Le verrou du coffre n'est jamais tenu pendant un appel réseau
//! (voir `ph_remote::db`), l'interface reste donc fluide pendant une synchro lente.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ph_core::{CoreError, Vault};
use ph_remote::RemoteError;
use ph_remote::aragonite::{self, Account, Login, Sso};
use ph_remote::cloud::{self, HttpCloud, PairPoll, PendingDevice, SsoAuth};
use ph_remote::pluralkit::{HttpPk, PkSystem};
use ph_remote::sync::{self, Status};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;

use crate::ipc::{AppState, IpcError};

/// Switchs : toutes les 30 s ; membres : toutes les 5 min (ou sur demande).
const SWITCH_EVERY: Duration = Duration::from_secs(30);
const MEMBERS_EVERY: Duration = Duration::from_secs(300);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

/// Émis après une synchro qui a modifié des données locales ou l'état.
pub const EVENT_SYNCED: &str = "pluralkit://synced";
/// Émis après une synchro chiffrée qui a reçu des données ou changé d'état.
pub const EVENT_CLOUD: &str = "cloud://synced";

#[derive(Debug, Clone, Copy)]
pub enum Wake {
    /// Un switch local vient d'être fait : l'envoyer sans attendre.
    Switches,
    /// Les membres ont changé ici, ou l'utilisateur demande une synchro.
    Full,
}

#[derive(Clone)]
pub struct Remote {
    wake: Sender<Wake>,
    login_cancel: Arc<AtomicBool>,
    /// Une seule synchro à la fois (boucle ou bouton « Synchroniser »).
    sync_lock: Arc<Mutex<()>>,
}

impl Remote {
    pub fn start(app: AppHandle, vault: Arc<Mutex<Vault>>) -> std::io::Result<Self> {
        let (wake, rx) = mpsc::channel();
        let sync_lock = Arc::new(Mutex::new(()));
        let lock = Arc::clone(&sync_lock);
        std::thread::Builder::new()
            .name("pluralkit-sync".into())
            .spawn(move || {
                let mut last_members: Option<Instant> = None;
                loop {
                    let full = match rx.recv_timeout(SWITCH_EVERY) {
                        Ok(Wake::Full) => true,
                        Ok(Wake::Switches) | Err(RecvTimeoutError::Timeout) => false,
                        Err(RecvTimeoutError::Disconnected) => return,
                    };
                    let full = full || last_members.is_none_or(|t| t.elapsed() >= MEMBERS_EVERY);
                    let _guard = lock
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    match run_cloud(&vault) {
                        Ok(Some(true)) | Err(_) => {
                            let _ = app.emit(EVENT_CLOUD, ());
                        }
                        Ok(_) => {}
                    }
                    match run_sync(&vault, full) {
                        Ok(Some(changed)) => {
                            if full {
                                last_members = Some(Instant::now());
                            }
                            if changed {
                                let _ = app.emit(EVENT_SYNCED, ());
                            }
                        }
                        // Coffre verrouillé ou PluralKit non lié : rien à faire.
                        Ok(None) => last_members = None,
                        Err(e) => {
                            tracing::warn!(code = e.code(), "synchro PluralKit échouée");
                            let _ = app.emit(EVENT_SYNCED, ());
                        }
                    }
                }
            })?;
        Ok(Self {
            wake,
            login_cancel: Arc::new(AtomicBool::new(false)),
            sync_lock,
        })
    }

    pub fn wake(&self, w: Wake) {
        let _ = self.wake.send(w);
    }
}

/// `Ok(None)` : rien à synchroniser. `Ok(Some(changed))` sinon.
fn run_sync(vault: &Mutex<Vault>, full: bool) -> ph_remote::Result<Option<bool>> {
    let token = match sync::status(vault) {
        Ok(s) if s.enabled => sync::token(vault)?,
        Ok(_) | Err(RemoteError::Core(CoreError::Locked | CoreError::NotInitialized)) => {
            return Ok(None);
        }
        Err(e) => return Err(e),
    };
    let Some(token) = token else { return Ok(None) };
    let api = HttpPk::new(&token)?;
    let r = sync::sync(vault, &api, full)?;
    Ok(Some(r != sync::SyncReport::default()))
}

/// `Ok(None)` : synchro chiffrée inactive. `Ok(Some(changed))` sinon.
fn run_cloud(vault: &Mutex<Vault>) -> ph_remote::Result<Option<bool>> {
    match cloud::status(vault) {
        Ok(s) if s.enabled => {}
        Ok(_) | Err(RemoteError::Core(CoreError::Locked | CoreError::NotInitialized)) => {
            return Ok(None);
        }
        Err(e) => return Err(e),
    }
    let sso = Sso::new()?;
    let auth = SsoAuth {
        db: vault,
        sso: &sso,
    };
    match cloud::sync(vault, &HttpCloud::new()?, &auth) {
        Ok(r) => Ok(Some(r.pulled > 0)),
        Err(e) => {
            tracing::warn!(code = e.code(), "synchro chiffrée échouée");
            Err(e)
        }
    }
}

/// Exécute `f` avec le client de synchro et le jeton du compte Aragonite.
fn with_cloud<T>(
    vault: &Mutex<Vault>,
    f: impl FnOnce(&HttpCloud, &SsoAuth<'_, Mutex<Vault>>) -> ph_remote::Result<T>,
) -> ph_remote::Result<T> {
    let sso = Sso::new()?;
    let auth = SsoAuth {
        db: vault,
        sso: &sso,
    };
    f(&HttpCloud::new()?, &auth)
}

impl From<RemoteError> for IpcError {
    fn from(e: RemoteError) -> Self {
        match e {
            RemoteError::Core(c) => c.into(),
            e => {
                tracing::info!(code = e.code(), "service distant : {e}");
                Self {
                    code: e.code().to_owned(),
                    field: None,
                }
            }
        }
    }
}

type IpcResult<T> = Result<T, IpcError>;

async fn remote<T: Send + 'static>(
    f: impl FnOnce() -> ph_remote::Result<T> + Send + 'static,
) -> IpcResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| IpcError {
            code: "internal".into(),
            field: None,
        })?
        .map_err(IpcError::from)
}

// ---------- compte Aragonite ----------

#[tauri::command]
#[specta::specta]
pub async fn account_get(state: State<'_, AppState>) -> IpcResult<Option<Account>> {
    let vault = state.vault();
    remote(move || aragonite::account(&*vault)).await
}

/// Ouvre le navigateur sur la page de connexion et attend son retour (5 min au plus).
#[tauri::command]
#[specta::specta]
pub async fn account_login(
    app: AppHandle,
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
) -> IpcResult<Account> {
    let vault = state.vault();
    // Le coffre doit être ouvert avant d'aller plus loin.
    remote({
        let vault = Arc::clone(&vault);
        move || aragonite::account(&*vault).map(drop)
    })
    .await?;
    let cancel = Arc::clone(&rem.login_cancel);
    cancel.store(false, Ordering::Relaxed);
    remote(move || {
        let sso = Sso::new()?;
        let login = Login::begin(&sso)?;
        app.opener()
            .open_url(&login.authorize_url, None::<&str>)
            .map_err(|e| RemoteError::Network(e.to_string()))?;
        login.finish(&*vault, &sso, LOGIN_TIMEOUT, &cancel)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn account_login_cancel(rem: State<'_, Remote>) -> IpcResult<()> {
    rem.login_cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn account_refresh(state: State<'_, AppState>) -> IpcResult<Account> {
    let vault = state.vault();
    remote(move || aragonite::refresh(&*vault, &Sso::new()?)).await
}

#[tauri::command]
#[specta::specta]
pub async fn account_logout(state: State<'_, AppState>) -> IpcResult<()> {
    let vault = state.vault();
    remote(move || aragonite::logout(&*vault, &Sso::new()?)).await
}

// ---------- PluralKit ----------

#[tauri::command]
#[specta::specta]
pub async fn pk_status(state: State<'_, AppState>) -> IpcResult<Status> {
    let vault = state.vault();
    remote(move || sync::status(&*vault)).await
}

#[tauri::command]
#[specta::specta]
pub async fn pk_link(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    token: String,
) -> IpcResult<PkSystem> {
    let vault = state.vault();
    let lock = Arc::clone(&rem.sync_lock);
    let token = zeroize::Zeroizing::new(token);
    remote(move || {
        let _guard = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let api = HttpPk::new(token.trim())?;
        sync::link(&*vault, &api, token.trim())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn pk_unlink(state: State<'_, AppState>, rem: State<'_, Remote>) -> IpcResult<()> {
    let vault = state.vault();
    let lock = Arc::clone(&rem.sync_lock);
    remote(move || {
        let _guard = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sync::unlink(&*vault)
    })
    .await
}

/// Synchro complète immédiate ; renvoie l'état à jour (erreur comprise).
#[tauri::command]
#[specta::specta]
pub async fn pk_sync_now(state: State<'_, AppState>, rem: State<'_, Remote>) -> IpcResult<Status> {
    let vault = state.vault();
    let lock = Arc::clone(&rem.sync_lock);
    remote(move || {
        let _guard = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Err(e) = run_sync(&vault, true) {
            tracing::warn!(code = e.code(), "synchro PluralKit échouée");
        }
        sync::status(&*vault)
    })
    .await
}

/// Active ou coupe l'envoi des descriptions sur PluralKit (propre à l'appareil).
#[tauri::command]
#[specta::specta]
pub async fn pk_set_sync_description(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    enabled: bool,
) -> IpcResult<Status> {
    let vault = state.vault();
    let r = remote(move || {
        sync::set_sync_description(&*vault, enabled)?;
        sync::status(&*vault)
    })
    .await?;
    rem.wake(Wake::Full);
    Ok(r)
}

// ---------- synchro chiffrée ----------

#[tauri::command]
#[specta::specta]
pub async fn cloud_status(state: State<'_, AppState>) -> IpcResult<cloud::CloudStatus> {
    let vault = state.vault();
    remote(move || cloud::status(&*vault)).await
}

/// Indique si une synchro existe déjà sur ce compte (il faut alors la rejoindre).
#[tauri::command]
#[specta::specta]
pub async fn cloud_probe(state: State<'_, AppState>) -> IpcResult<cloud::Remote> {
    let vault = state.vault();
    remote(move || with_cloud(&vault, |api, auth| cloud::probe(api, auth))).await
}

/// Active la synchro ; renvoie la clé de secours, affichée une seule fois.
#[tauri::command]
#[specta::specta]
pub async fn cloud_enable(state: State<'_, AppState>, rem: State<'_, Remote>) -> IpcResult<String> {
    let vault = state.vault();
    let lock = Arc::clone(&rem.sync_lock);
    let key = remote(move || {
        let _guard = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        with_cloud(&vault, |api, auth| cloud::enable(&*vault, api, auth))
    })
    .await?;
    rem.wake(Wake::Full);
    Ok(key.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn cloud_join_recovery(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
    recovery: String,
) -> IpcResult<()> {
    let vault = state.vault();
    let lock = Arc::clone(&rem.sync_lock);
    let recovery = zeroize::Zeroizing::new(recovery);
    remote(move || {
        let _guard = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        with_cloud(&vault, |api, auth| {
            cloud::join_with_recovery(&*vault, api, auth, &recovery)
        })
    })
    .await?;
    rem.wake(Wake::Full);
    Ok(())
}

/// Demande la clé à un appareil déjà synchronisé ; renvoie l'empreinte à comparer.
#[tauri::command]
#[specta::specta]
pub async fn cloud_pair_request(state: State<'_, AppState>) -> IpcResult<String> {
    let vault = state.vault();
    remote(move || with_cloud(&vault, |api, auth| cloud::pair_request(&*vault, api, auth))).await
}

#[tauri::command]
#[specta::specta]
pub async fn cloud_pair_poll(
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
) -> IpcResult<PairPoll> {
    let vault = state.vault();
    let lock = Arc::clone(&rem.sync_lock);
    let r = remote(move || {
        let _guard = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        with_cloud(&vault, |api, auth| cloud::pair_poll(&*vault, api, auth))
    })
    .await?;
    if r == PairPoll::Joined {
        rem.wake(Wake::Full);
    }
    Ok(r)
}

#[tauri::command]
#[specta::specta]
pub async fn cloud_pair_cancel(state: State<'_, AppState>) -> IpcResult<()> {
    let vault = state.vault();
    remote(move || with_cloud(&vault, |api, auth| cloud::pair_cancel(&*vault, api, auth))).await
}

/// Appareils qui attendent la clé (vu depuis un appareil déjà synchronisé).
#[tauri::command]
#[specta::specta]
pub async fn cloud_pair_requests(state: State<'_, AppState>) -> IpcResult<Vec<PendingDevice>> {
    let vault = state.vault();
    remote(move || with_cloud(&vault, |api, auth| cloud::pair_requests(&*vault, api, auth))).await
}

#[tauri::command]
#[specta::specta]
pub async fn cloud_pair_approve(state: State<'_, AppState>, id: String) -> IpcResult<()> {
    let vault = state.vault();
    remote(move || {
        with_cloud(&vault, |api, auth| {
            cloud::pair_approve(&*vault, api, auth, &id)
        })
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn cloud_pair_reject(state: State<'_, AppState>, id: String) -> IpcResult<()> {
    let vault = state.vault();
    remote(move || with_cloud(&vault, |api, auth| cloud::pair_reject(api, auth, &id))).await
}

/// Arrête la synchro sur cet appareil ; rien n'est effacé.
#[tauri::command]
#[specta::specta]
pub async fn cloud_disable(state: State<'_, AppState>, rem: State<'_, Remote>) -> IpcResult<()> {
    let vault = state.vault();
    let lock = Arc::clone(&rem.sync_lock);
    remote(move || {
        let _guard = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cloud::disable(&*vault)
    })
    .await
}

/// Efface les données du serveur pour ce compte (tous les appareils s'arrêtent).
#[tauri::command]
#[specta::specta]
pub async fn cloud_delete(state: State<'_, AppState>, rem: State<'_, Remote>) -> IpcResult<()> {
    let vault = state.vault();
    let lock = Arc::clone(&rem.sync_lock);
    remote(move || {
        let _guard = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        with_cloud(&vault, |api, auth| cloud::delete_remote(&*vault, api, auth))
    })
    .await
}

/// Synchro immédiate ; renvoie l'état à jour (erreur comprise).
#[tauri::command]
#[specta::specta]
pub async fn cloud_sync_now(
    app: AppHandle,
    state: State<'_, AppState>,
    rem: State<'_, Remote>,
) -> IpcResult<cloud::CloudStatus> {
    let vault = state.vault();
    let lock = Arc::clone(&rem.sync_lock);
    remote(move || {
        let _guard = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Ok(Some(true)) = run_cloud(&vault) {
            let _ = app.emit(EVENT_CLOUD, ());
        }
        cloud::status(&*vault)
    })
    .await
}
