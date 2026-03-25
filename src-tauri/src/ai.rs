use serde::Deserialize;
use reqwest::Client;
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};

// ── Request / Response types ───────────────────────────────────────────────

#[derive(serde::Serialize)]
#[allow(non_snake_case)]
struct GeminiRequest {
    contents: Vec<Content>,
    generationConfig: GenerationConfig,
}
#[derive(serde::Serialize)]
struct Content { parts: Vec<Part> }
#[derive(serde::Serialize)]
struct Part    { text: String }
#[derive(serde::Serialize)]
#[allow(non_snake_case)]
struct GenerationConfig { temperature: f32, maxOutputTokens: i32 }

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

// ── Architecture: Direct Gemini, key rotation in Rust ─────────────────────
//
// Why NOT a proxy on Render:
//   - Render free tier sleeps after 15 min inactivity
//   - Cold start = 10–30 second first-request delay
//   - Completely unacceptable UX for a hotkey tool
//
// Why direct Gemini:
//   - ~400ms TTFB instead of 10–30s cold start
//   - Zero infrastructure to maintain
//   - Keys rotate client-side in compiled Rust — not visible in JS bundle
//   - Gemini free tier is generous enough for launch (1500 req/day per key)
//
// Key pool: XOR-obfuscated at compile time. Rotate round-robin.
// Add real keys below before shipping. Each key = ~1500 free calls/day.
// With 3 keys = 4500 calls/day free — more than enough for launch.

const MODEL_PRIMARY: &str = "gemini-2.0-flash";
const MODEL_SECONDARY: &str = "gemini-1.5-flash";

// XOR key for compile-time obfuscation (not real security, just not plaintext)
const XOR_BYTE: u8 = 0x5A;

// Store keys XOR-obfuscated. Generate with: echo -n "AIzaSy..." | xxd
// Placeholder keys below — replace with real ones before build.
// Each entry is the XOR(0x5A) of each byte of the API key string.
const OBFUSCATED_KEYS: &[&[u8]] = &[
    &[27, 19, 32, 59, 9, 35, 27, 29, 53, 60, 57, 49, 52, 56, 63, 14, 119, 34, 107, 57, 48, 109, 10, 3, 21, 59, 48, 119, 44, 45, 54, 32, 20, 49, 15, 51, 59, 24, 45],
    &[27, 19, 32, 59, 9, 35, 27, 43, 16, 55, 45, 10, 63, 24, 98, 62, 0, 14, 18, 15, 46, 42, 47, 13, 30, 12, 29, 24, 41, 55, 107, 51, 50, 15, 48, 35, 18, 110, 98],
    &[27, 19, 32, 59, 9, 35, 30, 55, 12, 25, 16, 20, 28, 107, 9, 24, 107, 49, 10, 2, 62, 35, 46, 111, 105, 14, 60, 104, 32, 12, 47, 119, 99, 44, 35, 63, 18, 51, 53],
];

static KEY_INDEX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn deobfuscate(obfuscated: &[u8]) -> String {
    obfuscated.iter().map(|&b| (b ^ XOR_BYTE) as char).collect()
}

fn get_next_key() -> Option<String> {
    let valid_keys: Vec<String> = OBFUSCATED_KEYS
        .iter()
        .map(|k| deobfuscate(k))
        .filter(|k| !k.is_empty() && k.starts_with("AIza"))
        .collect();

    if valid_keys.is_empty() { return None; }

    let idx = KEY_INDEX.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % valid_keys.len();
    Some(valid_keys[idx].clone())
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

// ── URL builder ────────────────────────────────────────────────────────────

fn gemini_url(model: &str, stream: bool) -> String {
    let endpoint = if stream { "streamGenerateContent" } else { "generateContent" };
    format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:{}",
        model, endpoint
    )
}

// ── Shared request builder ─────────────────────────────────────────────────

fn build_gemini_request(system_prompt: &str, user_text: &str, max_tokens: i32) -> GeminiRequest {
    let full_prompt = format!("{}\n\nInput: {}", system_prompt, user_text);
    GeminiRequest {
        contents: vec![Content { parts: vec![Part { text: full_prompt }] }],
        generationConfig: GenerationConfig { temperature: 0.7, maxOutputTokens: max_tokens },
    }
}

fn make_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default()
}

// ── Streaming generation — used by overlay (Alt+K) ────────────────────────
// Uses the user-supplied key first, falls back to bundled key pool.

