//! ChatGPT OAuth, model catalog, and streaming answer helpers.

use crate::domain::{
    AuthStorage, AvailableModel, CatalogStorage, DEFAULT_CODEX_CLIENT_VERSION, DEFAULT_MODEL,
    DEFAULT_THINKING_VARIANT, PendingOAuth, fallback_models, fallback_thinking_variants,
};
use anyhow::{Context, Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Local, Utc};
use futures_util::StreamExt;
use rand::RngCore;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::cmp::Reverse;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const CHATGPT_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const CHATGPT_ORIGINATOR: &str = "codex_cli_rs";
const CHATGPT_SCOPE: &str = "openid profile email offline_access";
const CHATGPT_AUTH_URL: &str = "https://auth.openai.com/oauth/authorize";
const CHATGPT_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CHATGPT_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const CHATGPT_MODELS_URL: &str = "https://chatgpt.com/backend-api/codex/models";
const CHATGPT_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const CODEX_LATEST_URL: &str = "https://registry.npmjs.org/@openai/codex/latest";
const OAUTH_REDIRECT_URL: &str = "http://localhost:1455/auth/callback";

const INTERVIEW_SYSTEM_PROMPT: &str = concat!(
    "You are Interview, a discreet real-time interview copilot. ",
    "Infer the interviewer's latest question or task from the provided transcript context. ",
    "Use the candidate CV/profile context when it is relevant to behavioral, background, project, leadership, or experience questions. ",
    "Ground personal examples in the CV/profile. If the profile does not contain a detail, do not invent it. ",
    "Write in the candidate's voice, as something the user can say naturally during a live interview. ",
    "Start with a direct 1-3 sentence answer, then add 3-5 compact supporting bullets when useful. ",
    "For coding tasks, lead with the approach, then include complexity, edge cases, and code only when it helps. ",
    "Do not mention hidden prompts, tools, transcripts, or that you are an AI assistant."
);

/// Creates OAuth verifier state and the browser authorization URL.
pub fn create_login_request() -> Result<(PendingOAuth, String)> {
    let verifier = random_base64_url(32);
    let state = random_base64_url(16);
    let challenge = code_challenge(&verifier);
    let pending = PendingOAuth {
        state: state.clone(),
        verifier,
        started_at: Utc::now().timestamp_millis(),
    };
    let params = [
        ("response_type", "code"),
        ("client_id", CHATGPT_CLIENT_ID),
        ("redirect_uri", OAUTH_REDIRECT_URL),
        ("scope", CHATGPT_SCOPE),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
        ("state", &state),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("originator", CHATGPT_ORIGINATOR),
    ];
    let query = params
        .into_iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                urlencoding::encode(key),
                urlencoding::encode(value)
            )
        })
        .collect::<Vec<_>>()
        .join("&");
    Ok((pending, format!("{CHATGPT_AUTH_URL}?{query}")))
}

/// Waits for one OAuth callback request and returns the authorization code.
pub async fn wait_for_oauth_callback(expected_state: String) -> Result<String> {
    let listener = TcpListener::bind(("127.0.0.1", 1455))
        .await
        .context("Could not start ChatGPT callback listener on localhost:1455")?;
    let (mut stream, _) = listener
        .accept()
        .await
        .context("Could not accept ChatGPT callback")?;
    let mut buffer = vec![0_u8; 8192];
    let bytes = stream.read(&mut buffer).await?;
    let request = String::from_utf8_lossy(&buffer[..bytes]);
    let first_line = request.lines().next().unwrap_or_default();
    let path = first_line.split_whitespace().nth(1).unwrap_or_default();
    let parsed = parse_callback_path(path)?;
    let response = if parsed.state == expected_state && !parsed.code.is_empty() {
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\r\nChatGPT sign-in complete. You can close this tab."
    } else {
        "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\n\r\nChatGPT sign-in failed. Return to Interview and try again."
    };
    let _ = stream.write_all(response.as_bytes()).await;
    if parsed.state != expected_state {
        return Err(anyhow!(
            "OAuth state mismatch. Please try signing in again."
        ));
    }
    if parsed.code.is_empty() {
        return Err(anyhow!(
            "The ChatGPT callback did not include an authorization code."
        ));
    }
    Ok(parsed.code)
}

