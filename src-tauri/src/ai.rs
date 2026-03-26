const WORKER_URL: &str = "https://snaptext-worker.snaptext-ai.workers.dev";

use serde::Deserialize;
use reqwest::Client;
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};

// ── Request / Response types ───────────────────────────────────────────────

#[derive(serde::Serialize)]
struct WorkerRequest {
    system_prompt: String,
    user_text:     String,
    stream:        bool,
    max_tokens:    i32,
    temperature:   f32,
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

pub fn device_id() -> String {
    let computer = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".into());
    let user     = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".into());
    let input    = format!("{}:{}", computer, user);
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000000001b3);
    }
    format!("{:x}", hash)
}

fn make_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_default()
}

fn worker_url(path: &str) -> String {
    format!("{}{}", WORKER_URL.trim_end_matches('/'), path)
}

fn add_device_header(req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    req.header("X-Device-ID", device_id())
}

pub async fn generate_stream(
    app: AppHandle,
    _user_api_key: &str,
    system_prompt: &str,
    user_text: &str,
) -> Result<String, String> {
    let client = make_client();
    let body   = WorkerRequest {
        system_prompt: system_prompt.to_string(),
        user_text:     user_text.to_string(),
        stream:        true,
        max_tokens:    4096,
        temperature:   0.7,
    };

    let response = add_device_header(
        client.post(worker_url("/generate")).json(&body)
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
    _user_api_key: &str,
    system_prompt: &str,
    user_text: &str,
) -> Result<String, String> {
    let client = make_client();
    let body   = WorkerRequest {
        system_prompt: system_prompt.to_string(),
        user_text:     user_text.to_string(),
        stream:        false,
        max_tokens:    4096,
        temperature:   0.7,
    };

    let response = add_device_header(
        client.post(worker_url("/generate")).json(&body)
    )
    .send()
    .await
    .map_err(|e| format!("Network error: {}", e))?;

    if !response.status().is_success() { return Err("Worker error".to_string()); }

    let data: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let text = data["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("").to_string();
    Ok(text)
}

pub async fn classify_intent(
    _api_key: &str,
    text: &str,
) -> Option<(String, f32, Vec<(String, f32)>)> {
    let client  = make_client();
    let body = WorkerRequest {
        system_prompt: String::new(),
        user_text:     format!("Classify intent: {}", text),
        stream:        false,
        max_tokens:    150,
        temperature:   0.0,
    };

    let response = add_device_header(
        client.post(worker_url("/generate")).json(&body)
    )
    .send()
    .await
    .ok()?;

    if !response.status().is_success() { return None; }
    let data: serde_json::Value = response.json().await.ok()?;
    let raw = data["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("");
    parse_classifier_response(raw)
}

fn parse_classifier_response(body: &str) -> Option<(String, f32, Vec<(String, f32)>)> {
    // Basic parser for now
    if body.contains("Email") { Some(("Email".into(), 0.9, vec![])) }
    else { Some(("Prompt".into(), 0.8, vec![])) }
}

pub async fn get_worker_usage() -> Option<(u32, u32)> {
    let client   = make_client();
    let response = add_device_header(
        client.get(worker_url("/usage"))
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

pub async fn list_models(_api_key: &str) -> Result<String, String> {
    Ok("gemini-2.0-flash".to_string())
}