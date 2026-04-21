use serde::{Deserialize, Serialize};
use reqwest::Client;
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};

const OLLAMA_URL: &str = "http://localhost:11434";

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    system: String,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: i32,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: Option<String>,
    done: bool,
}

fn make_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_default()
}

pub async fn generate_stream(
    app: Option<AppHandle>,
    model: &str,
    system_prompt: &str,
    user_text: &str,
) -> Result<String, String> {
    let client = make_client();
    let body = OllamaRequest {
        model: model.to_string(),
        prompt: user_text.to_string(),
        system: system_prompt.to_string(),
        stream: true,
        options: OllamaOptions {
            temperature: 0.7,
            num_predict: 4096,
        },
    };

    let url = format!("{}/api/generate", OLLAMA_URL);
    let response = client.post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ollama not found or error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Ollama error: {}", response.status()));
    }

    let mut stream = response.bytes_stream();
    let mut full_output = String::new();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| format!("Stream error: {}", e))?;
        let text = String::from_utf8_lossy(&chunk);
        
        // Ollama returns a sequence of JSON objects, one per line (or chunk)
        for line in text.lines() {
            if let Ok(res) = serde_json::from_str::<OllamaResponse>(line) {
                if let Some(token) = res.response {
                    full_output.push_str(&token);
                    // Emit the token if we have a handle
                    if let Some(ref h) = app {
                        h.emit("ai_token", &token).ok();
                    }
                }
                if res.done { break; }
            }
        }
    }

    if let Some(ref h) = app {
        h.emit("ai_stream_end", ()).ok();
    }
    Ok(full_output)
}

pub async fn list_local_models() -> Vec<String> {
    let client = make_client();
    let url = format!("{}/api/tags", OLLAMA_URL);
    
    let res = client.get(url).send().await;
    if let Ok(response) = res {
        if let Ok(json) = response.json::<serde_json::Value>().await {
            if let Some(models) = json["models"].as_array() {
                return models.iter()
                    .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                    .collect();
            }
        }
    }
    vec![]
}