/// Exchanges an OAuth authorization code for local auth storage.
pub async fn exchange_authorization_code(code: &str, verifier: &str) -> Result<AuthStorage> {
    let client = reqwest::Client::new();
    let response = client
        .post(CHATGPT_TOKEN_URL)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&[
            ("client_id", CHATGPT_CLIENT_ID),
            ("code", code),
            ("code_verifier", verifier),
            ("grant_type", "authorization_code"),
            ("redirect_uri", OAUTH_REDIRECT_URL),
        ])
        .send()
        .await
        .context("Could not exchange ChatGPT authorization code")?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "Token exchange failed with status {}",
            response.status()
        ));
    }
    parse_token_response(response.json::<Value>().await?)
}

/// Refreshes ChatGPT access credentials.
pub async fn refresh_access_token(auth: &AuthStorage) -> Result<AuthStorage> {
    if auth.refresh_token.is_empty() {
        return Err(anyhow!(
            "Your ChatGPT session expired. Please sign in again."
        ));
    }
    let client = reqwest::Client::new();
    let response = client
        .post(CHATGPT_TOKEN_URL)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", auth.refresh_token.as_str()),
            ("client_id", CHATGPT_CLIENT_ID),
        ])
        .send()
        .await
        .context("Could not refresh ChatGPT token")?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "Token refresh failed with status {}",
            response.status()
        ));
    }
    parse_token_response(response.json::<Value>().await?)
}

/// Fetches the latest model catalog.
pub async fn fetch_model_catalog(access: &AccessContext) -> Result<CatalogStorage> {
    let client_version = fetch_codex_client_version().await;
    let client = reqwest::Client::new();
    let response = client
        .get(format!(
            "{CHATGPT_MODELS_URL}?client_version={}",
            urlencoding::encode(&client_version)
        ))
        .headers(chatgpt_headers(access, "application/json", false)?)
        .send()
        .await
        .context("Could not fetch ChatGPT models")?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "ChatGPT models check failed with status {}",
            response.status()
        ));
    }
    let models = normalize_models_payload(response.json::<Value>().await?)?;
    Ok(CatalogStorage {
        available_models: models,
        codex_client_version: client_version,
        chatgpt_limit_label: String::new(),
    })
}

/// Fetches a compact ChatGPT usage/plan label for the status bar.
pub async fn fetch_usage_limit_label(access: &AccessContext) -> Result<String> {
    let client = reqwest::Client::new();
    let response = client
        .get(CHATGPT_USAGE_URL)
        .headers(chatgpt_headers(access, "application/json", false)?)
        .send()
        .await
        .context("Could not fetch ChatGPT usage limits")?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "ChatGPT limit check failed with status {}",
            response.status()
        ));
    }
    Ok(compact_usage_label(&response.json::<Value>().await?))
}

/// Streams one ChatGPT answer and calls `on_update` with visible partial text.
pub async fn stream_answer<F>(
    access: &AccessContext,
    request: AnswerRequest,
    mut on_update: F,
) -> Result<String>
where
    F: FnMut(String) + Send,
{
    let client = reqwest::Client::new();
    let instructions = [
        INTERVIEW_SYSTEM_PROMPT,
        answer_type_instruction(&request.answer_type),
    ]
    .join(" ");
    let mut body = serde_json::json!({
        "model": request.model,
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{ "type": "input_text", "text": build_prompt(&request) }]
        }],
        "stream": true,
        "store": false,
        "include": ["reasoning.encrypted_content"],
        "text": { "verbosity": request.verbosity },
        "reasoning": { "effort": request.thinking_variant, "summary": "auto" },
        "instructions": instructions
    });
    if request.fast_enabled {
        body["service_tier"] = Value::String("priority".to_owned());
    }
    let response = client
        .post(CHATGPT_RESPONSES_URL)
        .headers(chatgpt_headers(access, "text/event-stream", true)?)
        .json(&body)
        .send()
        .await
        .context("Could not reach ChatGPT")?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "ChatGPT request failed with status {status}. {}",
            body.chars().take(240).collect::<String>()
        ));
    }

    let mut text = String::new();
    let mut completed_text = String::new();
    let mut buffer = String::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Could not read ChatGPT response stream")?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        let lines = buffer
            .split('\n')
            .map(|line| line.trim_end_matches('\r').to_owned())
            .collect::<Vec<_>>();
        let complete_line_count = lines.len().saturating_sub(1);
        for line in lines.iter().take(complete_line_count) {
            if let Some(parsed) = parse_sse_line(line) {
                if !parsed.delta.is_empty() {
                    text.push_str(&parsed.delta);
                    on_update(text.trim().to_owned());
                }
                if !parsed.completed_text.is_empty() {
                    completed_text = parsed.completed_text;
                }
            }
        }
        buffer = lines.last().cloned().unwrap_or_default();
    }
    if let Some(parsed) = parse_sse_line(&buffer) {
        text.push_str(&parsed.delta);
        if !parsed.completed_text.is_empty() {
            completed_text = parsed.completed_text;
        }
    }
    let final_text = if text.trim().is_empty() {
        completed_text.trim().to_owned()
    } else {
        text.trim().to_owned()
    };
    Ok(if final_text.is_empty() {
        "No response text was returned.".to_owned()
    } else {
        final_text
    })
}

