use serde::Deserialize;
use reqwest::Client;
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};

// ── Request types ──────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
#[allow(non_snake_case)]
struct GeminiRequest {
    contents: Vec<Content>,
    generationConfig: GenerationConfig,
}

#[derive(serde::Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(serde::Serialize)]
struct Part {
    text: String,
}

#[derive(serde::Serialize)]
#[allow(non_snake_case)]
struct GenerationConfig {
    temperature: f32,
    maxOutputTokens: i32,
}

// ── Response types ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GeminiStreamResponse {
    candidates: Option<Vec<Candidate>>,
}

#[derive(Deserialize)]
struct Candidate {
    content: Option<CandidateContent>,
}

#[derive(Deserialize)]
struct CandidateContent {
    parts: Option<Vec<ResponsePart>>,
}

#[derive(Deserialize)]
struct ResponsePart {
    text: Option<String>,
}

// ── Stream helper ──────────────────────────────────────────────────────────

fn extract_tokens(resp: &GeminiStreamResponse) -> Vec<String> {
    let mut tokens = Vec::new();
    if let Some(candidates) = &resp.candidates {
        for candidate in candidates {
            if let Some(content) = &candidate.content {
                if let Some(parts) = &content.parts {
                    for part in parts {
                        if let Some(text) = &part.text {
                            if !text.is_empty() {
                                tokens.push(text.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    tokens
}

// ── Config ─────────────────────────────────────────────────────────────────

const GEMINI_MODEL: &str = "gemini-2.5-flash";
const PROXY_URL: &str = "https://prompter-proxy.onrender.com";
const USE_PROXY: bool = true;

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

// ── URL helpers ────────────────────────────────────────────────────────────

fn build_url(stream: bool) -> String {
    let endpoint = if stream { "streamGenerateContent" } else { "generateContent" };
    if USE_PROXY {
        format!("{}/v1beta/models/{}:{}", PROXY_URL, GEMINI_MODEL, endpoint)
    } else {
        format!("https://generativelanguage.googleapis.com/v1beta/models/{}:{}", GEMINI_MODEL, endpoint)
    }
}

fn api_url(api_key: &str, stream: bool) -> String {
    let url = build_url(stream);
    if USE_PROXY {
        url
    } else {
        format!("{}?key={}", url, api_key)
    }
}

fn make_request(api_key: &str, system_prompt: &str, user_text: &str, stream: bool, max_tokens: i32) -> (Client, String, GeminiRequest) {
    let client = Client::new();
    let url    = api_url(api_key, stream);
    let full_prompt = format!("{}\n\nInput: {}", system_prompt, user_text);
    let request = GeminiRequest {
        contents: vec![Content {
            parts: vec![Part { text: full_prompt }],
        }],
        generationConfig: GenerationConfig {
            temperature: 0.7,
            maxOutputTokens: max_tokens,
        },
    };
    (client, url, request)
}

// ── Streaming generation — used by overlay (Alt+K) ────────────────────────

pub async fn generate_stream(
    app: AppHandle,
    api_key: &str,
    system_prompt: &str,
    user_text: &str,
) -> Result<String, String> {
    let (client, url, request) = make_request(api_key, system_prompt, user_text, true, 4096);

    let mut req = client.post(&url).json(&request);
    if USE_PROXY {
        req = req.header("X-Device-ID", device_id());
    }

    let response = req.send().await.map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body   = response.text().await.unwrap_or_default();
        let proxy_msg = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|j| j["message"].as_str().map(|s| s.to_string()));
        let error_msg = proxy_msg.unwrap_or_else(|| {
            if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
                "Invalid API Key. Please check your settings.".to_string()
            } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                "Daily limit reached. Try again tomorrow or go Pro!".to_string()
            } else {
                format!("AI Service Error ({}).", status)
            }
        });
        app.emit("ai_error", &error_msg).ok();
        return Err(format!("AI error {}: {}", status, body));
    }

    let mut stream      = response.bytes_stream();
    let mut buffer      = String::new();
    let mut full_output = String::new();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| format!("Stream error: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        let mut start_idx = 0;
        while let Some(obj_start) = buffer[start_idx..].find('{') {
            let actual_start = start_idx + obj_start;
            let mut depth     = 0;
            let mut obj_end   = None;
            let mut in_string = false;
            let mut escaped   = false;

            for (i, c) in buffer[actual_start..].chars().enumerate() {
                if escaped { escaped = false; continue; }
                match c {
                    '\\'             => escaped   = true,
                    '"'              => in_string = !in_string,
                    '{' if !in_string => depth   += 1,
                    '}' if !in_string => {
                        depth -= 1;
                        if depth == 0 { obj_end = Some(actual_start + i + 1); break; }
                    }
                    _ => {}
                }
            }

            if let Some(end) = obj_end {
                if let Ok(resp) = serde_json::from_str::<GeminiStreamResponse>(&buffer[actual_start..end]) {
                    for token in extract_tokens(&resp) {
                        full_output.push_str(&token);
                        app.emit("ai_token", &token).ok();
                    }
                }
                start_idx = end;
            } else {
                break;
            }
        }
        if start_idx > 0 {
            buffer = buffer[start_idx..].to_string();
        }
    }

    app.emit("ai_stream_end", ()).ok();
    Ok(full_output)
}

// ── Silent generation — used by sub-hotkeys (Alt+Shift+K / Alt+Shift+L) ───
// No window, no streaming events. Captures → generates → returns plain text.
// Uses the non-streaming Gemini endpoint for simplicity and speed.

pub async fn generate_silent(
    api_key: &str,
    system_prompt: &str,
    user_text: &str,
) -> Result<String, String> {
    let (client, url, request) = make_request(api_key, system_prompt, user_text, false, 4096);

    let mut req = client.post(&url).json(&request);
    if USE_PROXY {
        req = req.header("X-Device-ID", device_id());
    }

    let response = req.send().await.map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body   = response.text().await.unwrap_or_default();
        return Err(format!("AI error {}: {}", status, body));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let text = body["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if text.is_empty() {
        return Err("Empty response from AI".to_string());
    }

    Ok(text)
}

// ── Intent classifier (Layer 3 — Deep Intent) ─────────────────────────────

#[derive(Deserialize)]
struct ClassifierResponse {
    intent:       Option<String>,
    confidence:   Option<f32>,
    alternatives: Option<Vec<ClassifierAlt>>,
}
#[derive(Deserialize)]
struct ClassifierAlt {
    intent:     Option<String>,
    confidence: Option<f32>,
}

pub async fn classify_intent(api_key: &str, text: &str) -> Option<(String, f32, Vec<(String, f32)>)> {
    let client = Client::new();
    let url    = api_url(api_key, false);

    let prompt  = build_classifier_prompt(text);
    let request = GeminiRequest {
        contents: vec![Content { parts: vec![Part { text: prompt }] }],
        generationConfig: GenerationConfig { temperature: 0.0, maxOutputTokens: 200 },
    };

    let response = client.post(&url).json(&request).send().await.ok()?;
    if !response.status().is_success() { return None; }

    let body = response.text().await.ok()?;
    parse_classifier_response(&body)
}

fn build_classifier_prompt(text: &str) -> String {
    let snippet: String = text.chars().take(500).collect();
    format!(
        "You are a text classification system. Classify the following text into exactly ONE of: \
         Email, Chat, Prompt, Report, Social, General.\n\
         Return ONLY valid JSON in this format:\n\
         {{\"intent\":\"Email\",\"confidence\":0.92,\"alternatives\":[\
         {{\"intent\":\"Chat\",\"confidence\":0.05}}]}}\n\n\
         Text to classify:\n\"{}\"",
        snippet
    )
}

fn parse_classifier_response(body: &str) -> Option<(String, f32, Vec<(String, f32)>)> {
    let start = body.find('{')?;
    let mut depth     = 0;
    let mut end       = start;
    let chars: Vec<char> = body.chars().collect();
    let mut in_string = false;
    let mut escaped   = false;
    for (i, &c) in chars.iter().enumerate().skip(start) {
        if escaped { escaped = false; continue; }
        match c {
            '\\'             => escaped   = true,
            '"'              => in_string = !in_string,
            '{' if !in_string => depth   += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 { end = i + 1; break; }
            }
            _ => {}
        }
    }
    let parsed: ClassifierResponse = serde_json::from_str(body.get(start..end)?).ok()?;
    let intent     = parsed.intent?;
    let confidence = parsed.confidence.unwrap_or(0.5);
    let alts: Vec<(String, f32)> = parsed
        .alternatives.unwrap_or_default().into_iter()
        .filter_map(|a| Some((a.intent?, a.confidence.unwrap_or(0.0))))
        .collect();
    Some((intent, confidence, alts))
}

// ── Model listing ──────────────────────────────────────────────────────────

pub async fn list_models(api_key: &str) -> Result<String, String> {
    let client = Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );
    let res = client.get(url).send().await.map_err(|e| e.to_string())?;
    res.text().await.map_err(|e| e.to_string())
}