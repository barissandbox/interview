//! Application layer: state management, Tauri commands, and event forwarding.

mod capture;
mod commands;
mod events;
mod questions;
pub mod state;
mod transcripts;
mod view;

pub use commands::{
    create_transcript, delete_transcript, generate_answer, get_app_state, open_deepgram_site,
    open_developer_site, open_source_site, refresh_chatgpt_models, refresh_devices,
    remove_cv_profile, resize_window_height, save_settings, select_transcript_by_offset,
    set_always_on_top, sign_out_chatgpt, start_capture, start_chatgpt_login, stop_capture,
    test_deepgram_key, upload_cv_profile,
};
pub use events::spawn_event_forwarder;
pub use state::AppState;