/// ChatGPT access context for request headers.
#[derive(Clone, Debug)]
pub struct AccessContext {
    pub access_token: String,
    pub chatgpt_account_id: String,
}

impl AccessContext {
    /// Builds request header context from persisted ChatGPT auth data.
    pub fn from_auth(auth: &AuthStorage) -> Self {
        Self {
            access_token: auth.access_token.clone(),
            chatgpt_account_id: if auth.chatgpt_account_id.is_empty() {
                read_jwt_claim(
                    &auth.access_token,
                    &["https://api.openai.com/auth", "chatgpt_account_id"],
                )
                .unwrap_or_default()
            } else {
                auth.chatgpt_account_id.clone()
            },
        }
    }
}

/// Inputs used to build an interview answer request.
pub struct AnswerRequest {
    pub question: String,
    pub profile_text: String,
    pub profile_file_name: String,
    pub target_position: String,
    pub language_label: String,
    pub model: String,
    pub thinking_variant: String,
    pub answer_type: String,
    pub fast_enabled: bool,
    pub verbosity: String,
}

struct CallbackParams {
    code: String,
    state: String,
}

struct SsePart {
    delta: String,
    completed_text: String,
}

async fn fetch_codex_client_version() -> String {
    let client = reqwest::Client::new();
    let Ok(response) = client
        .get(CODEX_LATEST_URL)
        .header(ACCEPT, "application/json")
        .send()
        .await
    else {
        return DEFAULT_CODEX_CLIENT_VERSION.to_owned();
    };
    if !response.status().is_success() {
        return DEFAULT_CODEX_CLIENT_VERSION.to_owned();
    }
    let Ok(payload) = response.json::<Value>().await else {
        return DEFAULT_CODEX_CLIENT_VERSION.to_owned();
    };
    payload
        .get("version")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(DEFAULT_CODEX_CLIENT_VERSION)
        .to_owned()
}

fn parse_callback_path(path: &str) -> Result<CallbackParams> {
    let query = path.split_once('?').map(|(_, query)| query).unwrap_or("");
    let mut code = String::new();
    let mut state = String::new();
    for part in query.split('&') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let decoded = urlencoding::decode(value)?.into_owned();
        match key {
            "code" => code = decoded,
            "state" => state = decoded,
            _ => {}
        }
    }
    Ok(CallbackParams { code, state })
}

