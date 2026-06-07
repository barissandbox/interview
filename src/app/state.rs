//! Shared application state and business logic.

use super::capture::{CaptureSession, capture_status};
use super::questions::{detect_questions, normalize_question_candidate, tail_chars};
use super::transcripts::{
    apply_default_devices, format_transcript_text, recent_transcript_tail, resolve_active_id,
    selected_question_index,
};
use super::view::{AppViewState, ChatGptViewState, FrontendSettings, TranscriptSummary};
use crate::domain::{
    AppSettings, AudioDevice, AuthStorage, CandidateProfile, CatalogStorage, DeepgramAccountStatus,
    SPEAKER_LABEL, TranscriptRecord, fallback_models, language_options, normalize_answer_type,
    normalize_language, normalize_verbosity,
};
use crate::infra::{
    audio, chatgpt,
    paths::AppPaths,
    shell,
    storage::{QuestionAnswerUpdate, Storage},
};
use anyhow::{Result, anyhow};
use chrono::Utc;
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{AppHandle, Emitter};
use tokio::runtime::Runtime;

const MAX_CV_TEXT_CHARS: usize = 60_000;

/// Shared Tauri application state.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Mutex<InnerState>>,
    runtime: Arc<Runtime>,
    events_tx: crossbeam_channel::Sender<crate::infra::deepgram::DeepgramEvent>,
}

/// Mutable application state guarded by a mutex.
struct InnerState {
    storage: Storage,
    settings: AppSettings,
    auth: AuthStorage,
    profile: CandidateProfile,
    catalog: CatalogStorage,
    balance: String,
    status: String,
    transcripts: Vec<TranscriptRecord>,
    active_transcript_id: String,
    devices: Vec<AudioDevice>,
    capture: CaptureSession,
}

#[derive(Clone)]
struct AnswerWork {
    transcript_id: String,
    question_id: String,
    question: String,
    settings: AppSettings,
    profile: CandidateProfile,
}

impl AppState {
    /// Creates application state from persisted files.
    pub fn new(
        paths: AppPaths,
    ) -> Result<(
        Self,
        crossbeam_channel::Receiver<crate::infra::deepgram::DeepgramEvent>,
    )> {
        let storage = Storage::new(&paths)?;
        let mut settings = storage.load_settings()?;
        let auth = storage.load_auth()?;
        let profile = storage.load_profile()?;
        let catalog = storage.load_catalog()?;
        let devices = audio::list_devices();
        apply_default_devices(&mut settings, &devices);

        let mut transcripts = storage.load_transcripts()?;
        if transcripts.is_empty() {
            transcripts.push(storage.create_transcript(&settings.language)?);
        }

        let active_transcript_id = resolve_active_id(&settings, &transcripts);
        settings.active_transcript_id = active_transcript_id.clone();
        storage.save_settings(&settings)?;

        let (events_tx, events_rx) = crossbeam_channel::unbounded();
        let state = Self {
            inner: Arc::new(Mutex::new(InnerState {
                storage,
                settings,
                auth,
                profile,
                catalog,
                balance: String::new(),
                status: "Ready.".to_owned(),
                transcripts,
                active_transcript_id,
                devices,
                capture: CaptureSession::new(),
            })),
            runtime: Arc::new(Runtime::new()?),
            events_tx,
        };
        Ok((state, events_rx))
    }

    /// Returns a serializable view state.
    pub fn view_state(&self) -> Result<AppViewState> {
        let inner = self.lock()?;
        Ok(inner.build_view())
    }

