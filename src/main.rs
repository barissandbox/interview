//! Starts the Tauri desktop Interview application.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod domain;
mod infra;

use anyhow::Result;
use app::{
    AppState, create_transcript, delete_transcript, generate_answer, get_app_state,
    open_deepgram_site, open_developer_site, open_source_site, refresh_chatgpt_models,
    refresh_devices, remove_cv_profile, resize_window_height, save_settings,
    select_transcript_by_offset, set_always_on_top, sign_out_chatgpt, spawn_event_forwarder,
    start_capture, start_chatgpt_login, stop_capture, test_deepgram_key, upload_cv_profile,
};
use infra::paths::app_paths;
use tauri::Manager;

/// Configures logging, persistence, and launches the Tauri window.
fn main() -> Result<()> {
    let paths = app_paths()?;
    infra::logging::install_logger(paths.log_file.clone())?;
    log::info!(
        "Interview Tauri application starting; data_dir={}",
        paths.data_dir.display()
    );
    let (state, events_rx) = AppState::new(paths)?;
    let managed_state = state.clone();

    tauri::Builder::default()
        .manage(managed_state)
        .setup(move |app| {
            let app_version = app.package_info().version.to_string();
            if let Some(window) = app.get_webview_window("main") {
                window.set_title(&format!("Interview - v{app_version}"))?;
                if let Ok(view) = state.view_state()
                    && let Err(error) = window.set_always_on_top(view.settings.always_on_top)
                {
                    log::warn!("Could not apply always-on-top setting: {error}");
                }
            }
            spawn_event_forwarder(app.handle().clone(), state.clone(), events_rx);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            save_settings,
            test_deepgram_key,
            start_chatgpt_login,
            sign_out_chatgpt,
            refresh_chatgpt_models,
            upload_cv_profile,
            remove_cv_profile,
            generate_answer,
            create_transcript,
            delete_transcript,
            select_transcript_by_offset,
            start_capture,
            stop_capture,
            refresh_devices,
            set_always_on_top,
            resize_window_height,
            open_deepgram_site,
            open_developer_site,
            open_source_site
        ])
        .run(tauri::generate_context!())
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    log::info!("Interview Tauri application stopped");
    Ok(())
}
