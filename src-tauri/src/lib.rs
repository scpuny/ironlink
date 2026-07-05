// Re-export all modules for the Tauri app
pub mod api;
pub mod config;
pub mod models;
pub mod ocr;
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

use tauri::{Emitter, 
    Manager,
    menu::{MenuBuilder, SubmenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
};
use tauri_plugin_dialog;
use crate::config::AppState;

/// Start the Axum proxy server on the port from settings.
pub async fn start_proxy_server(state: Arc<AppState>) {
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

    *state.proxy_shutdown_tx.lock().await = None;
    *state.proxy_enabled.lock().await = false;
    info!("Proxy server stopped");
}

pub async fn stop_proxy_server(state: Arc<AppState>) {
    if let Some(tx) = state.proxy_shutdown_tx.lock().await.take() {
        let _ = tx.send(());
        info!("Proxy server shutdown signal sent");
    } else {
        tracing::warn!("No proxy server running to stop");
    }
}

/// App-level cleanup: stop proxy, restore configs, free OCR.
async fn cleanup(state: Arc<AppState>) {
    // Stop proxy if running
    let enabled = *state.proxy_enabled.lock().await;
    if enabled {
        tracing::info!("Stopping proxy server...");
        stop_proxy_server(state.clone()).await;

        let apps = state.apps.lock().await.clone();
        for app in apps.iter().filter(|a| a.config_injection.is_some()) {
            if let Some(inj) = &app.config_injection {
                if inj.backup_enabled {
                    crate::config::restore_app_config(inj);
                }
            }
        }
        crate::config::restore_codex_configs();
    }

    // Free OCR engine memory
    crate::ocr::shutdown();

    tracing::info!("Cleanup complete");
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
            commands::ocr::check_ocr_status,
            commands::ocr::run_ocr,
            commands::status::quit_app,
            commands::status::hide_window,
            commands::status::check_version,
        ])
        .setup(move |app| {
            info!("Tauri UI started (proxy server starts on demand)");

            // ── macOS menu with i18n ──
            #[cfg(target_os = "macos")]
            {
                // Read current language setting for menu labels
                let lang = crate::config::settings::AppSettings::load().language;
                let file_label = if lang == "zh" { "文件" } else { "File" };
                let edit_label = if lang == "zh" { "编辑" } else { "Edit" };
                let view_label = if lang == "zh" { "窗口" } else { "Window" };

                let file_menu = SubmenuBuilder::new(app, file_label)
                    .item(&MenuItemBuilder::with_id("prefs", if lang == "zh" { "设置…" } else { "Settings…" }).build(app)?)
                    .separator()
                    .item(&MenuItemBuilder::with_id("hide", if lang == "zh" { "隐藏 IronLink" } else { "Hide IronLink" }).accelerator("Cmd+H").build(app)?)
                    .item(&PredefinedMenuItem::quit(app, None)?)
                    .build()?;

                let edit_menu = SubmenuBuilder::new(app, edit_label)
                    .item(&PredefinedMenuItem::undo(app, None)?)
                    .item(&PredefinedMenuItem::redo(app, None)?)
                    .separator()
                    .item(&PredefinedMenuItem::cut(app, None)?)
                    .item(&PredefinedMenuItem::copy(app, None)?)
                    .item(&PredefinedMenuItem::paste(app, None)?)
                    .item(&PredefinedMenuItem::select_all(app, None)?)
                    .build()?;

                let window_menu = SubmenuBuilder::new(app, view_label)
                    .item(&PredefinedMenuItem::minimize(app, None)?)
                    .build()?;

                let menu = MenuBuilder::new(app)
                    .items(&[&file_menu, &edit_menu, &window_menu])
                    .build()?;

                app.set_menu(menu)?;
            }

            // ── System tray ──
            let show_item = MenuItemBuilder::with_id("show", "Show Window")
                .build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit")
                .build(app)?;
            let tray_menu = tauri::menu::MenuBuilder::new(app)
                .item(&show_item)
                .item(&quit_item)
                .build()?;

            TrayIconBuilder::new()
                .tooltip("IronLink")
                .menu(&tray_menu)
                .icon(app.default_window_icon().unwrap().clone())
                .on_menu_event(move |app_handle, event| {
                    match event.id().as_ref() {
                        "show" => {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            // Run cleanup synchronously before exit
                            let s: tauri::State<'_, Arc<AppState>> = app_handle.state();
                            let state = s.inner().clone();
                            tauri::async_runtime::block_on(async move {
                                cleanup(state).await;
                            });
                            app_handle.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray_handle, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event {
                        let app = tray_handle.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // ── OCR init ──
            let model_dir = app.path()
                .resolve("models/ppocrv5", tauri::path::BaseDirectory::Resource)
                .unwrap_or_else(|_| {
                    let dev = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("resources/models/ppocrv5");
                    if dev.exists() {
                        dev
                    } else {
                        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("src/resources/models/ppocrv5")
                    }
                });
            crate::ocr::init(model_dir.clone());

            if crate::ocr::models_ready() {
                tauri::async_runtime::spawn(async move {
                    tracing::info!("OCR engine warmup started in background...");
                    let ready = crate::ocr::warmup();
                    tracing::info!("OCR engine warmup {}",
                        if ready { "complete" } else { "failed" });
                });
            } else {
                tracing::warn!("OCR models not found at {:?}, OCR disabled", model_dir);
            }

            // Auto-start proxy if setting enabled
            {
                let settings = crate::config::settings::AppSettings::load();
                if settings.auto_start {
                    let state: tauri::State<'_, Arc<AppState>> = app.state();
                    let state_clone = state.inner().clone();
                    tauri::async_runtime::spawn(async move {
                        // Small delay to let everything settle
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        let enabled = *state_clone.proxy_enabled.lock().await;
                        if !enabled {
                            *state_clone.proxy_enabled.lock().await = true;
                            crate::start_proxy_server(state_clone).await;
                        }
                    });
                }
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |app_handle, event| {
            match event {
                // Window close → hide to tray if setting enabled
                tauri::RunEvent::WindowEvent { label: _, event: window_event, .. } => {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = window_event {
                        let s: tauri::State<'_, Arc<AppState>> = app_handle.state();
                        let settings = s.settings.blocking_lock();
                        if settings.minimize_to_tray_on_close {
                            // Auto-hide to tray without dialog
                            drop(settings);
                            drop(api);
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.hide();
                            }
                        } else {
                            drop(settings);
                            api.prevent_close();
                            // Ask frontend to show quit/hide dialog
                            let _ = app_handle.emit("close-requested", ());
                        }
                    }
                }
                // App exit → cleanup everything
                tauri::RunEvent::Exit => {
                    tracing::info!("App exiting: cleaning up...");
                    let s: tauri::State<'_, Arc<AppState>> = app_handle.state();
                    let state = s.inner().clone();
                    tauri::async_runtime::block_on(async move {
                        cleanup(state).await;
                    });
                }
                _ => {}
            }
        });
}