    /// Saves frontend settings (excludes secrets).
    pub fn save_frontend_settings(&self, input: FrontendSettings) -> Result<AppViewState> {
        let mut inner = self.lock()?;
        inner.settings.speaker_device_id = input.speaker_device_id;
        inner.settings.microphone_device_id = input.microphone_device_id;
        inner.settings.language = normalize_language(&input.language);
        inner.settings.speaker_enabled = input.speaker_enabled;
        inner.settings.microphone_enabled = input.microphone_enabled;
        inner.settings.model = normalize_model_choice(&input.model, &inner.catalog);
        inner.settings.thinking_variant = normalize_thinking_choice(
            &input.thinking_variant,
            &inner.settings.model,
            &inner.catalog,
        );
        inner.settings.answer_type = normalize_answer_type(&input.answer_type);
        inner.settings.fast_enabled = input.fast_enabled;
        inner.settings.verbosity = normalize_verbosity(&input.verbosity);
        inner.settings.target_position = input.target_position.trim().chars().take(160).collect();
        inner.settings.always_on_top = input.always_on_top;
        inner.settings.active_transcript_id = inner.active_transcript_id.clone();
        if inner.capture.is_running() {
            let settings = inner.settings.clone();
            inner
                .capture
                .reconcile(&settings, &self.runtime, &self.events_tx)?;
            inner.status = capture_status(&inner.settings, true);
        }
        inner.storage.save_settings(&inner.settings)?;
        Ok(inner.build_view())
    }

    /// Tests and saves a Deepgram API key after validation.
    pub async fn test_and_save_key(&self, api_key: String) -> Result<AppViewState> {
        let result: DeepgramAccountStatus =
            crate::infra::deepgram::test_key_and_balance(&api_key).await?;
        let mut inner = self.lock()?;
        inner.status = result.message;
        inner.balance = result.balance_label;
        if result.valid {
            inner.settings.api_key = api_key;
            inner.storage.save_settings(&inner.settings)?;
        }
        Ok(inner.build_view())
    }

    /// Starts ChatGPT OAuth sign-in in the default browser.
    pub fn start_chatgpt_login(&self, app_handle: AppHandle) -> Result<AppViewState> {
        let (pending, authorization_url) = chatgpt::create_login_request()?;
        {
            let mut inner = self.lock()?;
            inner.auth.pending_oauth = Some(pending.clone());
            inner.auth.error.clear();
            inner.status = "Opening ChatGPT sign-in...".to_owned();
            inner.storage.save_auth(&inner.auth)?;
        }

        let state = self.clone();
        let handle = app_handle.clone();
        self.runtime.spawn(async move {
            let result = async {
                let code = chatgpt::wait_for_oauth_callback(pending.state.clone()).await?;
                let auth = chatgpt::exchange_authorization_code(&code, &pending.verifier).await?;
                state.finish_chatgpt_login(auth).await
            }
            .await;
            match result {
                Ok(view) => {
                    let _ = handle.emit(
                        "transcript-event",
                        crate::app::events::UiEvent::State {
                            state: Box::new(view),
                        },
                    );
                }
                Err(error) => {
                    state.set_status(&format!("ChatGPT sign-in failed: {error}"));
                    let _ = handle.emit(
                        "transcript-event",
                        crate::app::events::UiEvent::Error {
                            message: error.to_string(),
                        },
                    );
                }
            }
        });
        shell::open_url(&authorization_url)?;
        self.view_state()
    }

    /// Signs out of ChatGPT locally.
    pub fn sign_out_chatgpt(&self) -> Result<AppViewState> {
        let mut inner = self.lock()?;
        inner.auth = AuthStorage::default();
        inner.storage.save_auth(&inner.auth)?;
        inner.status = "Signed out of ChatGPT.".to_owned();
        Ok(inner.build_view())
    }

    /// Refreshes the ChatGPT model catalog.
    pub async fn refresh_chatgpt_models(&self) -> Result<AppViewState> {
        let access = self.access_context().await?;
        let mut catalog = chatgpt::fetch_model_catalog(&access).await?;
        catalog.chatgpt_limit_label = chatgpt::fetch_usage_limit_label(&access)
            .await
            .unwrap_or_default();
        let mut inner = self.lock()?;
        inner.catalog = catalog;
        inner.settings.model = normalize_model_choice(&inner.settings.model, &inner.catalog);
        inner.settings.thinking_variant = normalize_thinking_choice(
            &inner.settings.thinking_variant,
            &inner.settings.model,
            &inner.catalog,
        );
        inner.storage.save_catalog(&inner.catalog)?;
        inner.storage.save_settings(&inner.settings)?;
        inner.status = "ChatGPT models refreshed.".to_owned();
        Ok(inner.build_view())
    }

