const WORKER_URL: &str = "https://snaptext-worker.snaptext-ai.workers.dev";

use serde::Deserialize;
use reqwest::Client;
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};

// ── Per-mode model config ──────────────────────────────────────────────────

pub struct ModelConfig {
    pub temperature:     f32,
    pub max_tokens:      i32,
    pub thinking_budget: i32,
}

pub fn mode_config(mode: &str) -> ModelConfig {
    match mode {
        "Correct" | "Translate" => ModelConfig { temperature: 0.2, max_tokens: 400,  thinking_budget: 0    },
        "Summarize"             => ModelConfig { temperature: 0.3, max_tokens: 600,  thinking_budget: 0    },
        "Do" | "Reply"          => ModelConfig { temperature: 0.3, max_tokens: 1000, thinking_budget: 1024 },
        "Email"                 => ModelConfig { temperature: 0.3, max_tokens: 1200, thinking_budget: 1024 },
        "Prompt"                => ModelConfig { temperature: 0.4, max_tokens: 800,  thinking_budget: 512  },
        "Knowledge"             => ModelConfig { temperature: 0.5, max_tokens: 1500, thinking_budget: 2048 },
        "Strategist"            => ModelConfig { temperature: 0.5, max_tokens: 1200, thinking_budget: 1024 },
        "Casual"                => ModelConfig { temperature: 0.75,max_tokens: 600,  thinking_budget: 0    },
        "Professional"          => ModelConfig { temperature: 0.3, max_tokens: 600,  thinking_budget: 0    },
        _                       => ModelConfig { temperature: 0.5, max_tokens: 800,  thinking_budget: 0    },
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum AiMode {
    Worker,
    Byok,
    Local,
}

// ── Request / Response types ───────────────────────────────────────────────

#[derive(serde::Serialize)]
struct WorkerRequest {
    system_prompt:   String,
    user_text:       String,
    stream:          bool,
    max_tokens:      i32,
    temperature:     f32,
    thinking_budget: i32,
}

#[derive(Deserialize)]
struct GeminiStreamResponse { candidates: Option<Vec<Candidate>> }
#[derive(Deserialize)]
struct Candidate            { content: Option<CandidateContent> }
#[derive(Deserialize)]
struct CandidateContent     { parts: Option<Vec<ResponsePart>> }
#[derive(Deserialize)]
struct ResponsePart         { text: Option<String> }

fn extract_tokens(resp: &GeminiStreamResponse) -> Vec<String> {
    let mut tokens = Vec::new();
    if let Some(candidates) = &resp.candidates {
        for c in candidates {
            if let Some(content) = &c.content {
                if let Some(parts) = &content.parts {
                    for p in parts {
                        if let Some(t) = &p.text {
                            if !t.is_empty() { tokens.push(t.clone()); }
                        }
                    }
                }
            }
        }
    }
    tokens
}

/// Get-or-create a stable UUID v4 device ID stored in the DB config table.
/// Called once at startup; result is cached in AppState.
pub fn get_or_create_device_id(conn: &rusqlite::Connection) -> String {
    if let Ok(id) = crate::db::get_config(conn, "device_id") {
        if !id.is_empty() { return id; }
    }
    let new_id = uuid::Uuid::new_v4().to_string();
    let _ = crate::db::set_config(conn, "device_id", &new_id);
    new_id
}

use std::sync::OnceLock;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

fn make_client() -> Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(4)
            .build()
            .expect("Failed to build HTTP client — TLS/system error")
    }).clone()
}

fn worker_url(path: &str) -> String {
    format!("{}{}", WORKER_URL.trim_end_matches('/'), path)
}

fn add_device_header(req: reqwest::RequestBuilder, secret: &str, device_id: &str) -> reqwest::RequestBuilder {
    req.header("X-Device-ID", device_id)
       .header("X-App-Secret", secret)
}

pub async fn generate_stream(
    app: AppHandle,
    user_api_key: &str,
    system_prompt: &str,
    user_text: &str,
    mode: AiMode,
    config: &ModelConfig,
    worker_secret: &str,
    device_id: &str,
) -> Result<String, String> {
    match mode {
        AiMode::Local => {
            crate::ollama::generate_stream(Some(app), "phi3:mini", system_prompt, user_text).await
        },
        AiMode::Byok => {
            if user_api_key.is_empty() { return Err("No BYOK key configured".into()); }
            generate_direct_gemini(app, user_api_key, system_prompt, user_text, config).await
        },
        AiMode::Worker => {
            generate_worker_stream(app, system_prompt, user_text, config, worker_secret, device_id).await
        }
    }
}

