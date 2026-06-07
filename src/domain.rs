//! Domain models and constants for the Interview app.

use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};

/// Deepgram default language.
pub const DEFAULT_LANGUAGE: &str = "en-US";

/// Default ChatGPT model.
pub const DEFAULT_MODEL: &str = "gpt-5.4-mini";

/// Default ChatGPT thinking variant.
pub const DEFAULT_THINKING_VARIANT: &str = "low";

/// Default answer style.
pub const DEFAULT_ANSWER_TYPE: &str = "details";

/// Default ChatGPT response verbosity.
pub const DEFAULT_VERBOSITY: &str = "low";

/// Default Codex client version used for ChatGPT model catalog fallback.
pub const DEFAULT_CODEX_CLIENT_VERSION: &str = "0.128.0";

/// Speaker source label.
pub const SPEAKER_LABEL: &str = "Speaker";

/// Microphone source label.
pub const MICROPHONE_LABEL: &str = "Microphone";

/// Identifies an audio source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioSourceKind {
    /// System speaker/output audio.
    Speaker,
    /// Microphone/input audio.
    Microphone,
}

impl AudioSourceKind {
    /// Returns a stable display label for the source.
    pub fn label(self) -> &'static str {
        match self {
            Self::Speaker => SPEAKER_LABEL,
            Self::Microphone => MICROPHONE_LABEL,
        }
    }
}

/// Selectable audio device shown by the UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    /// Stable device id.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Source kind.
    pub kind: AudioSourceKind,
    /// Whether this is the default device.
    pub is_default: bool,
    /// Whether the app can capture from this device.
    pub is_available: bool,
}

/// One Deepgram language option.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LanguageOption {
    /// Deepgram language code.
    pub value: &'static str,
    /// UI label.
    pub label: &'static str,
    /// Optional model override.
    pub model: Option<&'static str>,
}

/// Saved transcript record.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptRecord {
    /// Transcript id.
    pub id: String,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Updated timestamp.
    pub updated_at: DateTime<Utc>,
    /// Selected language.
    pub language: String,
    /// Final transcript segments.
    #[serde(default)]
    pub segments: Vec<TranscriptSegment>,
    /// Detected interview questions and generated answers.
    #[serde(default)]
    pub questions: Vec<QuestionRecord>,
}

impl TranscriptRecord {
    /// Returns list label in dd.MM - HH:mm format.
    pub fn list_label(&self) -> String {
        let timestamp = if self.is_empty() {
            self.created_at
        } else {
            self.updated_at
        };
        timestamp
            .with_timezone(&Local)
            .format("%d.%m - %H:%M")
            .to_string()
    }

    /// Returns true when no transcript or question has been saved.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty() && self.questions.is_empty()
    }
}

/// Saved final transcript segment.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    /// Segment id.
    pub id: i64,
    /// Source label.
    pub source: String,
    /// Final text.
    pub text: String,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
}

/// Detected interview question plus generated answer.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionRecord {
    /// Stable question id.
    pub id: String,
    /// Normalized duplicate-detection key.
    pub key: String,
    /// Question text sent to ChatGPT.
    pub text: String,
    /// Latest generated answer text.
    #[serde(default)]
    pub answer: String,
    /// Whether an answer request is currently in flight.
    #[serde(default)]
    pub pending: bool,
    /// Answer style used for the latest answer.
    #[serde(default)]
    pub answer_type: String,
    /// ChatGPT model used for the latest answer.
    #[serde(default)]
    pub model: String,
    /// Reasoning effort used for the latest answer.
    #[serde(default)]
    pub thinking_variant: String,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Updated timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Persisted app settings.
