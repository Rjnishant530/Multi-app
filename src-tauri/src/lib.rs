mod commands;
mod model;
mod nav_guard;
mod paths;
mod store;
mod webview_manager;

use std::sync::Arc;

use parking_lot::RwLock;
use tauri::{Emitter, Manager};
use tracing::info;

use crate::commands::{InstanceAddedEvent, state_ops};
use crate::model::AppState;
use crate::store::{Persister, SharedState};
use crate::webview_manager::{ForkReceiver, WebviewManager};

pub struct AppContext {
    pub state: SharedState,
    pub persister: Arc<Persister>,
    pub webviews: Arc<WebviewManager>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,multi_app_lib=debug".into()),
        )
        .with_target(false)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
                let _ = window.unminimize();
            }
        }))
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            commands::list_websites,
            commands::add_website,
            commands::remove_website,
            commands::list_instance_tree,
            commands::add_instance,
            commands::rename_instance,
            commands::set_instance_icon,
            commands::remove_instance,
            commands::activate_website,
            commands::activate_instance,
            commands::set_viewport_bounds,
            commands::set_active_webview_visibility,
            commands::instance_back,
            commands::instance_forward,
            commands::instance_reload,
        ])
        .setup(|app| {
            let handle = app.handle();
            let metadata = paths::metadata_dir(handle)?;
            let webviews = paths::webviews_dir(handle)?;
            info!(
                metadata = %metadata.display(),
                webviews = %webviews.display(),
                "app data directories ready"
            );

            let mut initial = store::load_initial_state(handle).unwrap_or_else(|err| {
                tracing::warn!(error = ?err, "failed to load persisted state; starting fresh");
                AppState::default()
            });
            let fixed = state_ops::sanitize(&mut initial);
            if fixed > 0 {
                tracing::warn!(
                    fixed,
                    "cleaned up dangling references in persisted state"
                );
            }
            let state: SharedState = Arc::new(RwLock::new(initial));
            let writer = store::make_app_writer(handle.clone());
            let persister = Arc::new(Persister::start(state.clone(), writer));

            let (fork_tx, fork_rx) = webview_manager::fork_channel();
            let webviews = Arc::new(WebviewManager::new(fork_tx));

            spawn_fork_consumer(
                handle.clone(),
                state.clone(),
                Arc::clone(&persister),
                Arc::clone(&webviews),
                fork_rx,
            );

            app.manage(AppContext {
                state,
                persister,
                webviews,
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn spawn_fork_consumer(
    handle: tauri::AppHandle,
    state: SharedState,
    persister: Arc<Persister>,
    webviews: Arc<WebviewManager>,
    mut rx: ForkReceiver,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(req) = rx.recv().await {
            let new_instance = {
                let mut s = state.write();
                match state_ops::fork_instance(
                    &mut s,
                    req.parent_instance_id,
                    &req.target_url,
                    crate::model::now_ms(),
                ) {
                    Ok(inst) => {
                        // Surface the new child as the active instance
                        // so the user lands on what they clicked. Failure
                        // is non-fatal — the instance still exists.
                        let _ = state_ops::set_active_instance(&mut s, inst.id);
                        inst
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "fork rejected by state_ops");
                        continue;
                    }
                }
            };
            persister.schedule();
            if let Err(err) = webviews.ensure_spawned(&handle, &new_instance) {
                tracing::warn!(error = ?err, "fork webview spawn failed");
                continue;
            }
            if let Err(err) = webviews.activate(&handle, new_instance.id) {
                tracing::warn!(error = ?err, "fork webview activate failed");
                // Fall through to still emit the event so the UI sees the
                // new node in its tree, even if it isn't visible yet.
            }
            if let Err(err) = handle.emit(
                "instance:added",
                InstanceAddedEvent {
                    instance: new_instance,
                },
            ) {
                tracing::warn!(error = ?err, "could not emit instance:added");
            }
        }
    });
}