    /// Stores PDF CV/profile text.
    pub fn upload_cv_profile(&self, file_name: String, bytes: Vec<u8>) -> Result<AppViewState> {
        if !file_name.to_lowercase().ends_with(".pdf") {
            return Err(anyhow!("Unsupported CV file type. Use PDF only."));
        }
        let text = sanitize_cv_text(&chatgpt::extract_pdf_text(&bytes)?)
            .chars()
            .take(MAX_CV_TEXT_CHARS)
            .collect::<String>();
        if text.trim().is_empty() {
            return Err(anyhow!(
                "Could not extract readable CV text from this file."
            ));
        }
        let mut inner = self.lock()?;
        inner.profile = CandidateProfile {
            file_name,
            text,
            updated_at: Some(Utc::now()),
        };
        inner.storage.save_profile(&inner.profile)?;
        inner.status = "CV profile loaded locally.".to_owned();
        Ok(inner.build_view())
    }

    /// Removes stored CV/profile context.
    pub fn remove_cv_profile(&self) -> Result<AppViewState> {
        let mut inner = self.lock()?;
        inner.profile = CandidateProfile::default();
        inner.storage.clear_profile()?;
        inner.status = "CV profile removed.".to_owned();
        Ok(inner.build_view())
    }

    /// Creates a new transcript.
    pub fn create_transcript(&self) -> Result<AppViewState> {
        let mut inner = self.lock()?;
        if inner.capture.is_running() {
            return Ok(inner.build_view());
        }
        let transcript = inner.storage.create_transcript(&inner.settings.language)?;
        inner.active_transcript_id = transcript.id.clone();
        inner.transcripts.push(transcript);
        inner.sync_active_id_to_settings()?;
        Ok(inner.build_view())
    }

    /// Deletes the active transcript when allowed.
    pub fn delete_transcript(&self) -> Result<AppViewState> {
        let mut inner = self.lock()?;
        if inner.capture.is_running() {
            return Ok(inner.build_view());
        }
        let Some(active) = inner.active_transcript().cloned() else {
            return Ok(inner.build_view());
        };
        if inner.transcripts.len() == 1 && active.is_empty() {
            inner.status = "The last empty transcript cannot be deleted.".to_owned();
            return Ok(inner.build_view());
        }
        inner.storage.delete_transcript(&active.id)?;
        inner.reload_transcripts()?;
        if inner.transcripts.is_empty() {
            let transcript = inner.storage.create_transcript(&inner.settings.language)?;
            inner.active_transcript_id = transcript.id.clone();
            inner.transcripts.push(transcript);
        } else {
            inner.active_transcript_id = inner
                .transcripts
                .first()
                .map(|item| item.id.clone())
                .unwrap_or_default();
        }
        inner.sync_active_id_to_settings()?;
        Ok(inner.build_view())
    }

    /// Selects a transcript by relative offset.
    pub fn select_transcript_by_offset(&self, offset: isize) -> Result<AppViewState> {
        let mut inner = self.lock()?;
        if inner.capture.is_running() {
            return Ok(inner.build_view());
        }
        let Some(index) = inner.active_transcript_index() else {
            return Ok(inner.build_view());
        };
        let next = index as isize + offset;
        if next < 0 || next >= inner.transcripts.len() as isize {
            return Ok(inner.build_view());
        }
        if let Some(transcript) = inner.transcripts.get(next as usize) {
            inner.active_transcript_id = transcript.id.clone();
            inner.sync_active_id_to_settings()?;
        }
        Ok(inner.build_view())
    }

    /// Starts the selected capture sources.
    pub fn start_capture(&self) -> Result<AppViewState> {
        let mut inner = self.lock()?;
        if inner.capture.is_running() {
            return Ok(inner.build_view());
        }
        if inner.settings.api_key.trim().is_empty() {
            inner.status = "Enter and test a Deepgram API key first.".to_owned();
            return Ok(inner.build_view());
        }
        if inner.auth.access_token.is_empty() && inner.auth.refresh_token.is_empty() {
            inner.status = "Sign in with ChatGPT before starting.".to_owned();
            return Ok(inner.build_view());
        }
        if !inner.settings.microphone_enabled && !inner.settings.speaker_enabled {
            inner.status = "Turn Speaker or Mic on before starting.".to_owned();
            return Ok(inner.build_view());
        }
        inner.sync_active_id_to_settings()?;
        let settings = inner.settings.clone();
        inner
            .capture
            .start(&settings, &self.runtime, &self.events_tx)?;
        inner.status = capture_status(&inner.settings, true);
        Ok(inner.build_view())
    }