///
/// Serialized as camelCase for the frontend. Aliases preserve backwards
/// compatibility with existing snake_case `settings.json` files where useful.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Deepgram API key.
    #[serde(default, alias = "api_key")]
    pub api_key: String,
    /// Selected speaker device id.
    #[serde(default, alias = "speaker_device_id")]
    pub speaker_device_id: String,
    /// Selected microphone device id.
    #[serde(default, alias = "microphone_device_id")]
    pub microphone_device_id: String,
    /// Deepgram language code.
    #[serde(default)]
    pub language: String,
    /// Whether speaker source is enabled.
    #[serde(default = "default_enabled", alias = "speaker_enabled")]
    pub speaker_enabled: bool,
    /// Whether microphone source is enabled.
    #[serde(default = "default_enabled", alias = "microphone_enabled")]
    pub microphone_enabled: bool,
    /// Active transcript id.
    #[serde(default, alias = "active_transcript_id")]
    pub active_transcript_id: String,
    /// Selected ChatGPT model.
    #[serde(default = "default_model")]
    pub model: String,
    /// Selected ChatGPT reasoning effort.
    #[serde(default = "default_thinking_variant")]
    pub thinking_variant: String,
    /// Selected answer format.
    #[serde(default = "default_answer_type")]
    pub answer_type: String,
    /// Whether ChatGPT requests should use the priority service tier.
    #[serde(default = "default_enabled")]
    pub fast_enabled: bool,
    /// Selected ChatGPT text verbosity.
    #[serde(default = "default_verbosity")]
    pub verbosity: String,
    /// Optional target role or position context.
    #[serde(default, alias = "target_position")]
    pub target_position: String,
    /// Whether the main window stays above other windows.
    #[serde(default, alias = "always_on_top")]
    pub always_on_top: bool,
}

impl Default for AppSettings {
    /// Builds default settings for a first launch.
    fn default() -> Self {
        Self {
            api_key: String::new(),
            speaker_device_id: String::new(),
            microphone_device_id: String::new(),
            language: DEFAULT_LANGUAGE.to_owned(),
            speaker_enabled: true,
            microphone_enabled: true,
            active_transcript_id: String::new(),
            model: DEFAULT_MODEL.to_owned(),
            thinking_variant: DEFAULT_THINKING_VARIANT.to_owned(),
            answer_type: DEFAULT_ANSWER_TYPE.to_owned(),
            fast_enabled: true,
            verbosity: DEFAULT_VERBOSITY.to_owned(),
            target_position: String::new(),
            always_on_top: false,
        }
    }
}

/// Returns the default enabled state for audio source settings.
fn default_enabled() -> bool {
    true
}

fn default_model() -> String {
    DEFAULT_MODEL.to_owned()
}

fn default_thinking_variant() -> String {
    DEFAULT_THINKING_VARIANT.to_owned()
}

fn default_answer_type() -> String {
    DEFAULT_ANSWER_TYPE.to_owned()
}

fn default_verbosity() -> String {
    DEFAULT_VERBOSITY.to_owned()
}

/// ChatGPT OAuth state persisted locally.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStorage {
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: i64,
    #[serde(default)]
    pub account_email: String,
    #[serde(default)]
    pub chatgpt_account_id: String,
    #[serde(default)]
    pub pending_oauth: Option<PendingOAuth>,
    #[serde(default)]
    pub error: String,
}

/// Pending OAuth verifier data.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingOAuth {
    pub state: String,
    pub verifier: String,
    pub started_at: i64,
}

/// Stored CV/profile context.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateProfile {
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

/// ChatGPT model catalog persisted locally.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogStorage {
    #[serde(default = "fallback_models")]
    pub available_models: Vec<AvailableModel>,
    #[serde(default = "default_codex_client_version")]
    pub codex_client_version: String,
    #[serde(default)]
    pub chatgpt_limit_label: String,
}

impl Default for CatalogStorage {
    /// Builds the fallback model catalog used before ChatGPT login.
    fn default() -> Self {
        Self {
            available_models: fallback_models(),
            codex_client_version: DEFAULT_CODEX_CLIENT_VERSION.to_owned(),
            chatgpt_limit_label: String::new(),
        }
    }
}

fn default_codex_client_version() -> String {
    DEFAULT_CODEX_CLIENT_VERSION.to_owned()
}

/// ChatGPT model entry shown in the UI.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableModel {
    pub id: String,
    pub model: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default = "default_input_modalities")]
    pub input_modalities: Vec<String>,
    #[serde(default = "default_thinking_variant")]
    pub default_thinking_variant: String,
    #[serde(default = "fallback_thinking_variants")]
    pub thinking_variants: Vec<ThinkingVariantOption>,
}

/// One reasoning option for a ChatGPT model.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingVariantOption {
    pub value: String,
    pub description: String,
}

