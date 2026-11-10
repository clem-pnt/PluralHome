mod ipc;
mod logging;
mod remote;

use tauri::{Manager, WindowEvent};

use crate::ipc::AppState;

pub fn run() {
    let specta = ipc::builder();

    tauri::Builder::default()
        // Une seule instance : deux processus sur la même base chiffrée, c'est non.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            let log_dir = app.path().app_log_dir()?;
            let guard = logging::init(&log_dir)?;
            app.manage(guard);
            let data_dir = app.path().app_data_dir()?;
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "démarrage");
            let state = AppState::new(ph_core::Vault::new(data_dir.join("vault")));
            app.manage(remote::Remote::start(app.handle().clone(), state.vault())?);
            app.manage(state);
            specta.mount_events(app);
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::Destroyed) {
                window.state::<AppState>().lock_vault();
            }
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            tracing::error!("échec du démarrage : {e}");
            std::process::exit(1);
        });
}

#[cfg(test)]
mod tests {
    /// Régénère `src/ipc/bindings.ts`. Lancé par `pnpm bindings` et vérifié en CI.
    #[test]
    fn export_bindings() {
        super::ipc::builder()
            .export(
                specta_typescript::Typescript::default()
                    .header("// Fichier généré par `pnpm bindings`. Ne pas modifier.\n"),
                concat!(env!("CARGO_MANIFEST_DIR"), "/../src/ipc/bindings.ts"),
            )
            .unwrap();
    }
}
