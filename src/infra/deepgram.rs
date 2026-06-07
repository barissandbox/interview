//! Deepgram HTTP validation, balance lookup, and realtime streaming.

use crate::domain::{DeepgramAccountStatus, model_for_language, normalize_language};
use anyhow::{Context, Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use http::header::{AUTHORIZATION, SEC_WEBSOCKET_PROTOCOL};
use serde_json::Value;
use tokio::sync::mpsc::Receiver;
use tokio::time::{self, Duration, MissedTickBehavior};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(5);
const SILENCE_FRAME_BYTES: usize = 16_000 / 10 * 2;
const KEEPALIVE_MESSAGE: &str = r#"{"type":"KeepAlive"}"#;
const CLOSE_STREAM_MESSAGE: &str = r#"{"type":"CloseStream"}"#;

/// Event emitted by the Deepgram worker.
#[derive(Debug)]
pub enum DeepgramEvent {
    /// Informational status update.
    Status(String),
    /// Interim transcript text.
    Interim { text: String },
    /// Final transcript text.
    Final { source: String, text: String },
    /// Worker error.
    Error(String),
}

/// Validates a Deepgram key and returns balance metadata when available.
pub async fn test_key_and_balance(api_key: &str) -> Result<DeepgramAccountStatus> {
    if api_key.trim().is_empty() {
        return Ok(DeepgramAccountStatus {
            valid: false,
            message: "Enter a Deepgram API key first.".to_owned(),
            balance_label: String::new(),
        });
    }

    let client = reqwest::Client::new();
    let auth = client
        .get("https://api.deepgram.com/v1/auth/token")
        .header("Authorization", format!("Token {}", api_key.trim()))
        .send()
        .await
        .context("Could not reach Deepgram")?;
    if !auth.status().is_success() {
        return Ok(DeepgramAccountStatus {
            valid: false,
            message: format!("Deepgram rejected the API key ({})", auth.status()),
            balance_label: String::new(),
        });
    }

    let balance_label = fetch_balance_label(&client, api_key)
        .await
        .unwrap_or_default();
    let message = "Deepgram API key verified and saved.".to_owned();
    Ok(DeepgramAccountStatus {
        valid: true,
        message,
        balance_label,
    })
}

/// Streams one audio source as PCM to Deepgram.
pub async fn stream_audio(
    api_key: String,
    language: String,
    source: String,
    mut audio: Receiver<Vec<u8>>,
    events: crossbeam_channel::Sender<DeepgramEvent>,
) -> Result<()> {
    let url = listen_url(&language);
    let (socket, _) = connect_deepgram(&url, &api_key).await?;
    let (mut writer, mut reader) = socket.split();
    let _ = events.send(DeepgramEvent::Status("Deepgram connected.".to_owned()));

    let event_reader = events.clone();
    let read_task = tokio::spawn(async move {
        while let Some(message) = reader.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    parse_deepgram_message(text.as_str(), &source, &event_reader)
                }
                Ok(Message::Close(close)) => {
                    let detail = close
                        .map(|frame| frame.reason.to_string())
                        .unwrap_or_else(|| "socket closed".to_owned());
                    let _ = event_reader
                        .send(DeepgramEvent::Status(format!("Deepgram closed: {detail}")));
                    break;
                }
                Err(error) => {
                    let _ = event_reader.send(DeepgramEvent::Error(error.to_string()));
                    break;
                }
                _ => {}
            }
        }
    });

    let mut keepalive = time::interval(KEEPALIVE_INTERVAL);
    keepalive.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            chunk = audio.recv() => {
                let Some(chunk) = chunk else {
                    break;
                };
                if !chunk.is_empty() {
                    writer.send(Message::Binary(chunk.into())).await?;
                }
            }
            _ = keepalive.tick() => {
                writer.send(Message::Binary(silent_pcm_frame().into())).await?;
                writer.send(Message::Text(KEEPALIVE_MESSAGE.into())).await?;
            }
        }
    }

    let _ = writer
        .send(Message::Text(CLOSE_STREAM_MESSAGE.into()))
        .await;
    let _ = writer.close().await;
    let _ = read_task.await;
    Ok(())
}

/// Builds 100 ms of 16 kHz mono PCM16 silence.
fn silent_pcm_frame() -> Vec<u8> {
    vec![0; SILENCE_FRAME_BYTES]
}

/// Connects to Deepgram with the same auth protocol fallback as the browser example.
async fn connect_deepgram(
    url: &str,
    api_key: &str,
) -> Result<(
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    http::Response<Option<Vec<u8>>>,
)> {
    let trimmed_key = api_key.trim();
    let attempts = [
        AuthAttempt::Protocol("token"),
        AuthAttempt::Protocol("bearer"),
        AuthAttempt::Header,
    ];
    let mut last_error = String::new();
    for attempt in attempts {
        let mut request = url
            .into_client_request()
            .context("Could not build Deepgram request")?;
        match attempt {
            AuthAttempt::Protocol(protocol) => {
                request.headers_mut().insert(
                    SEC_WEBSOCKET_PROTOCOL,
                    format!("{protocol}, {trimmed_key}").parse()?,
                );
            }
            AuthAttempt::Header => {
                request
                    .headers_mut()
                    .insert(AUTHORIZATION, format!("Token {trimmed_key}").parse()?);
            }
        }
        match tokio_tungstenite::connect_async(request).await {
            Ok(connection) => return Ok(connection),
            Err(error) => last_error = error.to_string(),
        }
    }
    Err(anyhow!("Could not connect to Deepgram: {last_error}"))
}

