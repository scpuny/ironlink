// Re-export all modules for the Tauri app
pub mod api;
pub mod config;
pub mod convert;
pub mod models;
pub mod proxy;
pub mod commands;
pub mod sse;

use std::sync::Arc;
use std::net::SocketAddr;

use axum::{
    routing::{get, post, any},
    Router,
};
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::config::AppState;

/// Run the Axum proxy server in a background tokio task.
pub fn start_proxy(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let app = Router::new()
            .route("/api/status", get(api::get_status))
            .route("/api/backend", get(api::get_backend).put(api::put_backend))
            .route("/api/models", get(api::get_models).put(api::put_models))
            .route("/api/proxy", post(api::toggle_proxy_handler))
            .route("/api/logs", get(api::get_logs))
            .route("/api/profiles", get(api::get_profiles).put(api::put_profiles))
            .route("/api/profiles/activate", post(api::post_profiles_activate))
            .route("/api/auth", get(api::get_auth).put(api::put_auth))
            .route("/api/config", get(api::get_config).put(api::put_config))
            .route("/v1/models", get(proxy::handle_models))
            .route("/v1/{*path}", any(proxy::handle_proxy))
            .layer(CorsLayer::permissive())
            .with_state(state);

        let addr = SocketAddr::from(([0, 0, 0, 0], config::PROXY_PORT));
        
        // Kill old process on this port
        let _ = std::process::Command::new("sh")
            .args(["-c", &format!("lsof -ti :{} | xargs kill 2>/dev/null", config::PROXY_PORT)])
            .output();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to bind proxy server: {e}");
                return;
            }
        };
        info!("Proxy server listening on http://{}", addr);
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Proxy server error: {e}");
        }
    });
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
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_backend,
            commands::update_backend,
            commands::get_models,
            commands::update_models,
            commands::toggle_proxy,
            commands::get_config_file,
            commands::write_config_file,
            commands::get_logs,
            commands::get_auth_file,
            commands::write_auth_file,
            commands::get_auto_start,
            commands::set_auto_start,
            commands::get_codex_config_file,
            commands::get_codex_auth_file,
            commands::write_codex_auth_file,
            commands::get_profiles,
            commands::save_profiles,
            commands::activate_profile,
        ])
        .setup(move |_app| {
            // Start proxy inside Tauri's tokio runtime
            start_proxy(state);
            info!("Tauri UI started");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
