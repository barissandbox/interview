//! Transcript and view-state helper functions.

use crate::domain::{AppSettings, AudioDevice, AudioSourceKind, QuestionRecord, TranscriptRecord};

/// Selects default audio devices when no saved device exists.
pub fn apply_default_devices(settings: &mut AppSettings, devices: &[AudioDevice]) {
    if settings.speaker_device_id.is_empty() {
        settings.speaker_device_id =
            default_device_id(devices, AudioSourceKind::Speaker).unwrap_or_default();
    }
    if settings.microphone_device_id.is_empty() {
        settings.microphone_device_id =
            default_device_id(devices, AudioSourceKind::Microphone).unwrap_or_default();
    }
}

/// Resolves which transcript id should be active.
pub fn resolve_active_id(settings: &AppSettings, transcripts: &[TranscriptRecord]) -> String {
    if transcripts
        .iter()
        .any(|item| item.id == settings.active_transcript_id)
    {
        settings.active_transcript_id.clone()
    } else {
        transcripts
            .first()
            .map(|item| item.id.clone())
            .unwrap_or_default()
    }
}

/// Joins saved transcript segments into display text.
pub fn format_transcript_text(transcript: &TranscriptRecord) -> String {
    transcript
        .segments
        .iter()
        .map(|segment| segment.text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Returns the selected question index from the active question list.
pub fn selected_question_index(questions: &[QuestionRecord]) -> isize {
    if questions.is_empty() {
        -1
    } else {
        questions.len() as isize - 1
    }
}

/// Returns a compact recent transcript tail for manual answer generation.
pub fn recent_transcript_tail(transcript: &TranscriptRecord, count: usize) -> String {
    let text = format_transcript_text(transcript);
    if text.trim().is_empty() {
        return String::new();
    }
    let sentences = text
        .split_inclusive(['.', '!', '?'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    if sentences.is_empty() {
        return text.chars().take(2000).collect();
    }
    sentences
        .iter()
        .rev()
        .take(count)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(2000)
        .collect()
}

/// Returns the default device id for a source kind.
fn default_device_id(devices: &[AudioDevice], kind: AudioSourceKind) -> Option<String> {
    devices
        .iter()
        .filter(|device| device.kind == kind)
        .find(|device| device.is_default)
        .or_else(|| devices.iter().find(|device| device.kind == kind))
        .map(|device| device.id.clone())
}