async fn generate_worker_stream(
    app: AppHandle,
    system_prompt: &str,
    user_text: &str,
    config: &ModelConfig,
    worker_secret: &str,
    device_id: &str,
) -> Result<String, String> {
    let client = make_client();
    let body   = WorkerRequest {
        system_prompt:   system_prompt.to_string(),
        user_text:       user_text.to_string(),
        stream:          true,
        max_tokens:      config.max_tokens,
        temperature:     config.temperature,
        thinking_budget: config.thinking_budget,
    };

    let response = add_device_header(
        client.post(worker_url("/generate")).json(&body),
        worker_secret,
        device_id,
    )
    .send()
    .await
    .map_err(|e| format!("Network error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text   = response.text().await.unwrap_or_default();
        let msg = format!("Worker error {}: {}", status, text);
        app.emit("ai_error", &msg).ok();
        return Err(msg);
    }

    handle_stream_response(app, response).await
}

async fn generate_direct_gemini(
    app: AppHandle,
    key: &str,
    system_prompt: &str,
    user_text: &str,
    config: &ModelConfig,
) -> Result<String, String> {
    let client = make_client();
    let model  = "gemini-2.5-flash";
    let body = serde_json::json!({
        "systemInstruction": { "parts": [{ "text": system_prompt }] },
        "contents": [{ "role": "user", "parts": [{ "text": user_text }] }],
        "generationConfig": {
            "temperature": config.temperature,
            "maxOutputTokens": config.max_tokens,
            "thinkingConfig": { "thinkingBudget": config.thinking_budget }
        }
    });

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse", model);
    let response = client.post(url).header("x-goog-api-key", key).json(&body).send().await.map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Gemini API error: {}", response.status()));
    }

    handle_stream_response(app, response).await
}

async fn handle_stream_response(app: AppHandle, response: reqwest::Response) -> Result<String, String> {
    let mut stream      = response.bytes_stream();
    let mut buffer      = String::new();
    let mut full_output = String::new();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| format!("Stream error: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        let lines: Vec<&str> = buffer.lines().collect();
        let mut consumed = 0;
        for line in &lines {
            if line.starts_with("data: ") {
                let json_str = &line[6..];
                if json_str == "[DONE]" { consumed += 1; continue; }
                if let Ok(resp) = serde_json::from_str::<GeminiStreamResponse>(json_str) {
                    for token in extract_tokens(&resp) {
                        full_output.push_str(&token);
                        app.emit("ai_token", &token).ok();
                    }
                }
                consumed += 1;
            } else if line.is_empty() {
                consumed += 1;
            } else { break; }
        }
        if consumed > 0 { buffer = lines[consumed..].join("\n"); }
    }
    app.emit("ai_stream_end", ()).ok();
    Ok(full_output)
}

pub async fn generate_silent(
    user_api_key: &str,
    system_prompt: &str,
    user_text: &str,
    mode: AiMode,
    worker_secret: &str,
    device_id: &str,
) -> Result<String, String> {
    let config = ModelConfig { temperature: 0.3, max_tokens: 800, thinking_budget: 0 };
    match mode {
        AiMode::Local => {
            crate::ollama::generate_stream(None, "phi3:mini", system_prompt, user_text).await
        },
        AiMode::Byok => {
            if user_api_key.is_empty() { return Err("No BYOK key".into()); }
            generate_direct_gemini_silent(user_api_key, system_prompt, user_text, &config).await
        },
        AiMode::Worker => {
            generate_worker_silent(system_prompt, user_text, &config, worker_secret, device_id).await
        }
    }
}

