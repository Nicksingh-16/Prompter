use serde::Deserialize;
use reqwest::Client;
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};

// ── Architecture: Cloudflare Worker proxy ──────────────────────────────────
//
// Zero secrets in binary. The Worker holds API keys as Cloudflare secrets.
// Client sends only X-Device-ID. Worker rotates keys, enforces daily cap,
// falls back across models, and streams SSE back to the client.
//
// Deploy the worker once from d:\ai_keyboard\worker\ using:
//   npx wrangler deploy
//
// Then update WORKER_URL below with your actual worker URL.

const WORKER_URL: &str = "https://snaptext-worker.YOUR-SUBDOMAIN.workers.dev";

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

#[derive(Deserialize)]
struct ClassifierResponse {
    intent:       Option<String>,
    confidence:   Option<f32>,
    alternatives: Option<Vec<ClassifierAlt>>,
}
#[derive(Deserialize)]
struct ClassifierAlt { intent: Option<String>, confidence: Option<f32> }

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

// ── Device ID — FNV-1a, stable across Rust versions ───────────────────────

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

// ── Worker request helper ──────────────────────────────────────────────────

fn worker_url(path: &str) -> String {
    format!("{}{}", WORKER_URL.trim_end_matches('/'), path)
}

fn add_device_header(req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    req.header("X-Device-ID", device_id())
}

// ── Streaming generation — used by overlay (Alt+K) ────────────────────────

pub async fn generate_stream(
    app: AppHandle,
    _user_api_key: &str,   // kept for API compatibility — keys live in Worker
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

        let msg = if status.as_u16() == 429 {
            // Try to parse Worker's JSON error
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(e) = v["error"].as_str() {
                    e.to_string()
                } else {
                    "Daily limit reached. Resets at midnight.".into()
                }
            } else {
                "Rate limit hit.".into()
            }
        } else {
            format!("Worker error {}: {}", status, text)
        };
        app.emit("ai_error", &msg).ok();
        return Err(msg);
    }

    handle_stream_response(app, response).await
}

// ── Stream handler helper ──────────────────────────────────────────────────

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

    // Fallback: whole-body JSON (non-SSE mode)
    if full_output.is_empty() && !buffer.trim().is_empty() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(buffer.trim()) {
            if let Some(t) = val["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                full_output = t.to_string();
                app.emit("ai_token", &full_output).ok();
            }
        }
    }

    app.emit("ai_stream_end", ()).ok();
    Ok(full_output)
}

// ── Silent generation — used by sub-hotkeys (Alt+Shift+K / Alt+Shift+L) ───

pub async fn generate_silent(
    _user_api_key: &str,   // kept for API compatibility
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

    if !response.status().is_success() {
        let status = response.status();
        let text   = response.text().await.unwrap_or_default();
        return Err(format!("Worker error {}: {}", status, text));
    }

    let data: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let text = data["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if text.is_empty() { return Err("Empty response from Worker.".into()); }
    Ok(text)
}

// ── Intent classifier ─────────────────────────────────────────────────────
// Routes through the Worker so keys are never in the client binary.

pub async fn classify_intent(
    _api_key: &str,   // kept for API compatibility
    text: &str,
) -> Option<(String, f32, Vec<(String, f32)>)> {
    let client  = make_client();
    let snippet: String = text.chars().take(500).collect();
    let prompt  = format!(
        "Classify text into ONE of: Email, Chat, Prompt, Report, Social, General.\n\
         Return ONLY JSON: {{\"intent\":\"Email\",\"confidence\":0.92,\"alternatives\":[{{\"intent\":\"Chat\",\"confidence\":0.05}}]}}\n\n\
         Text: \"{}\"",
        snippet
    );
    let body = WorkerRequest {
        system_prompt: String::new(),
        user_text:     prompt,
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
    let start = body.find('{')?;
    let mut depth = 0; let mut end = start;
    let chars: Vec<char> = body.chars().collect();
    let mut in_string = false; let mut escaped = false;
    for (i, &c) in chars.iter().enumerate().skip(start) {
        if escaped { escaped = false; continue; }
        match c {
            '\\'             => escaped   = true,
            '"'              => in_string = !in_string,
            '{' if !in_string => depth   += 1,
            '}' if !in_string => { depth -= 1; if depth == 0 { end = i + 1; break; } }
            _ => {}
        }
    }
    let parsed: ClassifierResponse = serde_json::from_str(body.get(start..end)?).ok()?;
    let intent     = parsed.intent?;
    let confidence = parsed.confidence.unwrap_or(0.5);
    let alts = parsed.alternatives.unwrap_or_default().into_iter()
        .filter_map(|a| Some((a.intent?, a.confidence.unwrap_or(0.0)))).collect();
    Some((intent, confidence, alts))
}

// ── Usage counter — reads from Worker KV ──────────────────────────────────

pub async fn get_worker_usage() -> Option<(u32, u32)> {
    let client   = make_client();
    let response = add_device_header(
        client.get(format!("{}/usage", WORKER_URL.trim_end_matches('/')))
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

// ── Model listing ──────────────────────────────────────────────────────────

pub async fn list_models(_api_key: &str) -> Result<String, String> {
    Ok("gemini-2.0-flash, gemini-1.5-flash".to_string())
}