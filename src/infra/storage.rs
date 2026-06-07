//! JSON file persistence for transcripts, settings, auth, and profile data.

use crate::domain::{
    AppSettings, AuthStorage, CandidateProfile, CatalogStorage, QuestionRecord, TranscriptRecord,
    TranscriptSegment, normalize_answer_type, normalize_language, normalize_verbosity,
};
use crate::infra::paths::AppPaths;
use anyhow::{Context, Result};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;

/// File-backed repository for local app data.
#[derive(Clone, Debug)]
pub struct Storage {
    settings: PathBuf,
    auth: PathBuf,
    profile: PathBuf,
    catalog: PathBuf,
    transcripts: PathBuf,
}

/// Update payload for one stored question answer.
pub struct QuestionAnswerUpdate<'a> {
    pub transcript_id: &'a str,
    pub question_id: &'a str,
    pub answer: &'a str,
    pub answer_type: &'a str,
    pub model: &'a str,
    pub thinking_variant: &'a str,
    pub pending: bool,
}

impl Storage {
    /// Creates a repository and ensures required folders exist.
    pub fn new(paths: &AppPaths) -> Result<Self> {
        fs::create_dir_all(&paths.data_dir).context("Could not create app data directory")?;
        fs::create_dir_all(&paths.transcripts)
            .context("Could not create transcript data directory")?;
        Ok(Self {
            settings: paths.settings.clone(),
            auth: paths.auth.clone(),
            profile: paths.profile.clone(),
            catalog: paths.catalog.clone(),
            transcripts: paths.transcripts.clone(),
        })
    }

    /// Loads saved settings from settings.json.
    pub fn load_settings(&self) -> Result<AppSettings> {
        if !self.settings.exists() {
            return Ok(AppSettings::default());
        }
        let text = fs::read_to_string(&self.settings).context("Could not read settings.json")?;
        let mut settings: AppSettings =
            serde_json::from_str(&text).context("Could not parse settings.json")?;
        settings.language = normalize_language(&settings.language);
        settings.answer_type = normalize_answer_type(&settings.answer_type);
        settings.verbosity = normalize_verbosity(&settings.verbosity);
        if settings.model.trim().is_empty() {
            settings.model = AppSettings::default().model;
        }
        if settings.thinking_variant.trim().is_empty() {
            settings.thinking_variant = AppSettings::default().thinking_variant;
        }
        settings.target_position = settings.target_position.trim().chars().take(160).collect();
        Ok(settings)
    }