    /// Stops all capture sources.
    pub fn stop_capture(&self) -> Result<AppViewState> {
        let mut inner = self.lock()?;
        inner.stop_capture();
        Ok(inner.build_view())
    }

    /// Persists the always-on-top preference.
    pub fn set_always_on_top(&self, enabled: bool) -> Result<AppViewState> {
        let mut inner = self.lock()?;
        inner.settings.always_on_top = enabled;
        inner.storage.save_settings(&inner.settings)?;
        Ok(inner.build_view())
    }

    /// Refreshes audio devices.
    pub fn refresh_devices(&self) -> Result<AppViewState> {
        let mut inner = self.lock()?;
        inner.devices = audio::list_devices();
        Ok(inner.build_view())
    }

    /// Generates an answer for an explicit question or recent transcript tail.
    pub fn generate_answer(
        &self,
        question: Option<String>,
        app_handle: AppHandle,
    ) -> Result<AppViewState> {
        let (view, work) = {
            let mut inner = self.lock()?;
            let question = inner.resolve_manual_question(question);
            if question.trim().is_empty() {
                inner.status = "No transcript question is available yet.".to_owned();
                return Ok(inner.build_view());
            }
            let transcript_id = inner.active_transcript_id.clone();
            let question_id = inner
                .storage
                .upsert_pending_question(&transcript_id, &question)?;
            inner.reload_transcripts()?;
            let work = inner.answer_work(&transcript_id, &question_id, &question);
            inner.status = "Generating answer...".to_owned();
            (inner.build_view(), work)
        };
        self.spawn_answer(work, app_handle);
        Ok(view)
    }

    /// Opens the Deepgram dashboard in the user's default browser.
    pub fn open_deepgram_site(&self) -> Result<()> {
        shell::open_url("https://console.deepgram.com/")
    }

    /// Opens the developer website in the user's default browser.
    pub fn open_developer_site(&self) -> Result<()> {
        shell::open_url("https://www.google.com")
    }

    /// Opens the source repository in the user's default browser.
    pub fn open_source_site(&self) -> Result<()> {
        shell::open_url("https://github.com/barissandbox/Interview")
    }

    /// Updates the status message.
    pub fn set_status(&self, message: &str) {
        if let Ok(mut inner) = self.lock() {
            inner.status = message.to_owned();
        }
    }

    /// Appends a final segment and returns the updated view.
    pub fn handle_final_segment(&self, source: &str, text: &str) -> Option<AppViewState> {
        match self.lock() {
            Ok(mut inner) => {
                let active_id = inner.active_transcript_id.clone();
                if let Err(error) = inner.storage.append_segment(&active_id, source, text) {
                    inner.status = error.to_string();
                    None
                } else if let Err(error) = inner.reload_transcripts() {
                    inner.status = error.to_string();
                    None
                } else {
                    Some(inner.build_view())
                }
            }
            Err(_) => None,
        }
    }

    /// Detects speaker questions and starts automatic answer generation.
    pub fn detect_and_answer_questions(&self, source: &str, text: &str, app_handle: AppHandle) {
        if source != SPEAKER_LABEL {
            return;
        }
        let (language, detection_text, previous_question) = match self.lock() {
            Ok(inner) => (
                inner.settings.language.clone(),
                inner.speaker_question_detection_text(),
                inner.linked_previous_question_for_segment(text),
            ),
            Err(_) => (String::new(), String::new(), None),
        };
        let questions = detect_questions(
            &detection_text,
            text,
            &language,
            previous_question.as_deref(),
        );
        if questions.is_empty() {
            return;
        }
        for question in questions {
            let (work, view) = match self.lock() {
                Ok(mut inner) => {
                    if inner.settings.answer_type.trim().is_empty() {
                        continue;
                    }
                    let transcript_id = inner.active_transcript_id.clone();
                    let Ok(Some(question_id)) =
                        inner.storage.append_question(&transcript_id, &question)
                    else {
                        continue;
                    };
                    if inner.reload_transcripts().is_err() {
                        continue;
                    }
                    let work = inner.answer_work(&transcript_id, &question_id, &question);
                    (work, inner.build_view())
                }
                Err(_) => continue,
            };
            let _ = app_handle.emit(
                "transcript-event",
                crate::app::events::UiEvent::State {
                    state: Box::new(view),
                },
            );
            self.spawn_answer(work, app_handle.clone());
        }
    }

