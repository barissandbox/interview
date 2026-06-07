//! View models and data transfer types for the frontend.

use crate::domain::{
    AppSettings, AudioDevice, AvailableModel, CandidateProfile, LanguageOption, QuestionRecord,
    ThinkingVariantOption,
};
use serde::{Deserialize, Serialize};

/// UI state returned to the frontend.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppViewState {
    /// Current persisted settings.
    pub settings: AppSettings,
    /// Deepgram balance label.
    pub balance: String,
    /// Status bar message.
    pub status: String,
    /// ChatGPT sign-in state.
    pub chatgpt: ChatGptViewState,
    /// Stored CV/profile metadata.
    pub profile: ProfileViewState,
    /// Cached model catalog.
    pub models: Vec<AvailableModel>,
    /// Reasoning options for the selected model.
    pub thinking_variants: Vec<ThinkingVariantOption>,
    /// Compact transcript list for navigation.
    pub transcripts: Vec<TranscriptSummary>,
    /// Currently selected transcript id.
    pub active_transcript_id: String,
    /// Zero-based index of the active transcript.
    pub active_index: usize,
    /// Total number of transcripts.
    pub transcript_count: usize,
    /// Rendered text of the active transcript.
    pub transcript_text: String,
    /// Detected questions for the active transcript.
    pub questions: Vec<QuestionRecord>,
    /// Selected question index.
    pub selected_question_index: isize,
    /// Selected question text.
    pub selected_question: String,
    /// Selected/generated answer text.
    pub answer_text: String,
    /// Whether the selected answer is currently generating.
    pub answer_pending: bool,
    /// Available audio devices.
    pub devices: Vec<AudioDevice>,
    /// Supported Deepgram languages.
    pub languages: Vec<LanguageOption>,
    /// Whether capture is running.
    pub running: bool,
}

/// ChatGPT auth summary exposed to the UI.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptViewState {
    pub logged_in: bool,
    pub account_email: String,
    pub limit_label: String,
    pub error: String,
}

/// CV/profile summary exposed to the UI.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileViewState {
    pub file_name: String,
    pub text_length: usize,
    pub updated_at: Option<String>,
}

impl From<&CandidateProfile> for ProfileViewState {
    /// Converts stored profile data into a secret-free frontend view.
    fn from(profile: &CandidateProfile) -> Self {
        Self {
            file_name: profile.file_name.clone(),
            text_length: profile.text.chars().count(),
            updated_at: profile.updated_at.map(|value| value.to_rfc3339()),
        }
    }
}

/// Compact transcript metadata for UI navigation.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSummary {
    /// Transcript id.
    pub id: String,
    /// Display label.
    pub label: String,
}

/// Settings payload accepted from the frontend.
///
/// Note: API keys and ChatGPT tokens are intentionally excluded.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendSettings {
    /// Selected speaker device id.
    pub speaker_device_id: String,
    /// Selected microphone device id.
    pub microphone_device_id: String,
    /// Deepgram language code.
    pub language: String,
    /// Whether speaker source is enabled.
    pub speaker_enabled: bool,
    /// Whether microphone source is enabled.
    pub microphone_enabled: bool,
    /// Selected ChatGPT model.
    pub model: String,
    /// Selected thinking variant.
    pub thinking_variant: String,
    /// Selected answer format.
    pub answer_type: String,
    /// Whether ChatGPT requests should use the priority service tier.
    pub fast_enabled: bool,
    /// Selected ChatGPT text verbosity.
    pub verbosity: String,
    /// Optional target position context.
    pub target_position: String,
    /// Whether the main window stays above other windows.
    pub always_on_top: bool,
}