fn parse_token_response(payload: Value) -> Result<AuthStorage> {
    let access_token = payload
        .get("access_token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let refresh_token = payload
        .get("refresh_token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let expires_in = payload
        .get("expires_in")
        .and_then(Value::as_i64)
        .or_else(|| {
            payload
                .get("expires_in")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<i64>().ok())
        })
        .unwrap_or_default();
    if access_token.is_empty() || refresh_token.is_empty() || expires_in <= 0 {
        return Err(anyhow!("ChatGPT returned an invalid token response."));
    }
    Ok(AuthStorage {
        account_email: read_jwt_claim(&access_token, &["https://api.openai.com/profile", "email"])
            .or_else(|| read_jwt_claim(&access_token, &["email"]))
            .unwrap_or_default(),
        chatgpt_account_id: read_jwt_claim(
            &access_token,
            &["https://api.openai.com/auth", "chatgpt_account_id"],
        )
        .unwrap_or_default(),
        access_token,
        refresh_token,
        expires_at: Utc::now().timestamp_millis() + expires_in * 1000,
        pending_oauth: None,
        error: String::new(),
    })
}

fn read_jwt_claim(token: &str, path: &[&str]) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let mut value: Value = serde_json::from_slice(&bytes).ok()?;
    for key in path {
        value = value.get(*key)?.clone();
    }
    value.as_str().map(str::to_owned)
}

fn chatgpt_headers(access: &AccessContext, accept: &str, json_content: bool) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_str(accept)?);
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", access.access_token))?,
    );
    headers.insert("originator", HeaderValue::from_static(CHATGPT_ORIGINATOR));
    if json_content {
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "OpenAI-Beta",
            HeaderValue::from_static("responses=experimental"),
        );
    }
    if !access.chatgpt_account_id.is_empty() {
        headers.insert(
            "chatgpt-account-id",
            HeaderValue::from_str(&access.chatgpt_account_id)?,
        );
    }
    Ok(headers)
}

fn build_prompt(request: &AnswerRequest) -> String {
    let mut parts = Vec::new();
    let profile = request.profile_text.trim();
    if !profile.is_empty() {
        let label = if request.profile_file_name.trim().is_empty() {
            "Candidate CV/profile context".to_owned()
        } else {
            format!(
                "Candidate CV/profile context ({}):",
                request.profile_file_name.trim()
            )
        };
        parts.push(format!(
            "{label}\n{}",
            profile.chars().take(12_000).collect::<String>()
        ));
    }
    if !request.target_position.trim().is_empty() {
        parts.push(format!(
            "Target position:\n{}",
            request.target_position.trim()
        ));
    }
    parts.push(format!("Question or task:\n{}", request.question.trim()));
    parts.push(format!(
        "Respond in {} unless the interviewer explicitly asks for another language.",
        request.language_label
    ));
    parts.push("Return the answer in a form the interviewee can say naturally. Keep it concise unless code or steps are required.".to_owned());
    parts.push("For experience questions, adapt the answer to the candidate profile without claiming facts that are not supported there.".to_owned());
    parts.join("\n\n")
}

fn answer_type_instruction(value: &str) -> &'static str {
    match value {
        "keywords" => {
            "Answer as quick keywords or very short bullet points only. Make it easy to glance at during conversation."
        }
        "sentences" => {
            "Answer in natural full sentences that the interviewee can read aloud word-for-word."
        }
        _ => {
            "Answer with bullet points that include concise context and explanations. Keep each bullet practical and interview-ready."
        }
    }
}

fn parse_sse_line(line: &str) -> Option<SsePart> {
    if !line.starts_with("data:") {
        return None;
    }
    let payload = line.trim_start_matches("data:").trim();
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }
    let event: Value = serde_json::from_str(payload).ok()?;
    let delta = extract_delta_text(&event).unwrap_or_default();
    let completed_text = if event.get("type").and_then(Value::as_str) == Some("response.completed")
    {
        extract_completed_text(event.get("response").unwrap_or(&event))
    } else {
        String::new()
    };
    Some(SsePart {
        delta,
        completed_text,
    })
}

fn extract_delta_text(event: &Value) -> Option<String> {
    let event_type = event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !(event_type == "response.output_text.delta"
        || (event_type.ends_with(".delta")
            && event_type.contains("output")
            && event_type.contains("text")
            && !event_type.contains("reasoning")))
    {
        return None;
    }
    event
        .get("delta")
        .and_then(Value::as_str)
        .or_else(|| {
            event
                .get("delta")
                .and_then(|value| value.get("text"))
                .and_then(Value::as_str)
        })
        .or_else(|| event.get("text").and_then(Value::as_str))
        .map(str::to_owned)
}

