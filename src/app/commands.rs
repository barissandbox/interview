//! Tauri command handlers.
//!
//! Each command is a thin wrapper that delegates to [`AppState`] methods
//! and maps errors to strings for the Tauri IPC boundary.

use super::state::AppState;
use super::view::{AppViewState, FrontendSettings};
use tauri::{AppHandle, Manager, PhysicalSize, Size, State};

/// Shorthand for Tauri command results.
type CommandResult<T> = std::result::Result<T, String>;

/// Returns the current app state.
#[tauri::command]
pub fn get_app_state(state: State<'_, AppState>) -> CommandResult<AppViewState> {
    state.view_state().map_err(|error| error.to_string())
}

/// Saves settings from the frontend.
#[tauri::command]
pub fn save_settings(
    settings: FrontendSettings,
    state: State<'_, AppState>,
) -> CommandResult<AppViewState> {
    state
        .save_frontend_settings(settings)
        .map_err(|error| error.to_string())
}

/// Tests and saves the Deepgram API key.
#[tauri::command]
pub async fn test_deepgram_key(
    api_key: String,
    state: State<'_, AppState>,
) -> CommandResult<AppViewState> {
    state
        .test_and_save_key(api_key)
        .await
        .map_err(|error| error.to_string())
}

/// Starts ChatGPT OAuth sign-in.
#[tauri::command]
pub fn start_chatgpt_login(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> CommandResult<AppViewState> {
    state
        .start_chatgpt_login(app_handle)
        .map_err(|error| error.to_string())
}

/// Signs out of ChatGPT locally.
#[tauri::command]
pub fn sign_out_chatgpt(state: State<'_, AppState>) -> CommandResult<AppViewState> {
    state.sign_out_chatgpt().map_err(|error| error.to_string())
}

/// Refreshes ChatGPT models and thinking variants.
#[tauri::command]
pub async fn refresh_chatgpt_models(state: State<'_, AppState>) -> CommandResult<AppViewState> {
    state
        .refresh_chatgpt_models()
        .await
        .map_err(|error| error.to_string())
}

/// Uploads a local PDF CV/profile.
#[tauri::command]
pub fn upload_cv_profile(
    file_name: String,
    bytes: Vec<u8>,
    state: State<'_, AppState>,
) -> CommandResult<AppViewState> {
    state
        .upload_cv_profile(file_name, bytes)
        .map_err(|error| error.to_string())
}

/// Removes stored CV/profile context.
#[tauri::command]
pub fn remove_cv_profile(state: State<'_, AppState>) -> CommandResult<AppViewState> {
    state.remove_cv_profile().map_err(|error| error.to_string())
}

/// Generates an answer for a selected or recent question.
#[tauri::command]
pub fn generate_answer(
    question: Option<String>,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> CommandResult<AppViewState> {
    state
        .generate_answer(question, app_handle)
        .map_err(|error| error.to_string())
}

/// Creates a new transcript.
#[tauri::command]
pub fn create_transcript(state: State<'_, AppState>) -> CommandResult<AppViewState> {
    state.create_transcript().map_err(|error| error.to_string())
}

/// Deletes the active transcript.
#[tauri::command]
pub fn delete_transcript(state: State<'_, AppState>) -> CommandResult<AppViewState> {
    state.delete_transcript().map_err(|error| error.to_string())
}

/// Moves to the previous or next transcript.
#[tauri::command]
pub fn select_transcript_by_offset(
    offset: isize,
    state: State<'_, AppState>,
) -> CommandResult<AppViewState> {
    state
        .select_transcript_by_offset(offset)
        .map_err(|error| error.to_string())
}

/// Starts audio capture.
#[tauri::command]
pub fn start_capture(state: State<'_, AppState>) -> CommandResult<AppViewState> {
    state.start_capture().map_err(|error| error.to_string())
}

/// Stops audio capture.
#[tauri::command]
pub fn stop_capture(state: State<'_, AppState>) -> CommandResult<AppViewState> {
    state.stop_capture().map_err(|error| error.to_string())
}

/// Refreshes available audio devices.
#[tauri::command]
pub fn refresh_devices(state: State<'_, AppState>) -> CommandResult<AppViewState> {
    state.refresh_devices().map_err(|error| error.to_string())
}

/// Enables or disables always-on-top for the main window.
#[tauri::command]
pub fn set_always_on_top(
    enabled: bool,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> CommandResult<AppViewState> {
    let view = state
        .set_always_on_top(enabled)
        .map_err(|error| error.to_string())?;
    let window = app_handle
        .get_webview_window("main")
        .ok_or_else(|| "Main window was not found.".to_owned())?;
    window
        .set_always_on_top(enabled)
        .map_err(|error| error.to_string())?;
    Ok(view)
}

/// Resizes the main window by a logical-height delta.
#[tauri::command]
pub fn resize_window_height(delta: i32, app_handle: AppHandle) -> CommandResult<()> {
    if delta == 0 {
        return Ok(());
    }
    let window = app_handle
        .get_webview_window("main")
        .ok_or_else(|| "Main window was not found.".to_owned())?;
    let size = window.inner_size().map_err(|error| error.to_string())?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let physical_delta = (delta as f64 * scale).round() as i32;
    let min_height = (320.0 * scale).round() as i32;
    let next_height = (size.height as i32 + physical_delta).max(min_height) as u32;
    window
        .set_size(Size::Physical(PhysicalSize::new(size.width, next_height)))
        .map_err(|error| error.to_string())
}

/// Opens the Deepgram dashboard in the user's default browser.
#[tauri::command]
pub fn open_deepgram_site(state: State<'_, AppState>) -> CommandResult<()> {
    state
        .open_deepgram_site()
        .map_err(|error| error.to_string())
}

/// Opens the developer website in the user's default browser.
#[tauri::command]
pub fn open_developer_site(state: State<'_, AppState>) -> CommandResult<()> {
    state
        .open_developer_site()
        .map_err(|error| error.to_string())
}

/// Opens the source repository in the user's default browser.
#[tauri::command]
pub fn open_source_site(state: State<'_, AppState>) -> CommandResult<()> {
    state.open_source_site().map_err(|error| error.to_string())
}