    /// Saves settings to settings.json.
    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        write_pretty(&self.settings, settings, "settings")
    }

    /// Loads ChatGPT auth data.
    pub fn load_auth(&self) -> Result<AuthStorage> {
        read_pretty_or_default(&self.auth, "auth")
    }

    /// Saves ChatGPT auth data.
    pub fn save_auth(&self, auth: &AuthStorage) -> Result<()> {
        write_pretty(&self.auth, auth, "auth")
    }

    /// Loads optional CV/profile context.
    pub fn load_profile(&self) -> Result<CandidateProfile> {
        read_pretty_or_default(&self.profile, "profile")
    }

    /// Saves optional CV/profile context.
    pub fn save_profile(&self, profile: &CandidateProfile) -> Result<()> {
        write_pretty(&self.profile, profile, "profile")
    }

    /// Removes the stored CV/profile context.
    pub fn clear_profile(&self) -> Result<()> {
        if self.profile.exists() {
            fs::remove_file(&self.profile).context("Could not remove profile.json")?;
        }
        Ok(())
    }

    /// Loads cached ChatGPT model catalog.
    pub fn load_catalog(&self) -> Result<CatalogStorage> {
        read_pretty_or_default(&self.catalog, "catalog")
    }

    /// Saves cached ChatGPT model catalog.
    pub fn save_catalog(&self, catalog: &CatalogStorage) -> Result<()> {
        write_pretty(&self.catalog, catalog, "catalog")
    }

    /// Loads all transcript JSON files.
    pub fn load_transcripts(&self) -> Result<Vec<TranscriptRecord>> {
        let mut transcripts = Vec::new();
        for entry in fs::read_dir(&self.transcripts).context("Could not read transcript data")? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let text = fs::read_to_string(&path)
                .with_context(|| format!("Could not read transcript file {}", path.display()))?;
            match serde_json::from_str::<TranscriptRecord>(&text) {
                Ok(transcript) => transcripts.push(transcript),
                Err(error) => log::error!("Could not parse transcript {}: {error}", path.display()),
            }
        }
        transcripts.sort_by_key(|transcript| transcript.created_at);
        Ok(transcripts)
    }

    /// Creates a new transcript JSON file.
    pub fn create_transcript(&self, language: &str) -> Result<TranscriptRecord> {
        let now = Utc::now();
        let transcript = TranscriptRecord {
            id: format!(
                "{}-{}",
                now.timestamp_millis(),
                now.timestamp_subsec_nanos()
            ),
            language: language.to_owned(),
            created_at: now,
            updated_at: now,
            segments: Vec::new(),
            questions: Vec::new(),
        };
        self.save_transcript(&transcript)?;
        Ok(transcript)
    }

    /// Deletes a transcript JSON file.
    pub fn delete_transcript(&self, transcript_id: &str) -> Result<()> {
        let path = self.transcript_path(transcript_id);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Could not delete transcript {}", path.display()))?;
        }
        Ok(())
    }

    /// Appends a final transcript segment to a transcript JSON file.
    pub fn append_segment(&self, transcript_id: &str, source: &str, text: &str) -> Result<()> {
        let mut transcript = self
            .load_transcript(transcript_id)?
            .with_context(|| format!("Transcript {transcript_id} was not found"))?;
        let now = Utc::now();
        transcript.segments.push(TranscriptSegment {
            id: transcript.segments.len() as i64 + 1,
            source: source.to_owned(),
            text: text.trim().to_owned(),
            created_at: now,
        });
        transcript.updated_at = now;
        self.save_transcript(&transcript)
    }

    /// Adds a question when it has not already been detected.
    pub fn append_question(&self, transcript_id: &str, question: &str) -> Result<Option<String>> {
        let question = question.trim();
        if question.is_empty() {
            return Ok(None);
        }
        let mut transcript = self
            .load_transcript(transcript_id)?
            .with_context(|| format!("Transcript {transcript_id} was not found"))?;
        let key = normalize_question_key(question);
        if key.is_empty() || transcript.questions.iter().any(|item| item.key == key) {
            return Ok(None);
        }
        let now = Utc::now();
        let id = format!(
            "q-{}-{}",
            now.timestamp_millis(),
            transcript.questions.len() + 1
        );
        transcript.questions.push(QuestionRecord {
            id: id.clone(),
            key,
            text: question.to_owned(),
            answer: String::new(),
            pending: true,
            answer_type: String::new(),
            model: String::new(),
            thinking_variant: String::new(),
            created_at: now,
            updated_at: now,
        });
        transcript.updated_at = now;
        self.save_transcript(&transcript)?;
        Ok(Some(id))
    }

    /// Marks a question as pending and returns its id.
    pub fn upsert_pending_question(&self, transcript_id: &str, question: &str) -> Result<String> {
        let question = question.trim();
        let mut transcript = self
            .load_transcript(transcript_id)?
            .with_context(|| format!("Transcript {transcript_id} was not found"))?;
        let key = normalize_question_key(question);
        if let Some(existing) = transcript.questions.iter_mut().find(|item| item.key == key) {
            existing.pending = true;
            existing.updated_at = Utc::now();
            let id = existing.id.clone();
            self.save_transcript(&transcript)?;
            return Ok(id);
        }
        let now = Utc::now();
        let id = format!(
            "q-{}-{}",
            now.timestamp_millis(),
            transcript.questions.len() + 1
        );
        transcript.questions.push(QuestionRecord {
            id: id.clone(),
            key,
            text: question.to_owned(),
            answer: String::new(),
            pending: true,
            answer_type: String::new(),
            model: String::new(),
            thinking_variant: String::new(),
            created_at: now,
            updated_at: now,
        });
        transcript.updated_at = now;
        self.save_transcript(&transcript)?;
        Ok(id)
    }

    /// Updates one question with generated answer text.
    pub fn set_question_answer(&self, update: QuestionAnswerUpdate<'_>) -> Result<()> {
        let mut transcript = self
            .load_transcript(update.transcript_id)?
            .with_context(|| format!("Transcript {} was not found", update.transcript_id))?;
        let Some(question) = transcript
            .questions
            .iter_mut()
            .find(|item| item.id == update.question_id)
        else {
            return Ok(());
        };
        question.answer = update.answer.trim().to_owned();
        question.pending = update.pending;
        question.answer_type = update.answer_type.to_owned();
        question.model = update.model.to_owned();
        question.thinking_variant = update.thinking_variant.to_owned();
        question.updated_at = Utc::now();
        transcript.updated_at = question.updated_at;
        self.save_transcript(&transcript)
    }

    /// Loads one transcript by id.
    fn load_transcript(&self, transcript_id: &str) -> Result<Option<TranscriptRecord>> {
        let path = self.transcript_path(transcript_id);
        if !path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&path)
            .with_context(|| format!("Could not read transcript {}", path.display()))?;
        let transcript =
            serde_json::from_str(&text).context("Could not parse transcript JSON file")?;
        Ok(Some(transcript))
    }

    /// Saves one transcript JSON file.
    fn save_transcript(&self, transcript: &TranscriptRecord) -> Result<()> {
        let path = self.transcript_path(&transcript.id);
        write_pretty(&path, transcript, "transcript")
    }

    /// Returns the JSON path for a transcript id.
    fn transcript_path(&self, transcript_id: &str) -> PathBuf {
        self.transcripts.join(format!("{transcript_id}.json"))
    }
}

/// Creates a stable comparison key for question text.
pub fn normalize_question_key(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(180)
        .collect()
}

fn read_pretty_or_default<T>(path: &PathBuf, label: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let text = fs::read_to_string(path).with_context(|| format!("Could not read {label}.json"))?;
    serde_json::from_str(&text).with_context(|| format!("Could not parse {label}.json"))
}

fn write_pretty<T>(path: &PathBuf, value: &T, label: &str) -> Result<()>
where
    T: serde::Serialize,
{
    let text = serde_json::to_string_pretty(value)
        .with_context(|| format!("Could not serialize {label}"))?;
    fs::write(path, text).with_context(|| format!("Could not write {}", path.display()))
}