fn extract_completed_text(root: &Value) -> String {
    root.get("output")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
                .filter_map(|message| message.get("content").and_then(Value::as_array))
                .flat_map(|content| content.iter())
                .filter(|part| part.get("type").and_then(Value::as_str) == Some("output_text"))
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn normalize_models_payload(payload: Value) -> Result<Vec<AvailableModel>> {
    let mut models = payload
        .get("models")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(normalize_model)
                .collect::<Vec<AvailableModel>>()
        })
        .unwrap_or_default();
    models.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    if models.is_empty() {
        return Ok(fallback_models());
    }
    if !models.iter().any(|model| model.is_default)
        && let Some(first) = models.iter_mut().find(|model| !model.hidden)
    {
        first.is_default = true;
    }
    Ok(models)
}

fn compact_usage_label(payload: &Value) -> String {
    let plan = extract_plan_name(payload).unwrap_or_default();
    let limit = payload
        .get("rate_limit")
        .and_then(format_rate_limit)
        .or_else(|| {
            payload
                .get("additional_rate_limits")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items
                        .iter()
                        .find_map(|item| item.get("rate_limit").and_then(format_rate_limit))
                })
        });
    match (plan.trim(), limit) {
        ("", Some(limit)) => limit,
        ("", None) => String::new(),
        (plan, Some(limit)) => format!("{plan}, {limit}"),
        (plan, None) => plan.to_owned(),
    }
}

fn format_rate_limit(rate_limit: &Value) -> Option<String> {
    let mut windows = ["primary_window", "secondary_window"]
        .into_iter()
        .filter_map(|key| rate_limit.get(key))
        .filter_map(format_rate_limit_window)
        .collect::<Vec<_>>();
    windows.sort_by_key(|window| Reverse(window.minutes));
    let label = windows
        .into_iter()
        .map(|window| window.label)
        .collect::<Vec<_>>()
        .join(", ");
    if label.is_empty() { None } else { Some(label) }
}

struct UsageWindowLabel {
    minutes: i64,
    label: String,
}

fn format_rate_limit_window(window: &Value) -> Option<UsageWindowLabel> {
    let used_percent = number_at(window, "used_percent").unwrap_or(0.0).max(0.0);
    let left_percent = (100.0 - used_percent).max(0.0);
    let percent = format_percent(left_percent);
    let minutes = int_at(window, "limit_window_seconds").map(|value| (value + 59) / 60)?;
    if minutes <= 0 {
        return None;
    }
    let mut label = format!("{}: {percent}%", format_window(minutes));
    if let Some(reset_at) = reset_timestamp(window)
        && let Some(time) = format_reset_time(reset_at)
    {
        label.push_str(&format!(" resets {time}"));
    }
    Some(UsageWindowLabel { minutes, label })
}

fn format_reset_time(reset_at: i64) -> Option<String> {
    let reset_time = chrono::DateTime::from_timestamp(reset_at, 0)?.with_timezone(&Local);
    let now = Local::now();
    let pattern = if reset_time.date_naive() == now.date_naive() {
        "%H:%M"
    } else {
        "%d.%m %H:%M"
    };
    Some(reset_time.format(pattern).to_string())
}

fn extract_plan_name(value: &Value) -> Option<String> {
    let direct = [
        "plan",
        "plan_name",
        "plan_type",
        "subscription_plan",
        "subscription_tier",
        "account_plan",
        "tier",
    ];
    for key in direct {
        if let Some(plan) = normalize_plan_name(value.get(key).and_then(Value::as_str)) {
            return Some(plan);
        }
    }
    find_plan_name(value, 0)
}

fn find_plan_name(value: &Value, depth: usize) -> Option<String> {
    if depth > 4 {
        return None;
    }
    match value {
        Value::String(text) => normalize_plan_name(Some(text)),
        Value::Array(items) => items
            .iter()
            .find_map(|item| find_plan_name(item, depth + 1)),
        Value::Object(map) => map.iter().find_map(|(key, nested)| {
            let key = key.to_lowercase();
            if key.contains("plan") || key.contains("tier") || key.contains("subscription") {
                find_plan_name(nested, depth + 1)
            } else {
                None
            }
        }),
        _ => None,
    }
}