/// Deepgram API key status.
#[derive(Clone, Debug)]
pub struct DeepgramAccountStatus {
    /// Whether the key was accepted.
    pub valid: bool,
    /// User-facing message.
    pub message: String,
    /// Optional balance label.
    pub balance_label: String,
}

/// Returns the supported Deepgram language list.
pub fn language_options() -> Vec<LanguageOption> {
    vec![
        LanguageOption {
            value: "en-US",
            label: "English",
            model: None,
        },
        LanguageOption {
            value: "tr",
            label: "Turkish",
            model: None,
        },
        LanguageOption {
            value: "multi",
            label: "Multilingual",
            model: None,
        },
        LanguageOption {
            value: "ar",
            label: "Arabic",
            model: None,
        },
        LanguageOption {
            value: "de",
            label: "German",
            model: None,
        },
        LanguageOption {
            value: "es",
            label: "Spanish",
            model: None,
        },
        LanguageOption {
            value: "fr",
            label: "French",
            model: None,
        },
        LanguageOption {
            value: "it",
            label: "Italian",
            model: None,
        },
        LanguageOption {
            value: "pt-BR",
            label: "Portuguese (Brazil)",
            model: None,
        },
        LanguageOption {
            value: "ru",
            label: "Russian",
            model: None,
        },
        LanguageOption {
            value: "zh",
            label: "Chinese (Mandarin)",
            model: None,
        },
        LanguageOption {
            value: "ja",
            label: "Japanese",
            model: None,
        },
        LanguageOption {
            value: "ko",
            label: "Korean",
            model: None,
        },
        LanguageOption {
            value: "th",
            label: "Thai",
            model: Some("nova-2"),
        },
        LanguageOption {
            value: "vi",
            label: "Vietnamese",
            model: None,
        },
    ]
}

/// Normalizes a stored language code.
pub fn normalize_language(value: &str) -> String {
    if language_options()
        .iter()
        .any(|language| language.value == value)
    {
        value.to_owned()
    } else {
        DEFAULT_LANGUAGE.to_owned()
    }
}

/// Normalizes a ChatGPT text verbosity value.
pub fn normalize_verbosity(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "low" | "medium" | "high" => value.trim().to_ascii_lowercase(),
        _ => DEFAULT_VERBOSITY.to_owned(),
    }
}

/// Returns the Deepgram model for a language.
pub fn model_for_language(value: &str) -> &'static str {
    language_options()
        .into_iter()
        .find(|language| language.value == value)
        .and_then(|language| language.model)
        .unwrap_or("nova-3")
}

/// Normalizes an answer type.
pub fn normalize_answer_type(value: &str) -> String {
    match value {
        "keywords" | "details" | "sentences" => value.to_owned(),
        _ => DEFAULT_ANSWER_TYPE.to_owned(),
    }
}

/// Returns fallback model catalog entries.
pub fn fallback_models() -> Vec<AvailableModel> {
    vec![
        fallback_model("gpt-5.4", false),
        fallback_model(DEFAULT_MODEL, true),
    ]
}

fn fallback_model(model: &str, is_default: bool) -> AvailableModel {
    AvailableModel {
        id: model.to_owned(),
        model: model.to_owned(),
        display_name: model.to_owned(),
        description: String::new(),
        hidden: false,
        is_default,
        input_modalities: default_input_modalities(),
        default_thinking_variant: DEFAULT_THINKING_VARIANT.to_owned(),
        thinking_variants: fallback_thinking_variants(),
    }
}

fn default_input_modalities() -> Vec<String> {
    vec!["text".to_owned(), "image".to_owned()]
}

/// Returns fallback thinking variants.
pub fn fallback_thinking_variants() -> Vec<ThinkingVariantOption> {
    vec![
        thinking("low", "Fast responses with lighter reasoning"),
        thinking("medium", "Balanced reasoning for everyday tasks"),
        thinking("high", "Greater reasoning depth for complex tasks"),
        thinking("xhigh", "Extra high reasoning depth for complex tasks"),
    ]
}

fn thinking(value: &str, description: &str) -> ThinkingVariantOption {
    ThinkingVariantOption {
        value: value.to_owned(),
        description: description.to_owned(),
    }
}