async fn generate_worker_silent(
    system_prompt: &str,
    user_text: &str,
    config: &ModelConfig,
    worker_secret: &str,
    device_id: &str,
) -> Result<String, String> {
    let client = make_client();
    let body   = WorkerRequest {
        system_prompt:   system_prompt.to_string(),
        user_text:       user_text.to_string(),
        stream:          false,
        max_tokens:      config.max_tokens,
        temperature:     config.temperature,
        thinking_budget: config.thinking_budget,
    };

    let response = add_device_header(
        client.post(worker_url("/generate")).json(&body),
        worker_secret,
        device_id,
    )
    .send()
    .await
    .map_err(|e| format!("Network error: {}", e))?;

    if !response.status().is_success() {
        let err = response.text().await.unwrap_or_default();
        return Err(format!("Worker error: {}", err));
    }

    let data: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let text = data["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("").to_string();
    if text.trim().is_empty() {
        let finish = data["candidates"][0]["finishReason"].as_str().unwrap_or("UNKNOWN");
        return Err(format!("Empty response from model (finish: {})", finish));
    }
    Ok(text)
}

pub async fn classify_intent(
    api_key: &str,
    text: &str,
    mode: AiMode,
    worker_secret: &str,
    device_id: &str,
) -> Option<(String, f32, Vec<(String, f32)>)> {
    let prompt = format!("Classify intent: {}", text);
    let cfg = ModelConfig { temperature: 0.2, max_tokens: 200, thinking_budget: 0 };
    let res: Option<String> = match mode {
        AiMode::Local => {
            crate::ollama::generate_stream(None, "phi3:mini", "", &prompt).await.ok()
        },
        AiMode::Byok => {
            generate_direct_gemini_silent(api_key, "", &prompt, &cfg).await.ok()
        },
        AiMode::Worker => {
            generate_worker_silent("", &prompt, &cfg, worker_secret, device_id).await.ok()
        }
    };
    
    res.as_ref().and_then(|raw| parse_classifier_response(raw.as_str()))
}

async fn generate_direct_gemini_silent(
    key: &str,
    system_prompt: &str,
    user_text: &str,
    config: &ModelConfig,
) -> Result<String, String> {
    let client = make_client();
    let body = serde_json::json!({
        "systemInstruction": { "parts": [{ "text": system_prompt }] },
        "contents": [{ "role": "user", "parts": [{ "text": user_text }] }],
        "generationConfig": {
            "temperature": config.temperature,
            "maxOutputTokens": config.max_tokens,
            "thinkingConfig": { "thinkingBudget": config.thinking_budget }
        }
    });
    let url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";
    let response = client.post(url).header("x-goog-api-key", key).json(&body).send().await.map_err(|e| e.to_string())?;
    
    if !response.status().is_success() {
        return Err(format!("Gemini API error: {}", response.status()));
    }
    
    let data: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let text = data["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("").to_string();
    if text.trim().is_empty() {
        let finish = data["candidates"][0]["finishReason"].as_str().unwrap_or("UNKNOWN");
        return Err(format!("Empty response from model (finish: {})", finish));
    }
    Ok(text)
}

fn parse_classifier_response(body: &str) -> Option<(String, f32, Vec<(String, f32)>)> {
    // Try JSON first
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(label) = json.get("intent").and_then(|v| v.as_str()) {
            let conf = json.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.8) as f32;
            return Some((label.to_string(), conf, vec![]));
        }
    }
    // Fallback: keyword scan on raw text
    let lower = body.to_lowercase();
    if lower.contains("email") || lower.contains("mail")          { Some(("Email".into(),     0.8, vec![])) }
    else if lower.contains("task") || lower.contains("list") || lower.contains("todo") { Some(("Do".into(), 0.8, vec![])) }
    else if lower.contains("translat")                             { Some(("Translate".into(), 0.8, vec![])) }
    else if lower.contains("summar")                               { Some(("Summarize".into(), 0.8, vec![])) }
    else if lower.contains("reply") || lower.contains("respond")  { Some(("Reply".into(),     0.8, vec![])) }
    else if lower.contains("correct") || lower.contains("fix")    { Some(("Correct".into(),   0.8, vec![])) }
    else                                                           { Some(("Do".into(),        0.6, vec![])) }
}

pub async fn get_worker_usage(worker_secret: &str, device_id: &str) -> Option<(u32, u32)> {
    let client   = make_client();
    let response = add_device_header(
        client.get(worker_url("/usage")),
        worker_secret,
        device_id,
    )
    .send()
    .await
    .ok()?;

    if !response.status().is_success() { return None; }
    let data: serde_json::Value = response.json().await.ok()?;
    let used = data["used"].as_u64()? as u32;
    let cap  = data["cap"].as_u64().unwrap_or(20) as u32;
    Some((used, cap))
}

// ── Embedding fetch ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct EmbedResponse { embedding: EmbedValues }
#[derive(Deserialize)]
struct EmbedValues   { values: Vec<f32> }

/// Fetch a `text-embedding-004` vector via the Cloudflare Worker `/embed` proxy.
/// Does NOT count against the daily usage cap.
/// Returns `None` on any error so callers can fall back to BM25 gracefully.
pub async fn fetch_embedding(text: &str, worker_secret: &str, device_id: &str) -> Option<Vec<f32>> {
    let client = make_client();
    let body   = serde_json::json!({ "text": &text[..text.len().min(5000)] });
    let res    = add_device_header(
        client.post(worker_url("/embed")).json(&body),
        worker_secret,
        device_id,
    )
    .send()
    .await.ok()?;

    if !res.status().is_success() { return None; }
    let data: EmbedResponse = res.json().await.ok()?;
    if data.embedding.values.is_empty() { return None; }
    Some(data.embedding.values)
}

pub async fn list_models() -> Result<String, String> {
    let local = crate::ollama::list_local_models().await;
    let mut models = vec!["gemini-2.5-flash (Cloud)".to_string()];
    for m in local {
        models.push(format!("{} (Local)", m));
    }
    Ok(models.join(","))
}
