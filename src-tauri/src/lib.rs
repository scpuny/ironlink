// Re-export all modules for the Tauri app
pub mod api;
pub mod config;
pub mod models;
pub mod protocol;
pub mod proxy;
pub mod commands;

use std::sync::Arc;
use std::net::SocketAddr;

use axum::{
    routing::{get, post, any},
    Router,
};
use tower_http::cors::CorsLayer;
use tracing::info;

use tauri::Manager;
use crate::config::AppState;

/// Start the Axum proxy server on the port from settings.
/// Uses graceful shutdown — call `stop_proxy_server` to send the shutdown signal.
pub async fn start_proxy_server(state: Arc<AppState>) {
    // Prevent double-start
    if state.proxy_shutdown_tx.lock().await.is_some() {
        tracing::warn!("Proxy server already running, ignoring start request");
        return;
    }

    let app = Router::new()
        .route("/api/status", get(api::get_status))
        .route("/api/backend", get(api::get_backend).put(api::put_backend))
        .route("/api/models", get(api::get_models).put(api::put_models))
        .route("/api/proxy", post(api::toggle_proxy_handler))
        .route("/api/logs", get(api::get_logs))
        .route("/api/profiles", get(api::get_profiles).put(api::put_profiles))
        .route("/api/apps", get(api::get_apps).put(api::put_apps))
        .route("/api/profiles/activate", post(api::post_profiles_activate))
        .route("/api/config", get(api::get_config).put(api::put_config))
        .route("/v1/models", get(proxy::handle_models))
        .route("/v1/responses/websocket", get(proxy::handle_websocket))
        .route("/v1/realtime", get(proxy::handle_websocket))
        .route("/v1/{*path}", any(proxy::handle_proxy))
        .layer(CorsLayer::permissive())
        .with_state(state.clone());

    let port = {
        let s = state.settings.lock().await;
        s.proxy_port
    };
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    // Kill old process on this port
    let _ = std::process::Command::new("sh")
        .args(["-c", &format!("lsof -ti :{} | xargs kill 2>/dev/null", port)])
        .output();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    *state.proxy_shutdown_tx.lock().await = Some(tx);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind proxy server on port {port}: {e}");
            *state.proxy_shutdown_tx.lock().await = None;
            return;
        }
    };

    info!("Proxy server listening on http://{}", addr);
    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(async { rx.await.ok(); })
        .await
    {
        tracing::error!("Proxy server error: {e}");
    }

    // Clean up after server stops
    *state.proxy_shutdown_tx.lock().await = None;
    *state.proxy_enabled.lock().await = false;
    info!("Proxy server stopped");
}

/// Send graceful shutdown signal to the running proxy server.
pub async fn stop_proxy_server(state: Arc<AppState>) {
    if let Some(tx) = state.proxy_shutdown_tx.lock().await.take() {
        let _ = tx.send(());
        info!("Proxy server shutdown signal sent");
    } else {
        tracing::warn!("No proxy server running to stop");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into())
        )
        .init();

    let state = AppState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::status::get_status,
            commands::backend::get_backend,
            commands::backend::update_backend,
            commands::models_cmd::get_models,
            commands::models_cmd::update_models,
            commands::backend::toggle_proxy,
            commands::config::get_config_file,
            commands::config::write_config_file,
            commands::status::get_logs,
            commands::config::get_auto_start,
            commands::config::set_auto_start,
            commands::config::get_codex_config_file,
            commands::config::read_file_content,
            commands::config::preview_app_config,
            commands::config::get_app_config_files,
            commands::config::get_model_catalog,
            commands::config::regenerate_model_catalog,
            commands::profiles::get_profiles,
            commands::profiles::save_profiles,
            commands::profiles::activate_profile,
            commands::models_cmd::fetch_upstream_models,
            commands::models_cmd::test_provider_connection,
            commands::status::get_proxy_config,
            commands::status::set_proxy_config,
            commands::apps::get_apps,
            commands::apps::save_apps,
            commands::settings_cmd::get_settings,
            commands::settings_cmd::save_settings,
            commands::settings_cmd::export_config,
            commands::settings_cmd::import_config,
        ])
        .setup(move |_app| {
            info!("Tauri UI started (proxy server starts on demand)");
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                tracing::info!("App exiting: cleaning up and restoring all configs");

                // Spawn cleanup; the runtime is still alive during Exit
                let h = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let state: tauri::State<'_, Arc<AppState>> = h.state::<Arc<AppState>>();
                    let s = state.inner().clone();

                    // ── 1. Stop proxy if running ──
                    let enabled = *s.proxy_enabled.lock().await;
                    if enabled {
                        tracing::info!("Stopping proxy server...");
                        crate::stop_proxy_server(s.clone()).await;

                        // ── 2. Restore backed-up app configs only when proxy was ON ──
                        // When proxy was disabled normally, toggle_proxy(false) already restored backups.
                        // This is a safety net for crashes / unclean exits.
                        let apps = s.apps.lock().await.clone();
                        for app in apps.iter().filter(|a| a.config_injection.is_some()) {
                            if let Some(inj) = &app.config_injection {
                                if inj.backup_enabled {
                                    crate::config::restore_app_config(inj);
                                }
                            }
                        }
                        // Legacy fallback
                        crate::config::restore_codex_configs();
                    }

                    tracing::info!("Cleanup complete on exit");
                });
            }
        });
}