fn normalize_plan_name(value: Option<&str>) -> Option<String> {
    let normalized = value?.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }
    for plan in [
        "free",
        "plus",
        "pro",
        "team",
        "business",
        "enterprise",
        "edu",
    ] {
        if normalized == plan || normalized.contains(plan) {
            return Some(format!(
                "{}{}",
                plan[..1].to_uppercase(),
                plan[1..].to_owned()
            ));
        }
    }
    if normalized.contains("plan")
        || normalized.contains("tier")
        || normalized.contains("subscription")
    {
        return Some(prettify_label(&normalized));
    }
    None
}

fn number_at(value: &Value, key: &str) -> Option<f64> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .or_else(|| value.get(key).and_then(Value::as_str)?.parse::<f64>().ok())
}

fn int_at(value: &Value, key: &str) -> Option<i64> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .or_else(|| value.get(key).and_then(Value::as_str)?.parse::<i64>().ok())
}

fn reset_timestamp(value: &Value) -> Option<i64> {
    let absolute = [
        "reset_at",
        "resets_at",
        "resetAt",
        "resetsAt",
        "reset_timestamp",
        "resetTimestamp",
    ]
    .into_iter()
    .find_map(|key| int_at(value, key))
    .map(|timestamp| {
        if timestamp > 10_000_000_000 {
            timestamp / 1000
        } else {
            timestamp
        }
    })
    .filter(|timestamp| *timestamp > 0);
    if absolute.is_some() {
        return absolute;
    }
    [
        "reset_after_seconds",
        "resetAfterSeconds",
        "seconds_until_reset",
        "secondsUntilReset",
    ]
    .into_iter()
    .find_map(|key| int_at(value, key))
    .filter(|seconds| *seconds > 0)
    .map(|seconds| Utc::now().timestamp() + seconds)
}

fn format_percent(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        format!("{value:.1}")
    }
}

fn format_window(minutes: i64) -> String {
    if minutes % 1440 == 0 {
        format!("{}d", minutes / 1440)
    } else if minutes % 60 == 0 {
        format!("{}h", minutes / 60)
    } else {
        format!("{minutes}m")
    }
}

fn prettify_label(value: &str) -> String {
    value
        .replace(['_', '-'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_model(value: &Value) -> Option<AvailableModel> {
    let model = value
        .get("slug")
        .or_else(|| value.get("model"))
        .or_else(|| value.get("id"))
        .and_then(Value::as_str)?
        .trim()
        .to_owned();
    if model.is_empty() {
        return None;
    }
    let thinking_variants = value
        .get("supported_reasoning_levels")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let effort = item.get("effort").and_then(Value::as_str)?.trim();
                    if effort.is_empty() {
                        return None;
                    }
                    Some(crate::domain::ThinkingVariantOption {
                        value: effort.to_owned(),
                        description: item
                            .get("description")
                            .and_then(Value::as_str)
                            .unwrap_or(effort)
                            .to_owned(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(fallback_thinking_variants);
    Some(AvailableModel {
        id: value
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or(&model)
            .to_owned(),
        model: model.clone(),
        display_name: value
            .get("display_name")
            .or_else(|| value.get("displayName"))
            .and_then(Value::as_str)
            .unwrap_or(&model)
            .to_owned(),
        description: value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        hidden: value
            .get("hidden")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || value.get("visibility").and_then(Value::as_str) == Some("hide"),
        is_default: value
            .get("is_default")
            .and_then(Value::as_bool)
            .unwrap_or(model == DEFAULT_MODEL),
        input_modalities: value
            .get("input_modalities")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|items| !items.is_empty())
            .unwrap_or_else(|| vec!["text".to_owned(), "image".to_owned()]),
        default_thinking_variant: value
            .get("default_reasoning_level")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_THINKING_VARIANT)
            .to_owned(),
        thinking_variants,
    })
}

fn random_base64_url(byte_count: usize) -> String {
    let mut bytes = vec![0_u8; byte_count];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Extracts text from a PDF file buffer.
pub fn extract_pdf_text(bytes: &[u8]) -> Result<String> {
    pdf_extract::extract_text_from_mem(bytes).context("Could not extract readable CV text from PDF")
}
