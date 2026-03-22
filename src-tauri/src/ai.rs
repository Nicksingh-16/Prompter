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

// ── Correct model URL ──────────────────────────────────────────────────────
const GEMINI_MODEL: &str = "gemini-2.5-flash";
const PROXY_URL: &str = "https://prompter-proxy.onrender.com";
const USE_PROXY: bool = true;

pub fn device_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let computer = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".into());
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".into());
    let mut hasher = DefaultHasher::new();
    format!("{}:{}", computer, user).hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

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
        url // Proxy appends key from its own pool
    } else {
        format!("{}?key={}", url, api_key)
    }
}

// ── Streaming generation ───────────────────────────────────────────────────

pub async fn generate_stream(
    app: AppHandle,
    api_key: &str,
    system_prompt: &str,
    user_text: &str,
) -> Result<(), String> {
    let client = Client::new();
    let url = api_url(api_key, true);

    let full_prompt = format!("{}\n\nInput: {}", system_prompt, user_text);

    let request = GeminiRequest {
        contents: vec![Content {
            parts: vec![Part { text: full_prompt }],
        }],
        generationConfig: GenerationConfig {
            temperature: 0.7,
            maxOutputTokens: 2048,
        },
    };

    let mut request_builder = client.post(&url).json(&request);
    
    if USE_PROXY {
        request_builder = request_builder.header("X-Device-ID", device_id());
    }

    let response = request_builder
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        
        // Try to parse error message from proxy if applicable
        let proxy_msg = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            json["message"].as_str().map(|s| s.to_string())
        } else {
            None
        };

        let error_msg = if let Some(msg) = proxy_msg {
            msg
        } else if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            "Invalid API Key. Please check your settings.".to_string()
        } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            "Daily limit reached. Try again tomorrow or go Pro!".to_string()
        } else {
            format!("AI Service Error ({}).", status)
        };
        app.emit("ai_error", &error_msg).ok();
        return Err(format!("AI error {}: {}", status, body));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| format!("Stream error: {}", e))?;
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        let mut start_idx = 0;
        while let Some(obj_start) = buffer[start_idx..].find('{') {
            let actual_start = start_idx + obj_start;
            let mut depth = 0;
            let mut obj_end = None;
            let mut in_string = false;
            let mut escaped = false;

            for (i, c) in buffer[actual_start..].chars().enumerate() {
                if escaped { escaped = false; continue; }
                match c {
                    '\\'  => escaped = true,
                    '"'   => in_string = !in_string,
                    '{'  if !in_string => depth += 1,
                    '}'  if !in_string => {
                        depth -= 1;
                        if depth == 0 {
                            obj_end = Some(actual_start + i + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if let Some(end) = obj_end {
                let json_obj = &buffer[actual_start..end];
                if let Ok(resp) = serde_json::from_str::<GeminiStreamResponse>(json_obj) {
                    for token in extract_tokens(&resp) {
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
    Ok(())
}

// ── Intent classifier (Layer 3 — Deep Intent) ─────────────────────────────

#[derive(Deserialize)]
struct ClassifierResponse {
    intent: Option<String>,
    confidence: Option<f32>,
    alternatives: Option<Vec<ClassifierAlt>>,
}
#[derive(Deserialize)]
struct ClassifierAlt {
    intent: Option<String>,
    confidence: Option<f32>,
}

pub async fn classify_intent(api_key: &str, text: &str) -> Option<(String, f32, Vec<(String, f32)>)> {
    let client = Client::new();
    let url = api_url(api_key, false);

    let prompt = build_classifier_prompt(text);
    let request = GeminiRequest {
        contents: vec![Content {
            parts: vec![Part { text: prompt }],
        }],
        generationConfig: GenerationConfig {
            temperature: 0.0, // deterministic for classification
            maxOutputTokens: 200,
        },
    };

    let response = client.post(&url).json(&request).send().await.ok()?;
    if !response.status().is_success() { return None; }

    let body = response.text().await.ok()?;
    parse_classifier_response(&body)
}

fn build_classifier_prompt(text: &str) -> String {
    // Truncate to 500 chars to minimize latency for classification call
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
    // Find the first valid JSON object in the response (model may wrap it in markdown)
    let start = body.find('{')?;
    let mut depth = 0;
    let mut end = start;
    let chars: Vec<char> = body.chars().collect();
    let mut in_string = false;
    let mut escaped = false;
    for (i, &c) in chars.iter().enumerate().skip(start) {
        if escaped { escaped = false; continue; }
        match c {
            '\\'  => escaped = true,
            '"'   => in_string = !in_string,
            '{'  if !in_string => depth += 1,
            '}'  if !in_string => {
                depth -= 1;
                if depth == 0 { end = i + 1; break; }
            }
            _ => {}
        }
    }
    let json_slice = body.get(start..end)?;
    let parsed: ClassifierResponse = serde_json::from_str(json_slice).ok()?;
    let intent = parsed.intent?;
    let confidence = parsed.confidence.unwrap_or(0.5);
    let alts: Vec<(String, f32)> = parsed
        .alternatives
        .unwrap_or_default()
        .into_iter()
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