    fn spawn_answer(&self, work: AnswerWork, app_handle: AppHandle) {
        let state = self.clone();
        self.runtime.spawn(async move {
            let result = state.run_answer(work.clone(), app_handle.clone()).await;
            match result {
                Ok(view) => {
                    let _ = app_handle.emit(
                        "transcript-event",
                        crate::app::events::UiEvent::State {
                            state: Box::new(view),
                        },
                    );
                }
                Err(error) => {
                    state.set_status(&format!("Could not generate answer: {error}"));
                    let _ = state.clear_pending_answer(&work, "");
                    let _ = app_handle.emit(
                        "transcript-event",
                        crate::app::events::UiEvent::Error {
                            message: error.to_string(),
                        },
                    );
                }
            }
        });
    }

    async fn run_answer(&self, work: AnswerWork, app_handle: AppHandle) -> Result<AppViewState> {
        let access = self.access_context().await?;
        let request = chatgpt::AnswerRequest {
            question: work.question.clone(),
            profile_text: work.profile.text.clone(),
            profile_file_name: work.profile.file_name.clone(),
            target_position: work.settings.target_position.clone(),
            language_label: language_label(&work.settings.language).to_owned(),
            model: work.settings.model.clone(),
            thinking_variant: work.settings.thinking_variant.clone(),
            answer_type: work.settings.answer_type.clone(),
            fast_enabled: work.settings.fast_enabled,
            verbosity: work.settings.verbosity.clone(),
        };
        let question_id = work.question_id.clone();
        let question = work.question.clone();
        let final_answer = chatgpt::stream_answer(&access, request, move |partial| {
            let _ = app_handle.emit(
                "transcript-event",
                crate::app::events::UiEvent::Answer {
                    question_id: question_id.clone(),
                    question: question.clone(),
                    answer: partial,
                    streaming: true,
                },
            );
        })
        .await?;
        self.clear_pending_answer(&work, &final_answer)
    }

    fn clear_pending_answer(&self, work: &AnswerWork, answer: &str) -> Result<AppViewState> {
        let mut inner = self.lock()?;
        inner.storage.set_question_answer(QuestionAnswerUpdate {
            transcript_id: &work.transcript_id,
            question_id: &work.question_id,
            answer,
            answer_type: &work.settings.answer_type,
            model: &work.settings.model,
            thinking_variant: &work.settings.thinking_variant,
            pending: false,
        })?;
        inner.reload_transcripts()?;
        inner.status = if answer.trim().is_empty() {
            "Answer failed.".to_owned()
        } else {
            "Answer ready.".to_owned()
        };
        Ok(inner.build_view())
    }

    async fn finish_chatgpt_login(&self, auth: AuthStorage) -> Result<AppViewState> {
        let access = chatgpt::AccessContext::from_auth(&auth);
        let mut catalog = chatgpt::fetch_model_catalog(&access)
            .await
            .unwrap_or_else(|_| CatalogStorage::default());
        catalog.chatgpt_limit_label = chatgpt::fetch_usage_limit_label(&access)
            .await
            .unwrap_or_default();
        let mut inner = self.lock()?;
        inner.auth = auth;
        inner.catalog = catalog;
        inner.settings.model = normalize_model_choice(&inner.settings.model, &inner.catalog);
        inner.settings.thinking_variant = normalize_thinking_choice(
            &inner.settings.thinking_variant,
            &inner.settings.model,
            &inner.catalog,
        );
        inner.status = "Signed in with ChatGPT.".to_owned();
        inner.storage.save_auth(&inner.auth)?;
        inner.storage.save_catalog(&inner.catalog)?;
        inner.storage.save_settings(&inner.settings)?;
        Ok(inner.build_view())
    }