pub async fn generate_stream(
    app: AppHandle,
    user_api_key: &str,
    system_prompt: &str,
    user_text: &str,
) -> Result<String, String> {
    println!("[TRACE] generate_stream called with text len: {}", user_text.len());
    let client = make_client();
    let request = build_gemini_request(system_prompt, user_text, 4096);
    let mut last_error = "No keys available".to_string();
    
    // Model fallback: try 2.0 first, then 1.5
    let models = [MODEL_PRIMARY, MODEL_SECONDARY];
    
    // Retry loop: if 429, try next key in pool
    let max_attempts = if !user_api_key.is_empty() { 1 } else { OBFUSCATED_KEYS.len() };
    
    for model in models {
        for _ in 0..max_attempts {
            // Key resolution
            let api_key = if !user_api_key.is_empty() {
                user_api_key.to_string()
            } else if let Some(k) = get_next_key() {
                k
            } else { break; };

            let url = format!("{}?key={}&alt=sse", gemini_url(model, true), api_key);
            let response = match client.post(&url).json(&request).send().await {
                Ok(r) => r,
                Err(e) => { last_error = format!("Request failed: {}", e); continue; }
            };

            if !response.status().is_success() {
                let status = response.status();
                last_error = response.text().await.unwrap_or_default();
                
                if status == reqwest::StatusCode::TOO_MANY_REQUESTS && user_api_key.is_empty() {
                    // Rate limit hit on bundled key — try next one or next model
                    continue;
                }

                let error_msg = if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
                    "Invalid API key. Check settings.".to_string()
                } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    "Rate limit hit. Please wait a moment or add your own key.".to_string()
                } else {
                    format!("AI error {}.", status)
                };
                app.emit("ai_error", &error_msg).ok();
                return Err(format!("AI error {}: {}", status, last_error));
            }

            // Successful response (stream)
            return handle_stream_response(app, response).await;
        }
    }

    let msg = "All API keys are rate-limited across both Gemini 2.0 and 1.5. Wait a moment.".to_string();
    app.emit("ai_error", &msg).ok();
    Err(last_error)
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
        let mut consumed_lines = 0;

        for line in &lines {
            if line.starts_with("data: ") {
                let json_str = &line[6..];
                if json_str == "[DONE]" { consumed_lines += 1; continue; }
                if let Ok(resp) = serde_json::from_str::<GeminiStreamResponse>(json_str) {
                    for token in extract_tokens(&resp) {
                        full_output.push_str(&token);
                        app.emit("ai_token", &token).ok();
                    }
                }
                consumed_lines += 1;
            } else if line.is_empty() {
                consumed_lines += 1;
            } else { break; }
        }
        if consumed_lines > 0 { buffer = lines[consumed_lines..].join("\n"); }
    }

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
// Non-streaming. No window. Returns full text.

pub async fn generate_silent(
    user_api_key: &str,
    system_prompt: &str,
    user_text: &str,
) -> Result<String, String> {
    let client = make_client();
    let request = build_gemini_request(system_prompt, user_text, 4096);
    let mut last_error = "No keys available".to_string();

    let models = [MODEL_PRIMARY, MODEL_SECONDARY];
    let max_attempts = if !user_api_key.is_empty() { 1 } else { OBFUSCATED_KEYS.len() };

    for model in models {
        for _ in 0..max_attempts {
            let api_key = if !user_api_key.is_empty() {
                user_api_key.to_string()
            } else if let Some(k) = get_next_key() {
                k
            } else { break; };

            let url = format!("{}?key={}", gemini_url(model, false), api_key);
            let response = match client.post(&url).json(&request).send().await {
                Ok(r) => r,
                Err(e) => { last_error = format!("Request failed: {}", e); continue; }
            };

            if !response.status().is_success() {
                let status = response.status();
                last_error = response.text().await.unwrap_or_default();
                if status == reqwest::StatusCode::TOO_MANY_REQUESTS && user_api_key.is_empty() {
                    continue;
                }
                return Err(format!("AI error {}: {}", status, last_error));
            }
            
            let body: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
            let text = body["candidates"][0]["content"]["parts"][0]["text"]
                .as_str().unwrap_or_default().to_string();
            
            if text.is_empty() { return Err("Empty response".into()); }
            return Ok(text);
        }
    }
    Err(format!("All keys/models rate-limited across both models: {}", last_error))
}

// ── Intent classifier ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ClassifierResponse { intent: Option<String>, confidence: Option<f32>, alternatives: Option<Vec<ClassifierAlt>> }
#[derive(Deserialize)]
struct ClassifierAlt { intent: Option<String>, confidence: Option<f32> }

    let models = [MODEL_PRIMARY, MODEL_SECONDARY];
    let max_attempts = if !api_key.is_empty() && api_key != "proxy" { 1 } else { OBFUSCATED_KEYS.len() };

    for model in models {
        for _ in 0..max_attempts {
            let key = if !api_key.is_empty() && api_key != "proxy" {
                api_key.to_string()
            } else {
                get_next_key()?
            };
            println!("[TRACE] classify_intent triggered on model {} with text: \"{}\"", model, text.chars().take(20).collect::<String>());

            let url = format!("{}?key={}", gemini_url(model, false), key);
            let snippet: String = text.chars().take(500).collect();
            let prompt = format!(
                "Classify text into ONE of: Email, Chat, Prompt, Report, Social, General.\n\
                 Return ONLY JSON: {{\"intent\":\"Email\",\"confidence\":0.92,\"alternatives\":[{{\"intent\":\"Chat\",\"confidence\":0.05}}]}}\n\n\
                 Text: \"{}\"",
                snippet
            );
            let request = GeminiRequest {
                contents: vec![Content { parts: vec![Part { text: prompt }] }],
                generationConfig: GenerationConfig { temperature: 0.0, maxOutputTokens: 150 },
            };

            let response = match client.post(&url).json(&request).send().await {
                Ok(r) => r,
                Err(_) => continue,
            };

            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS && (api_key.is_empty() || api_key == "proxy") {
                continue;
            }

            if !response.status().is_success() { return None; }

            let body = response.text().await.ok()?;
            return parse_classifier_response(&body);
        }
    }
    None
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

// ── Model listing ──────────────────────────────────────────────────────────

pub async fn list_models(api_key: &str) -> Result<String, String> {
    let key = if !api_key.is_empty() { api_key.to_string() } else { get_next_key().unwrap_or_default() };
    let client = make_client();
    let url    = format!("https://generativelanguage.googleapis.com/v1beta/models?key={}", key);
    let res    = client.get(url).send().await.map_err(|e| e.to_string())?;
    res.text().await.map_err(|e| e.to_string())
}