/// Deepgram WebSocket authentication attempt.
enum AuthAttempt {
    /// Browser-style WebSocket protocol authentication.
    Protocol(&'static str),
    /// Authorization header fallback.
    Header,
}

/// Builds the realtime listen URL.
fn listen_url(language: &str) -> String {
    let language = normalize_language(language);
    let model = model_for_language(&language);
    let query = [
        ("model", model.to_owned()),
        ("language", language),
        ("encoding", "linear16".to_owned()),
        ("sample_rate", "16000".to_owned()),
        ("channels", "1".to_owned()),
        ("smart_format", "true".to_owned()),
        ("interim_results", "true".to_owned()),
        ("vad_events", "true".to_owned()),
        ("punctuate", "true".to_owned()),
        ("utterance_end_ms", "1000".to_owned()),
    ]
    .into_iter()
    .map(|(key, value)| {
        format!(
            "{}={}",
            urlencoding::encode(key),
            urlencoding::encode(&value)
        )
    })
    .collect::<Vec<_>>()
    .join("&");
    format!("wss://api.deepgram.com/v1/listen?{query}")
}

/// Fetches a formatted balance label.
async fn fetch_balance_label(client: &reqwest::Client, api_key: &str) -> Result<String> {
    let projects_response = client
        .get("https://api.deepgram.com/v1/projects")
        .header("Authorization", format!("Token {}", api_key.trim()))
        .header("Accept", "application/json")
        .send()
        .await?;
    if !projects_response.status().is_success() {
        log::warn!(
            "Deepgram projects lookup failed with {}",
            projects_response.status()
        );
        return Ok(String::new());
    }
    let projects_json: Value = projects_response.json().await?;
    let Some(project_id) = projects_json
        .get("projects")
        .and_then(Value::as_array)
        .and_then(|projects| projects.first())
        .and_then(|project| project.get("project_id"))
        .and_then(Value::as_str)
    else {
        log::warn!("Deepgram returned no project for balance lookup");
        return Ok(String::new());
    };

    let balances_response = client
        .get(format!(
            "https://api.deepgram.com/v1/projects/{project_id}/balances"
        ))
        .header("Authorization", format!("Token {}", api_key.trim()))
        .header("Accept", "application/json")
        .send()
        .await?;
    if !balances_response.status().is_success() {
        log::warn!(
            "Deepgram balance lookup failed with {}",
            balances_response.status()
        );
        return Ok(String::new());
    }
    let balances_json: Value = balances_response.json().await?;
    let balances = balances_json
        .get("balances")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(format_balances(&balances))
}

/// Formats Deepgram balance entries by summing amounts per unit.
fn format_balances(balances: &[Value]) -> String {
    if balances.is_empty() {
        return "Deepgram: no balance data".to_owned();
    }

    let mut totals = std::collections::BTreeMap::<String, f64>::new();
    for balance in balances {
        let amount = balance
            .get("amount")
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let units = balance
            .get("units")
            .or_else(|| balance.get("currency"))
            .and_then(Value::as_str)
            .unwrap_or("UNITS")
            .trim()
            .to_uppercase();
        let key = if units.is_empty() {
            "UNITS".to_owned()
        } else {
            units
        };
        *totals.entry(key).or_default() += amount;
    }

    let text = totals
        .into_iter()
        .map(|(units, amount)| {
            if units == "USD" {
                format!("${amount:.2}")
            } else if amount.fract() == 0.0 {
                format!("{amount:.0} {units}")
            } else {
                format!("{amount:.2} {units}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("Deepgram: {text}")
}

/// Parses one Deepgram JSON message.
fn parse_deepgram_message(
    text: &str,
    source: &str,
    events: &crossbeam_channel::Sender<DeepgramEvent>,
) {
    let parsed: Result<Value> = serde_json::from_str(text).map_err(|error| anyhow!(error));
    let Ok(json) = parsed else {
        return;
    };
    let transcript = json
        .get("channel")
        .and_then(|channel| channel.get("alternatives"))
        .and_then(Value::as_array)
        .and_then(|alternatives| alternatives.first())
        .and_then(|first| first.get("transcript"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned();
    if transcript.is_empty() {
        if let Some(message) = json.get("message").and_then(Value::as_str) {
            let _ = events.send(DeepgramEvent::Status(message.to_owned()));
        }
        return;
    }
    let is_final = json
        .get("is_final")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || json
            .get("speech_final")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let event = if is_final {
        DeepgramEvent::Final {
            source: source.to_owned(),
            text: transcript,
        }
    } else {
        DeepgramEvent::Interim { text: transcript }
    };
    let _ = events.send(event);
}