    async fn access_context(&self) -> Result<chatgpt::AccessContext> {
        let auth = {
            let inner = self.lock()?;
            inner.auth.clone()
        };
        if auth.access_token.is_empty() && auth.refresh_token.is_empty() {
            return Err(anyhow!("Please sign in with ChatGPT first."));
        }
        if !auth.access_token.is_empty()
            && auth.expires_at > Utc::now().timestamp_millis() + 5 * 60 * 1000
        {
            return Ok(chatgpt::AccessContext::from_auth(&auth));
        }
        let refreshed = chatgpt::refresh_access_token(&auth).await?;
        let access = chatgpt::AccessContext::from_auth(&refreshed);
        let mut inner = self.lock()?;
        inner.auth = refreshed;
        inner.storage.save_auth(&inner.auth)?;
        Ok(access)
    }

    /// Locks the inner state.
    fn lock(&self) -> Result<MutexGuard<'_, InnerState>> {
        self.inner
            .lock()
            .map_err(|_| anyhow!("App state lock failed"))
    }
}

impl InnerState {
    /// Returns the active transcript.
    fn active_transcript(&self) -> Option<&TranscriptRecord> {
        self.transcripts
            .iter()
            .find(|item| item.id == self.active_transcript_id)
    }

    /// Returns recent speaker text for sentence-level question detection.
    fn speaker_question_detection_text(&self) -> String {
        let Some(transcript) = self.active_transcript() else {
            return String::new();
        };
        let mut parts = transcript
            .segments
            .iter()
            .rev()
            .filter(|segment| segment.source == SPEAKER_LABEL)
            .take(24)
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>();
        parts.reverse();
        tail_chars(&parts.join(" "), 4000)
    }

    /// Returns the previous question only when it belongs to the current or immediately previous speaker segment.
    fn linked_previous_question_for_segment(&self, current_text: &str) -> Option<String> {
        let transcript = self.active_transcript()?;
        let previous_question = transcript.questions.last()?.text.clone();
        let previous_question_key = normalize_question_candidate(&previous_question).to_lowercase();
        if previous_question_key.is_empty() {
            return None;
        }
        let current_key = normalize_question_candidate(current_text).to_lowercase();
        if current_key.contains(&previous_question_key) {
            return Some(previous_question);
        }
        let mut speaker_segments = transcript
            .segments
            .iter()
            .filter(|segment| segment.source == SPEAKER_LABEL)
            .rev();
        let _current_segment = speaker_segments.next()?;
        let previous_segment = speaker_segments.next()?;
        let previous_previous_segment = speaker_segments.next();
        let previous_context = previous_previous_segment
            .map(|segment| format!("{} {}", segment.text, previous_segment.text))
            .unwrap_or_else(|| previous_segment.text.clone());
        let previous_context_key = normalize_question_candidate(&previous_context).to_lowercase();
        if previous_context_key.contains(&previous_question_key) {
            Some(previous_question)
        } else {
            None
        }
    }

    /// Returns the active transcript index.
    fn active_transcript_index(&self) -> Option<usize> {
        self.transcripts
            .iter()
            .position(|item| item.id == self.active_transcript_id)
    }

    /// Reloads transcript files from storage.
    fn reload_transcripts(&mut self) -> Result<()> {
        self.transcripts = self.storage.load_transcripts()?;
        Ok(())
    }

    /// Persists the current active transcript id into settings.
    fn sync_active_id_to_settings(&mut self) -> Result<()> {
        self.settings.active_transcript_id = self.active_transcript_id.clone();
        self.storage.save_settings(&self.settings)
    }

    /// Stops all active capture resources.
    fn stop_capture(&mut self) {
        self.capture.stop();
        self.status = "Stopped.".to_owned();
    }

    /// Creates an answer work item from current state.
    fn answer_work(&self, transcript_id: &str, question_id: &str, question: &str) -> AnswerWork {
        AnswerWork {
            transcript_id: transcript_id.to_owned(),
            question_id: question_id.to_owned(),
            question: question.to_owned(),
            settings: self.settings.clone(),
            profile: self.profile.clone(),
        }
    }

