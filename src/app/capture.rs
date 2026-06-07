//! Audio capture session orchestration.

use crate::domain::{AppSettings, MICROPHONE_LABEL, SPEAKER_LABEL};
use crate::infra::{audio, deepgram};
use anyhow::{Context, Result};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

/// Owns active audio streams and their Deepgram input channels.
pub struct CaptureSession {
    microphone_tx: Option<mpsc::Sender<Vec<u8>>>,
    speaker_tx: Option<mpsc::Sender<Vec<u8>>>,
    microphone_stream: Option<audio::AudioStreamHandle>,
    speaker_stream: Option<audio::AudioStreamHandle>,
    running: bool,
}

impl CaptureSession {
    /// Creates an idle capture session.
    pub fn new() -> Self {
        Self {
            microphone_tx: None,
            speaker_tx: None,
            microphone_stream: None,
            speaker_stream: None,
            running: false,
        }
    }

    /// Returns whether capture is considered active by the UI.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Starts streams for all enabled sources.
    pub fn start(
        &mut self,
        settings: &AppSettings,
        runtime: &Runtime,
        events_tx: &crossbeam_channel::Sender<deepgram::DeepgramEvent>,
    ) -> Result<()> {
        self.reconcile(settings, runtime, events_tx)?;
        self.running = true;
        Ok(())
    }

    /// Starts or stops source streams so they match the current settings.
    pub fn reconcile(
        &mut self,
        settings: &AppSettings,
        runtime: &Runtime,
        events_tx: &crossbeam_channel::Sender<deepgram::DeepgramEvent>,
    ) -> Result<()> {
        if settings.speaker_enabled {
            if self.speaker_stream.is_none() {
                self.start_speaker_source(settings, runtime, events_tx)?;
            }
        } else {
            self.speaker_tx = None;
            self.speaker_stream = None;
        }

        if settings.microphone_enabled {
            if self.microphone_stream.is_none() {
                self.start_microphone_source(settings, runtime, events_tx)?;
            }
        } else {
            self.microphone_tx = None;
            self.microphone_stream = None;
        }

        Ok(())
    }

    /// Stops all active capture resources.
    pub fn stop(&mut self) {
        self.microphone_tx = None;
        self.speaker_tx = None;
        self.microphone_stream = None;
        self.speaker_stream = None;
        self.running = false;
    }

    /// Starts speaker capture and its Deepgram worker.
    fn start_speaker_source(
        &mut self,
        settings: &AppSettings,
        runtime: &Runtime,
        events_tx: &crossbeam_channel::Sender<deepgram::DeepgramEvent>,
    ) -> Result<()> {
        let (tx, rx) = mpsc::channel(128);
        let handle = audio::start_speaker_loopback(&settings.speaker_device_id, tx.clone())
            .context("Speaker capture failed")?;
        self.speaker_stream = Some(handle);
        self.speaker_tx = Some(tx);
        spawn_deepgram_stream(SPEAKER_LABEL.to_owned(), rx, settings, runtime, events_tx);
        Ok(())
    }

    /// Starts microphone capture and its Deepgram worker.
    fn start_microphone_source(
        &mut self,
        settings: &AppSettings,
        runtime: &Runtime,
        events_tx: &crossbeam_channel::Sender<deepgram::DeepgramEvent>,
    ) -> Result<()> {
        let (tx, rx) = mpsc::channel(128);
        let handle = audio::start_microphone(&settings.microphone_device_id, tx.clone())
            .context("Mic capture failed")?;
        self.microphone_stream = Some(handle);
        self.microphone_tx = Some(tx);
        spawn_deepgram_stream(
            MICROPHONE_LABEL.to_owned(),
            rx,
            settings,
            runtime,
            events_tx,
        );
        Ok(())
    }
}

/// Builds the user-facing capture status from active source settings.
pub fn capture_status(settings: &AppSettings, running: bool) -> String {
    if !running {
        return "Stopped.".to_owned();
    }
    match (settings.speaker_enabled, settings.microphone_enabled) {
        (true, true) => "Listening to speaker and microphone.".to_owned(),
        (true, false) => "Listening to speaker.".to_owned(),
        (false, true) => "Listening to microphone.".to_owned(),
        (false, false) => "Capture paused. Turn Speaker or Mic on to resume.".to_owned(),
    }
}

/// Spawns one Deepgram streaming worker.
fn spawn_deepgram_stream(
    source: String,
    audio_rx: mpsc::Receiver<Vec<u8>>,
    settings: &AppSettings,
    runtime: &Runtime,
    events_tx: &crossbeam_channel::Sender<deepgram::DeepgramEvent>,
) {
    let api_key = settings.api_key.clone();
    let language = settings.language.clone();
    let events = events_tx.clone();
    runtime.spawn(async move {
        if let Err(error) =
            deepgram::stream_audio(api_key, language, source, audio_rx, events.clone()).await
        {
            let _ = events.send(deepgram::DeepgramEvent::Error(error.to_string()));
        }
    });
}