    /// Resolves a manual answer input.
    fn resolve_manual_question(&self, input: Option<String>) -> String {
        if let Some(value) = input.map(|value| value.trim().to_owned())
            && !value.is_empty()
        {
            return value.chars().take(2000).collect();
        }
        if let Some(question) = self
            .active_transcript()
            .and_then(|transcript| transcript.questions.last())
        {
            return question.text.clone();
        }
        self.active_transcript()
            .map(|transcript| recent_transcript_tail(transcript, 2))
            .unwrap_or_default()
    }

    /// Returns serializable view state.
    fn build_view(&self) -> AppViewState {
        let active_index = self.active_transcript_index().unwrap_or(0);
        let active = self.active_transcript();
        let transcript_text = active.map(format_transcript_text).unwrap_or_default();
        let questions = active
            .map(|transcript| transcript.questions.clone())
            .unwrap_or_default();
        let selected_question_index = selected_question_index(&questions);
        let selected_question = if selected_question_index >= 0 {
            questions
                .get(selected_question_index as usize)
                .map(|item| item.text.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        let answer_text = if selected_question_index >= 0 {
            questions
                .get(selected_question_index as usize)
                .map(|item| item.answer.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        let answer_pending = if selected_question_index >= 0 {
            questions
                .get(selected_question_index as usize)
                .map(|item| item.pending)
                .unwrap_or(false)
        } else {
            false
        };
        AppViewState {
            settings: self.settings.clone(),
            balance: self.balance.clone(),
            status: self.status.clone(),
            chatgpt: ChatGptViewState {
                logged_in: !self.auth.access_token.is_empty()
                    || !self.auth.refresh_token.is_empty(),
                account_email: self.auth.account_email.clone(),
                limit_label: self.catalog.chatgpt_limit_label.clone(),
                error: self.auth.error.clone(),
            },
            profile: (&self.profile).into(),
            models: self.catalog.available_models.clone(),
            thinking_variants: thinking_variants_for_model(&self.settings.model, &self.catalog),
            transcripts: self
                .transcripts
                .iter()
                .map(|item| TranscriptSummary {
                    id: item.id.clone(),
                    label: item.list_label(),
                })
                .collect(),
            active_transcript_id: self.active_transcript_id.clone(),
            active_index,
            transcript_count: self.transcripts.len(),
            transcript_text,
            questions,
            selected_question_index,
            selected_question,
            answer_text,
            answer_pending,
            devices: self.devices.clone(),
            languages: language_options(),
            running: self.capture.is_running(),
        }
    }
}

fn normalize_model_choice(value: &str, catalog: &CatalogStorage) -> String {
    if catalog
        .available_models
        .iter()
        .any(|item| item.model == value)
    {
        value.to_owned()
    } else {
        catalog
            .available_models
            .iter()
            .find(|item| item.is_default)
            .or_else(|| catalog.available_models.first())
            .map(|item| item.model.clone())
            .unwrap_or_else(|| fallback_models()[0].model.clone())
    }
}

fn normalize_thinking_choice(value: &str, model: &str, catalog: &CatalogStorage) -> String {
    let variants = thinking_variants_for_model(model, catalog);
    if variants.iter().any(|item| item.value == value) {
        value.to_owned()
    } else {
        catalog
            .available_models
            .iter()
            .find(|item| item.model == model)
            .map(|item| item.default_thinking_variant.clone())
            .filter(|item| !item.is_empty())
            .unwrap_or_else(|| crate::domain::DEFAULT_THINKING_VARIANT.to_owned())
    }
}

fn thinking_variants_for_model(
    model: &str,
    catalog: &CatalogStorage,
) -> Vec<crate::domain::ThinkingVariantOption> {
    catalog
        .available_models
        .iter()
        .find(|item| item.model == model)
        .or_else(|| catalog.available_models.iter().find(|item| item.is_default))
        .or_else(|| catalog.available_models.first())
        .map(|item| item.thinking_variants.clone())
        .filter(|items| !items.is_empty())
        .unwrap_or_else(crate::domain::fallback_thinking_variants)
}

fn language_label(language: &str) -> &'static str {
    language_options()
        .into_iter()
        .find(|item| item.value == language)
        .map(|item| item.label)
        .unwrap_or("the same language as the interviewer")
}

fn sanitize_cv_text(text: &str) -> String {
    text.replace('\0', " ")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
