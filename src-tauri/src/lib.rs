mod capture;
mod inject;
mod db;
mod keychain;
mod ai;
mod nlp;
mod ollama;
mod embedding;

use zeroize::Zeroizing;

// ── Sensitive data detection ───────────────────────────────────────────────

fn contains_sensitive_data(text: &str) -> Option<&'static str> {
    let t = text;
    // Credit card: 16-digit groups
    if regex_lite_match(t, r"\b\d{4}[\s\-]?\d{4}[\s\-]?\d{4}[\s\-]?\d{4}\b") {
        return Some("credit card number");
    }
    // Aadhaar: 12-digit Indian ID
    if regex_lite_match(t, r"\b\d{4}\s\d{4}\s\d{4}\b") {
        return Some("Aadhaar number");
    }
    // SSN
    if regex_lite_match(t, r"\b\d{3}-\d{2}-\d{4}\b") {
        return Some("SSN");
    }
    // Password label patterns
    let lower = t.to_lowercase();
    for pat in &["password:", "password =", "passwd:", "secret:", "api_key:", "api-key:"] {
        if lower.contains(pat) { return Some("credential"); }
    }
    // Long base64-like strings (likely API keys/tokens)
    for word in t.split_whitespace() {
        let clean: String = word.chars().filter(|c| c.is_alphanumeric() || *c == '+' || *c == '/' || *c == '=').collect();
        if clean.len() > 40 && clean.chars().all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '=') {
            return Some("API key or token");
        }
    }
    None
}

fn regex_lite_match(text: &str, pattern: &str) -> bool {
    match pattern {
        p if p.contains(r"\d{4}[\s\-]?\d{4}[\s\-]?\d{4}[\s\-]?\d{4}") => {
            // Credit card: look for 4 consecutive groups of 4 digits separated by space/dash/nothing
            has_credit_card_pattern(text)
        },
        p if p.contains(r"\d{4}\s\d{4}\s\d{4}") => {
            // Aadhaar: look for standalone "XXXX XXXX XXXX" token within text
            text.split_whitespace()
                .collect::<Vec<_>>()
                .windows(3)
                .any(|w| {
                    w[0].len() == 4 && w[0].chars().all(|c| c.is_ascii_digit()) &&
                    w[1].len() == 4 && w[1].chars().all(|c| c.is_ascii_digit()) &&
                    w[2].len() == 4 && w[2].chars().all(|c| c.is_ascii_digit())
                })
        },
        p if p.contains(r"\d{3}-\d{2}-\d{4}") => {
            // SSN: search word by word for DDD-DD-DDDD pattern
            text.split_whitespace().any(|word| {
                let parts: Vec<&str> = word.split('-').collect();
                parts.len() == 3
                    && parts[0].len() == 3 && parts[0].chars().all(|c| c.is_ascii_digit())
                    && parts[1].len() == 2 && parts[1].chars().all(|c| c.is_ascii_digit())
                    && parts[2].len() == 4 && parts[2].chars().all(|c| c.is_ascii_digit())
            })
        },
        _ => false,
    }
}

fn has_credit_card_pattern(text: &str) -> bool {
    // Match 16 contiguous digits, or 4 groups of 4 separated by spaces or dashes
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        if chars[i].is_ascii_digit() {
            // Try no-separator: 16 consecutive digits
            if i + 16 <= n && chars[i..i+16].iter().all(|c| c.is_ascii_digit())
                && (i == 0 || !chars[i-1].is_ascii_digit())
                && (i + 16 == n || !chars[i+16].is_ascii_digit())
            {
                return true;
            }
            // Try separator pattern: DDDD[sep]DDDD[sep]DDDD[sep]DDDD
            if i + 4 <= n && chars[i..i+4].iter().all(|c| c.is_ascii_digit()) {
                let sep_pos = i + 4;
                if sep_pos < n && (chars[sep_pos] == ' ' || chars[sep_pos] == '-') {
                    let g2 = sep_pos + 1;
                    if g2 + 4 <= n && chars[g2..g2+4].iter().all(|c| c.is_ascii_digit()) {
                        let sep2 = g2 + 4;
                        if sep2 < n && chars[sep2] == chars[sep_pos] {
                            let g3 = sep2 + 1;
                            if g3 + 4 <= n && chars[g3..g3+4].iter().all(|c| c.is_ascii_digit()) {
                                let sep3 = g3 + 4;
                                if sep3 < n && chars[sep3] == chars[sep_pos] {
                                    let g4 = sep3 + 1;
                                    if g4 + 4 <= n && chars[g4..g4+4].iter().all(|c| c.is_ascii_digit())
                                        && (g4 + 4 == n || !chars[g4+4].is_ascii_digit())
                                    {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        i += 1;
    }
    false
}

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Emitter, State, menu::{Menu, MenuItem}, tray::TrayIconBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState, Shortcut, Modifiers, Code};

// ── App state ──────────────────────────────────────────────────────────────

pub struct AppState {
    pub db:                  Arc<Mutex<rusqlite::Connection>>,
    pub worker_secret:       String,
    pub device_id:           String,
    pub incognito:           std::sync::atomic::AtomicBool,
    /// suggested_mode → preferred_mode overrides learned from user corrections.
    /// Keys and values are mode strings ("Email", "Reply", "Do", etc.).
    pub correction_overrides: std::sync::RwLock<std::collections::HashMap<String, String>>,
}

/// Safe UTF-8 truncation by character count, not bytes.
/// Prevents panics when multi-byte chars (CJK, Devanagari, etc.) land on the boundary.
fn safe_truncate(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

/// SHA-256 hex of (preview + "|" + mode) — used for history deduplication.
fn content_hash(preview: &str, mode: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut h = Sha256::new();
    h.update(preview.as_bytes());
    h.update(b"|");
    h.update(mode.as_bytes());
    format!("{:x}", h.finalize())
}

// ── Tauri commands ─────────────────────────────────────────────────────────

#[tauri::command]
fn get_captured_text() -> String {
    capture::capture_text().unwrap_or_default()
}

#[tauri::command]
async fn generate_ai_response(
    state: State<'_, AppState>,
    app: AppHandle,
    mode: String,
    text: String,
    custom_prompt: Option<String>,
    sub_mode: Option<String>,
) -> Result<(), String> {
    let ctx = nlp::analyze(&text);

    // Parse conversation thread for Reply mode (gives AI full context, not just last message).
    let parsed_thread = if mode == "Reply" { nlp::thread::parse_thread(&text) } else { None };
    let thread_prompt_block: Option<String> = parsed_thread.as_ref()
        .map(|t| nlp::thread::format_for_prompt(t));

    // Phase 1: Read from DB (synchronous, minimal lock time)
    let skip_personalization = mode == "Prompt";
    let sanitized_query      = nlp::prompt::sanitize(&text);
    let (api_key, profile, memory, history_for_rag, contact_language, contact_examples) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let key  = Zeroizing::new(keychain::get_api_key(&conn).unwrap_or_default());
        if skip_personalization {
            (key, vec![], vec![], vec![], None, vec![])
        } else {
            let prof  = db::get_voice_profile(&conn).unwrap_or_default();
            let names: Vec<String> = ctx.detected_entities.iter().map(|(n, _)| n.clone()).collect();
            let mem   = db::get_entities_context(&conn, &names).unwrap_or_default();

            // Load history WITH stored embeddings for semantic RAG
            let mut all_history = db::get_history_with_embeddings(&conn, &mode, 50)
                .unwrap_or_default();
            if mode == "Email" || mode == "Reply" {
                let other = if mode == "Email" { "Reply" } else { "Email" };
                all_history.extend(
                    db::get_history_with_embeddings(&conn, other, 20).unwrap_or_default()
                );
            }

            let contact_lang = if mode == "Reply" {
                names.first()
                    .and_then(|name| db::get_contact_language(&conn, name).ok().flatten())
            } else {
                None
            };

            // Fetch accepted reply examples for this specific contact (highest-quality RAG).
            let contact_ex = if mode == "Reply" {
                let contact = parsed_thread.as_ref()
                    .and_then(|t| t.contact_name.as_deref())
                    .or_else(|| names.first().map(|s| s.as_str()));
                contact.and_then(|c| db::get_accepted_reply_examples(&conn, c, 5).ok())
                    .unwrap_or_default()
            } else {
                vec![]
            };

            (key, prof, mem, all_history, contact_lang, contact_ex)
        }
    };

    // Phase 2: Compute RAG outside the DB lock (may need async embedding fetch)
    let use_semantic = matches!(mode.as_str(), "Reply" | "Email" | "Do");
    let rag_examples: Vec<(String, String)> = if !skip_personalization && !history_for_rag.is_empty() {
        if use_semantic {
            // Try semantic RAG: fetch query embedding, score by cosine similarity
            let query_emb = ai::fetch_embedding(&sanitized_query, &state.worker_secret, &state.device_id).await;
            let has_stored = history_for_rag.iter().any(|(_, _, _, e)| e.is_some());

            if let (Some(qemb), true) = (query_emb, has_stored) {
                let results = embedding::semantic_retrieve(&qemb, history_for_rag, 3);
                if !results.is_empty() {
                    results.into_iter().map(|(_, i, o)| (i, o)).collect()
                } else {
                    vec![] // no sufficiently similar past examples
                }
            } else {
                // Fall back to BM25 when no embeddings stored yet
                let corpus: Vec<(String, String)> = history_for_rag
                    .into_iter()
                    .filter(|(_, inp, out, _)| !inp.is_empty() && !out.is_empty())
                    .map(|(_, inp, out, _)| (inp, out))
                    .collect();
                embedding::retrieve(&sanitized_query, corpus, 3)
                    .into_iter().map(|(_, i, o)| (i, o)).collect()
            }
        } else {
            // BM25 for modes that don't benefit from semantic search
            let corpus: Vec<(String, String)> = history_for_rag
                .into_iter()
                .filter(|(_, inp, out, _)| !inp.is_empty() && !out.is_empty())
                .map(|(_, inp, out, _)| (inp, out))
                .collect();
            embedding::retrieve(&sanitized_query, corpus, 3)
                .into_iter().map(|(_, i, o)| (i, o)).collect()
        }
    } else {
        vec![]
    };

    let app_exe     = capture::get_active_app();
    let app_category = app_exe.as_deref().map(capture::classify_app);
    let system_prompt = nlp::prompt::build_prompt(
        &app, &ctx, &mode,
        sub_mode.as_deref().or(custom_prompt.as_deref()),
        &profile, &memory,
        &rag_examples,
        contact_language.as_deref(),
        app_category,
        thread_prompt_block.as_deref(),
        &contact_examples,
    );

    let mode_enum = get_current_ai_mode(&state);
    let config = ai::mode_config(&mode);
    let res = ai::generate_stream(app.clone(), &api_key, &system_prompt, &text, mode_enum, &config, &state.worker_secret, &state.device_id).await;

    if let Ok(ref output) = res {
        let conn = state.db.lock().map_err(|e| format!("DB lock error: {}", e))?;
        let preview  = safe_truncate(&text, 1000);
        let out_str  = safe_truncate(output, 2000);
        let hash     = content_hash(&preview, &mode);
        let ai_mode_str = format!("{:?}", get_current_ai_mode(&state));

        use std::sync::atomic::Ordering;
        let incognito = state.incognito.load(Ordering::Relaxed);
        let sensitive = contains_sensitive_data(&text);
        let should_store = !incognito && sensitive.is_none();

        if should_store {
            let _ = db::save_history(
                &conn, &preview, &mode, &out_str,
                ctx.tone, ctx.formality, None, Some(&hash),
            );
            let _ = db::observe_session_v2(
                &conn, &text, ctx.tone, ctx.formality,
                ctx.contraction_rate, ctx.avg_sentence_len, ctx.emoji_count,
            );
            // Extract per-contact opener/closer for relationship graph
            let words: Vec<&str> = text.split_whitespace().collect();
            let opener = words.first().map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase());
            let closer  = words.last().map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase());
            for (name, etype) in &ctx.detected_entities {
                let _ = db::record_entity_mention(&conn, etype, name, ctx.tone, ctx.formality);
                if let Some(ref op) = opener {
                    if !op.is_empty() { let _ = db::record_contact_pattern(&conn, name, "opener", op); }
                }
                if let Some(ref cl) = closer {
                    if !cl.is_empty() { let _ = db::record_contact_pattern(&conn, name, "closer", cl); }
                }
            }
            if !ctx.language.candidate_languages.is_empty() {
                for (name, _) in &ctx.detected_entities {
                    let _ = db::record_contact_language(&conn, name, &ctx.language.candidate_languages);
                }
            }

            // Spawn background task to fetch + store the embedding for this history entry.
            // Best-effort: failures are silently ignored so they never block the main flow.
            if use_semantic {
                let row_id = db::get_last_history_id(&conn, &preview, &mode);
                if let Some(id) = row_id {
                    let db_arc      = state.db.clone();
                    let secret      = state.worker_secret.clone();
                    let device      = state.device_id.clone();
                    let embed_text  = preview.clone();
                    tauri::async_runtime::spawn(async move {
                        match ai::fetch_embedding(&embed_text, &secret, &device).await {
                            Some(emb) => {
                                let bytes = embedding::vec_to_bytes(&emb);
                                match db_arc.lock() {
                                    Ok(conn) => {
                                        if db::update_embedding(&conn, id, &bytes).is_err() {
                                            log::warn!("embed: DB write failed for history id={}", id);
                                        } else {
                                            log::debug!("embed: stored {} dims for id={}", emb.len(), id);
                                        }
                                    }
                                    Err(e) => log::warn!("embed: DB lock error: {}", e),
                                }
                            }
                            None => log::warn!("embed: fetch failed for history id={}", id),
                        }
                    });
                }
            }
        } else {
            let reason = if incognito { "private mode" } else { sensitive.unwrap_or("sensitive data") };
            app.emit("sensitive_data_detected", reason).ok();
        }

        let _ = db::save_audit_entry(&conn, &mode, &ai_mode_str, text.len(), should_store);

        // ── Anonymous telemetry: fire-and-forget, no text content ──
        if !state.incognito.load(std::sync::atomic::Ordering::Relaxed) {
            let secret    = state.worker_secret.clone();
            let device    = state.device_id.clone();
            let tele_mode = mode.clone();
            let tele_ctx  = app_category.unwrap_or("unknown").to_string();
            tauri::async_runtime::spawn(async move {
                let client = reqwest::Client::new();
                let _ = client
                    .post("https://snaptext-worker.snaptext-ai.workers.dev/telemetry")
                    .header("X-App-Secret", &secret)
                    .header("X-Device-ID", &device)
                    .header("Content-Type", "application/json")
                    .json(&serde_json::json!({
                        "event": "transform",
                        "mode": tele_mode,
                        "app_context": tele_ctx,
                    }))
                    .timeout(std::time::Duration::from_secs(5))
                    .send()
                    .await;
            });
        }
    }

    res.map(|_| ())
}

#[tauri::command]
fn get_voice_profile(state: State<'_, AppState>) -> Result<Vec<(String, String, String)>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db::get_voice_profile(&db).map_err(|e| e.to_string())
}

#[tauri::command]
fn inject_result(app: tauri::AppHandle, text: String) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    inject::inject_text(text);
}


fn get_current_ai_mode(state: &State<'_, AppState>) -> ai::AiMode {
    if let Ok(conn) = state.db.lock() {
        if let Ok(val) = db::get_config(&conn, "ai_mode") {
            return match val.as_str() {
                "Local"  => ai::AiMode::Local,
                "Byok"   => ai::AiMode::Byok,
                _        => ai::AiMode::Worker,
            };
        }
    }
    ai::AiMode::Worker
}

#[tauri::command]
fn get_ai_mode(state: State<'_, AppState>) -> String {
    format!("{:?}", get_current_ai_mode(&state))
}

#[tauri::command]
fn set_ai_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::set_config(&conn, "ai_mode", &mode).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_hardware_stats() -> serde_json::Value {
    // Basic stats for routing decisions
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();
    serde_json::json!({
        "ram_gb": sys.total_memory() / 1024 / 1024 / 1024,
        "cpu_count": sys.cpus().len(),
    })
}

#[tauri::command]
fn store_api_key(state: State<'_, AppState>, key: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    keychain::store_api_key(&conn, &key)
}

#[tauri::command]
fn delete_api_key(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    keychain::delete_api_key(&conn)
}

#[tauri::command]
fn has_api_key(state: State<'_, AppState>) -> bool {
    let mode = get_current_ai_mode(&state);
    if mode == ai::AiMode::Worker || mode == ai::AiMode::Local { return true; }
    
    // BYOK mode requires a real key
    if let Ok(conn) = state.db.lock() {
        if let Ok(key) = keychain::get_api_key(&conn) {
            return !key.trim().is_empty();
        }
    }
    false
}

#[tauri::command]
async fn list_available_models(_state: State<'_, AppState>) -> Result<String, String> {
    ai::list_models().await
}

#[tauri::command]
fn hide_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[tauri::command]
fn analyze_text(text: String) -> String {
    let ctx = nlp::analyze(&text);
    serde_json::to_string(&ctx).unwrap_or_else(|_| "{}".into())
}

#[tauri::command]
fn record_intent_correction(
    state: State<'_, AppState>,
    suggested_intent: String,
    chosen_intent: String,
    confidence: f32,
    text_length: usize,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| format!("DB lock error: {}", e))?;
    db::save_correction(&conn, &suggested_intent, &chosen_intent, confidence, text_length)
        .map_err(|e| e.to_string())?;
    // Refresh in-memory override map so correction takes effect this session
    if let Ok(pairs) = db::get_top_corrections(&conn) {
        if let Ok(mut overrides) = state.correction_overrides.write() {
            overrides.clear();
            for (suggested, chosen) in pairs {
                overrides.insert(suggested, chosen);
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn get_communication_score(state: State<'_, AppState>) -> Result<db::CommReport, String> {
    let db = state.db.lock().map_err(|e| format!("DB lock error: {}", e))?;
    db::get_communication_report(&db).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_history(state: State<'_, AppState>, limit: i64) -> Result<Vec<db::HistoryEntry>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db::get_recent_history(&db, limit).map_err(|e| e.to_string())
}

#[tauri::command]
fn record_reply_feedback(
    state: State<'_, AppState>,
    input: String,
    ai_output: String,
    accepted: bool,
    contact_hint: Option<String>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let preview  = safe_truncate(&input, 500);
    let out_snip = safe_truncate(&ai_output, 1000);
    db::save_reply_feedback(&db, &preview, &out_snip, accepted, contact_hint.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_config_value(state: State<'_, AppState>, key: String) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db::get_config(&db, &key).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_config_value(state: State<'_, AppState>, key: String, value: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db::set_config(&db, &key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_device_id(state: State<'_, AppState>) -> String {
    state.device_id.clone()
}

#[tauri::command]
fn get_audit_log(state: State<'_, AppState>, limit: i64) -> Result<Vec<db::AuditEntry>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db::get_audit_log(&db, limit).map_err(|e| e.to_string())
}

#[tauri::command]
fn toggle_incognito(state: State<'_, AppState>) -> bool {
    use std::sync::atomic::Ordering;
    let current = state.incognito.load(Ordering::Relaxed);
    state.incognito.store(!current, Ordering::Relaxed);
    !current
}

#[tauri::command]
fn get_incognito(state: State<'_, AppState>) -> bool {
    use std::sync::atomic::Ordering;
    state.incognito.load(Ordering::Relaxed)
}

#[tauri::command]
async fn get_worker_usage(state: State<'_, AppState>) -> Result<Option<serde_json::Value>, String> {
    Ok(ai::get_worker_usage(&state.worker_secret, &state.device_id).await.map(|(used, cap)| {
        serde_json::json!({ "used": used, "cap": cap })
    }))
}

// ── Loader toast helpers ───────────────────────────────────────────────────
// A tiny always-on-top window that shows while silent hotkeys are working.

fn loader_show(handle: &AppHandle, mode: &str) {
    if let Some(w) = handle.get_webview_window("loader") {
        // Position bottom-center of screen
        if let Ok(Some(monitor)) = w.primary_monitor() {
            let size  = monitor.size();
            let scale = monitor.scale_factor();
            let x = (size.width  as f64 / scale - 220.0) / 2.0;
            let y =  size.height as f64 / scale - 48.0 - 40.0;
            let _ = w.set_position(tauri::LogicalPosition::new(x, y));
        }
        let label = match mode {
            "Prompt"  => "Structuring prompt…",
            "Correct" => "Rewriting to English…",
            _         => "Working…",
        };
        handle.emit("loader_state", serde_json::json!({
            "state": "working",
            "label": label,
        })).ok();
        let _ = w.show();
        let _ = w.set_always_on_top(true);
    }
}

fn loader_hide(handle: &AppHandle, success: bool) {
    loader_hide_with_hint(handle, success, false);
}

fn loader_hide_with_hint(handle: &AppHandle, success: bool, show_undo: bool) {
    if let Some(w) = handle.get_webview_window("loader") {
        let label = if !success {
            "Failed"
        } else if show_undo {
            "Done · Ctrl+Z to undo"
        } else {
            "Done"
        };
        handle.emit("loader_state", serde_json::json!({
            "state": if success { "done" } else { "error" },
            "label": label,
        })).ok();
        // Longer pause when showing undo hint so user has time to read it
        let pause = if show_undo { 2500 } else { 700 };
        std::thread::sleep(std::time::Duration::from_millis(pause));
        let _ = w.hide();
    }
}

// ── Clipboard-watch pill ──────────────────────────────────────────────────
// Monitors clipboard via sequence number. Shows pill only on double-copy
// (Ctrl+C twice within 1.5s). Positions near the text caret, not mouse.

fn start_clipboard_monitor(handle: AppHandle) {
    std::thread::spawn(move || {
        #[cfg(target_os = "windows")]
        {
            use std::mem::{MaybeUninit, zeroed};

            #[repr(C)]
            struct POINT { x: i32, y: i32 }

            #[repr(C)]
            struct RECT { left: i32, top: i32, right: i32, bottom: i32 }

            #[repr(C)]
            #[allow(non_snake_case)]
            struct GUITHREADINFO {
                cbSize: u32,
                flags: u32,
                hwndActive: isize,
                hwndFocus: isize,
                hwndCapture: isize,
                hwndMenuOwner: isize,
                hwndMoveSize: isize,
                hwndCaret: isize,
                rcCaret: RECT,
            }

            extern "system" {
                fn GetClipboardSequenceNumber() -> u32;
                fn GetCursorPos(lp: *mut POINT) -> i32;
                fn GetGUIThreadInfo(idThread: u32, pgui: *mut GUITHREADINFO) -> i32;
                fn ClientToScreen(hWnd: isize, lpPoint: *mut POINT) -> i32;
            }

            let mut last_seq = unsafe { GetClipboardSequenceNumber() };
            let mut first_copy_seq: u32 = 0;
            let mut first_copy_time = std::time::Instant::now();
            let mut first_copy_text = String::new();

            // Track Ctrl key rapid presses as a fallback for same-content copies
            let mut last_ctrl_c_time = std::time::Instant::now();
            let mut ctrl_c_count: u32 = 0;
            // Rising-edge tracking: only count a new press, not a held key
            let mut ctrl_c_was_held = false;

            extern "system" {
                fn GetAsyncKeyState(vKey: i32) -> i16;
            }
            const VK_CONTROL: i32 = 0x11;
            const VK_C: i32 = 0x43;

            loop {
                std::thread::sleep(std::time::Duration::from_millis(150));

                let seq = unsafe { GetClipboardSequenceNumber() };

                // Rising-edge detection: only count when both keys are newly pressed
                let ctrl_pressed = unsafe { GetAsyncKeyState(VK_CONTROL) } & 0x8000u16 as i16 != 0;
                let c_pressed    = unsafe { GetAsyncKeyState(VK_C)       } & 0x8000u16 as i16 != 0;
                let both_pressed = ctrl_pressed && c_pressed;

                if both_pressed && !ctrl_c_was_held {
                    // Rising edge — new Ctrl+C press
                    if last_ctrl_c_time.elapsed() < std::time::Duration::from_millis(1500) {
                        ctrl_c_count += 1;
                    } else {
                        ctrl_c_count = 1;
                    }
                    last_ctrl_c_time = std::time::Instant::now();
                }
                ctrl_c_was_held = both_pressed;

                if ctrl_c_count >= 2 && both_pressed {
                        ctrl_c_count = 0;
                        // Read clipboard and trigger pill
                        let current = {
                            let mut cb = match arboard::Clipboard::new() {
                                Ok(c) => c,
                                Err(e) => { eprintln!("[clipboard] open failed (ctrl+c): {}", e); continue; }
                            };
                            cb.get_text().unwrap_or_default()
                        };
                        if current.trim().len() >= 3 {
                            let trimmed = current.trim();
                            let word_count = trimmed.split_whitespace().count();
                            if word_count >= 4
                                && !trimmed.starts_with("http")
                                && !trimmed.starts_with('/')
                                && !trimmed.starts_with('\\')
                                && !trimmed.starts_with("C:")
                                && !trimmed.starts_with("D:")
                            {
                                let alpha_ratio = trimmed.chars().filter(|c| c.is_alphabetic() || c.is_whitespace()).count() as f32
                                    / trimmed.len().max(1) as f32;
                                if alpha_ratio >= 0.4 {
                                    last_seq = unsafe { GetClipboardSequenceNumber() };
                                    first_copy_seq = 0;
                                    // Jump to pill show logic below
                                    // (duplicated to keep the flow clear)
                                    if let Some(w) = handle.get_webview_window("pill") {
                                        let mut pos: Option<(i32, i32)> = None;
                                        unsafe {
                                            let mut gui: GUITHREADINFO = zeroed();
                                            gui.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
                                            if GetGUIThreadInfo(0, &mut gui) != 0 && gui.hwndCaret != 0 {
                                                let mut pt = POINT { x: gui.rcCaret.left, y: gui.rcCaret.bottom };
                                                if ClientToScreen(gui.hwndCaret, &mut pt) != 0 {
                                                    pos = Some((pt.x, pt.y));
                                                }
                                            }
                                        }
                                        if pos.is_none() {
                                            unsafe {
                                                let mut pt = MaybeUninit::<POINT>::uninit();
                                                if GetCursorPos(pt.as_mut_ptr()) != 0 {
                                                    let p = pt.assume_init();
                                                    pos = Some((p.x, p.y));
                                                }
                                            }
                                        }
                                        if let Some((x, y)) = pos {
                                            let scale = w.primary_monitor().ok().flatten()
                                                .map(|m| m.scale_factor()).unwrap_or(1.0);
                                            let pill_w = 280.0;
                                            let lx = (x as f64 / scale) - (pill_w / 2.0);
                                            let ly = (y as f64 / scale) + 6.0;
                                            let _ = w.set_position(tauri::LogicalPosition::new(lx.max(4.0), ly));
                                        }
                                        let _ = w.show();
                                        let _ = w.set_always_on_top(true);
                                        std::thread::sleep(std::time::Duration::from_millis(40));
                                        let _ = w.emit("pill_show", ());
                                    }
                                    continue;
                                }
                            }
                        }
                    }

                if seq == last_seq { continue; }

                // Clipboard changed — read the text
                let current = {
                    let mut cb = match arboard::Clipboard::new() {
                        Ok(c) => c,
                        Err(e) => { eprintln!("[clipboard] open failed (seq): {}", e); last_seq = seq; continue; }
                    };
                    cb.get_text().unwrap_or_default()
                };

                if current.trim().len() < 3 { last_seq = seq; continue; }

                // Double-copy detection using sequence numbers
                if first_copy_seq == 0
                    || first_copy_time.elapsed() > std::time::Duration::from_millis(1500)
                    || first_copy_text != current
                {
                    // First copy — record and wait
                    first_copy_seq = seq;
                    first_copy_time = std::time::Instant::now();
                    first_copy_text = current;
                    last_seq = seq;
                    continue;
                }

                // Second copy within 1.5s — pill triggered!
                first_copy_seq = 0;
                last_seq = seq;

                // Smart filter: only show pill for text worth transforming
                let trimmed = current.trim();
                let word_count = trimmed.split_whitespace().count();
                if word_count < 4 { continue; }
                if trimmed.starts_with("http") || trimmed.starts_with('/') || trimmed.starts_with('\\') { continue; }
                if trimmed.starts_with("C:") || trimmed.starts_with("D:") { continue; }
                let alpha_ratio = trimmed.chars().filter(|c| c.is_alphabetic() || c.is_whitespace()).count() as f32
                    / trimmed.len().max(1) as f32;
                if alpha_ratio < 0.4 { continue; }

                // Position pill near the text caret (not mouse cursor)
                if let Some(w) = handle.get_webview_window("pill") {
                    let mut pos: Option<(i32, i32)> = None;

                    // Try caret position via GetGUIThreadInfo
                    unsafe {
                        let mut gui: GUITHREADINFO = zeroed();
                        gui.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
                        if GetGUIThreadInfo(0, &mut gui) != 0 && gui.hwndCaret != 0 {
                            let mut pt = POINT {
                                x: gui.rcCaret.left,
                                y: gui.rcCaret.bottom,
                            };
                            if ClientToScreen(gui.hwndCaret, &mut pt) != 0 {
                                pos = Some((pt.x, pt.y));
                            }
                        }
                    }

                    // Fallback to mouse cursor if caret not available
                    if pos.is_none() {
                        unsafe {
                            let mut pt = MaybeUninit::<POINT>::uninit();
                            if GetCursorPos(pt.as_mut_ptr()) != 0 {
                                let p = pt.assume_init();
                                pos = Some((p.x, p.y));
                            }
                        }
                    }

                    if let Some((x, y)) = pos {
                        let scale = w.primary_monitor()
                            .ok().flatten()
                            .map(|m| m.scale_factor()).unwrap_or(1.0);
                        let pill_w = 280.0; // approximate pill width
                        let lx = (x as f64 / scale) - (pill_w / 2.0); // center on caret
                        let ly = (y as f64 / scale) + 6.0; // snug below caret
                        let _ = w.set_position(tauri::LogicalPosition::new(lx.max(4.0), ly));
                    }

                    let _ = w.show();
                    let _ = w.set_always_on_top(true);
                    let _ = w.emit("pill_show", ());
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // macOS/Linux: double-copy detection (same text within 1.5s) → show pill
            let mut first_copy_text = String::new();
            let mut first_copy_time = std::time::Instant::now() - std::time::Duration::from_secs(10);
            let mut last_text = String::new();
            loop {
                std::thread::sleep(std::time::Duration::from_millis(300));
                let current = {
                    let Ok(mut cb) = arboard::Clipboard::new() else { continue };
                    cb.get_text().unwrap_or_default()
                };
                if current.trim().len() < 3 || current == last_text { continue; }
                last_text = current.clone();

                let trimmed = current.trim();
                let word_count = trimmed.split_whitespace().count();
                if word_count < 4 { continue; }
                if trimmed.starts_with("http") { continue; }
                let alpha_ratio = trimmed.chars().filter(|c| c.is_alphabetic() || c.is_whitespace()).count() as f32
                    / trimmed.len().max(1) as f32;
                if alpha_ratio < 0.4 { continue; }

                let now = std::time::Instant::now();
                if current == first_copy_text && now.duration_since(first_copy_time) < std::time::Duration::from_millis(1500) {
                    // Double-copy — show pill near bottom-center of primary monitor
                    if let Some(w) = handle.get_webview_window("pill") {
                        if let Ok(Some(monitor)) = w.primary_monitor() {
                            let size = monitor.size();
                            let scale = monitor.scale_factor();
                            let pill_w = 280.0_f64;
                            let lx = (size.width as f64 / scale / 2.0) - (pill_w / 2.0);
                            let ly = size.height as f64 / scale - 120.0;
                            let _ = w.set_position(tauri::LogicalPosition::new(lx.max(4.0), ly));
                        }
                        let _ = w.show();
                        let _ = w.set_always_on_top(true);
                        let _ = w.emit("pill_show", ());
                    }
                    first_copy_text.clear();
                } else {
                    first_copy_text = current;
                    first_copy_time = now;
                }
            }
        }
    });
}

#[tauri::command]
fn pill_clicked(app: AppHandle, mode: String) {
    // Emit pill_hide BEFORE hiding so JS resets isWorking / button state
    if let Some(w) = app.get_webview_window("pill") {
        let _ = w.emit("pill_hide", ());
        let _ = w.hide();
    }

    // Text is already in clipboard from the double-copy gesture.
    // Don't re-capture (focus moved to pill window, Ctrl+C would fail).
    let text = {
        let Ok(mut cb) = arboard::Clipboard::new() else { return };
        cb.get_text().unwrap_or_default()
    };
    if text.trim().is_empty() { return; }

    let h = app.clone();
    let mode = mode.clone();
    tauri::async_runtime::spawn(async move {
        loader_show(&h, &mode);

        let ctx   = nlp::analyze(&text);
        let state = h.state::<AppState>();

        let (api_key, profile, memory) = {
            if let Ok(db) = state.db.lock() {
                let key  = Zeroizing::new(keychain::get_api_key(&db).unwrap_or_default());
                let prof = db::get_voice_profile(&db).unwrap_or_default();
                let names: Vec<String> = ctx.detected_entities.iter()
                    .map(|(n, _)| n.clone()).collect();
                let mem = db::get_entities_context(&db, &names).unwrap_or_default();
                (key, prof, mem)
            } else {
                (Zeroizing::new(String::new()), vec![], vec![])
            }
        };

        let app_exe      = capture::get_active_app();
        let app_category = app_exe.as_deref().map(capture::classify_app);
        let system_prompt = nlp::prompt::build_prompt(&h, &ctx, &mode, None, &profile, &memory, &[], None, app_category, None, &[]);
        let state     = h.state::<AppState>();
        let mode_enum = get_current_ai_mode(&state);

        match ai::generate_silent(&api_key, &system_prompt, &text, mode_enum, &state.worker_secret, &state.device_id).await {
            Ok(result) => {
                if result.trim().is_empty() {
                    loader_hide(&h, false);
                    eprintln!("Pill transform returned empty (mode={})", mode);
                    return;
                }
                loader_hide_with_hint(&h, true, true);
                inject::inject_text(result.clone());
                use std::sync::atomic::Ordering;
                let incognito = state.incognito.load(Ordering::Relaxed);
                if !incognito && contains_sensitive_data(&text).is_none() {
                    match state.db.lock() {
                        Ok(db) => {
                            let preview = safe_truncate(&text, 1000);
                            let out_str = safe_truncate(&result, 2000);
                            let hash    = content_hash(&preview, &mode);
                            if let Err(e) = db::save_history(&db, &preview, &mode,
                                &out_str, ctx.tone, ctx.formality, None, Some(&hash)) {
                                eprintln!("[db] save_history failed (pill): {}", e);
                            }
                            let _ = db::observe_session_v2(
                                &db, &text, ctx.tone, ctx.formality,
                                ctx.contraction_rate, ctx.avg_sentence_len, ctx.emoji_count,
                            );
                            let words: Vec<&str> = text.split_whitespace().collect();
                            let opener = words.first().map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase());
                            let closer  = words.last().map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase());
                            for (name, etype) in &ctx.detected_entities {
                                let _ = db::record_entity_mention(&db, etype, name, ctx.tone, ctx.formality);
                                if let Some(ref op) = opener {
                                    if !op.is_empty() { let _ = db::record_contact_pattern(&db, name, "opener", op); }
                                }
                                if let Some(ref cl) = closer {
                                    if !cl.is_empty() { let _ = db::record_contact_pattern(&db, name, "closer", cl); }
                                }
                            }
                            if !ctx.language.candidate_languages.is_empty() {
                                for (name, _) in &ctx.detected_entities {
                                    let _ = db::record_contact_language(&db, name, &ctx.language.candidate_languages);
                                }
                            }
                        }
                        Err(e) => eprintln!("[db] mutex poisoned (pill): {}", e),
                    }
                }
            }
            Err(e) => {
                loader_hide(&h, false);
                eprintln!("Pill transform failed (mode={}): {}", mode, e);
            }
        }
    });
}

#[tauri::command]
fn hide_pill(app: AppHandle) {
    if let Some(w) = app.get_webview_window("pill") {
        let _ = w.emit("pill_hide", ());
        let _ = w.hide();
    }
}

// ── Show overlay (Alt+K) ───────────────────────────────────────────────────

fn show_overlay(handle: &AppHandle) {
    let app_exe      = capture::get_active_app();
    let app_category = app_exe.as_deref().map(capture::classify_app).unwrap_or("other");
    let captured     = capture::capture_text().unwrap_or_default();
    let mut ctx      = nlp::analyze(&captured);

    // Apply learned correction override: if user has consistently chosen a
    // different mode than what NLP suggested, honour their preference.
    {
        let state = handle.state::<AppState>();
        let preferred_opt = state.correction_overrides.read().ok()
            .and_then(|overrides| overrides.get(&ctx.suggested_mode).cloned());
        if let Some(preferred) = preferred_opt {
            ctx.suggested_mode = preferred;
        }
    }

    handle.emit("text_captured", serde_json::json!({
        "text":        captured,
        "context":     ctx,
        "app_context": app_category,
    })).ok();

    if let Some(window) = handle.get_webview_window("main") {
        let _ = window.center();
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.set_always_on_top(true);
    }

    // Fire AI classifier async if NLP confidence is low
    if nlp::intent::should_fire_ai_classifier(&ctx.intent_result) {
        let h2         = handle.clone();
        let text_clone = captured.clone();
        tauri::async_runtime::spawn(async move {
            let state   = h2.state::<AppState>();
            let api_key = {
                if let Ok(conn) = state.db.lock() {
                    keychain::get_api_key(&conn).unwrap_or_default()
                } else { String::new() }
            };
            let key = if api_key.is_empty() { "proxy".to_string() } else { api_key };
            let state   = h2.state::<AppState>();
            let mode    = get_current_ai_mode(&state);
            if let Some((intent, confidence, alts)) = ai::classify_intent(&key, &text_clone, mode, &state.worker_secret, &state.device_id).await {
                h2.emit("intent_refined", serde_json::json!({
                    "intent":       intent,
                    "confidence":   confidence,
                    "alternatives": alts.iter().map(|(i, c)| {
                        serde_json::json!({ "intent": i, "confidence": c })
                    }).collect::<Vec<_>>()
                })).ok();
            }
        });
    }
}

// ── Silent run (Alt+Shift+K / Alt+Shift+L) ────────────────────────────────
// Captures → shows loader toast → generates → injects → hides toast.
// Main window never shown.

fn run_silent(handle: &AppHandle, mode: &str) {
    let h    = handle.clone();
    let mode = mode.to_string();

    tauri::async_runtime::spawn(async move {
        // 1. Capture selected text
        let text = capture::capture_text().unwrap_or_default();
        if text.trim().is_empty() { return; }

        // 2. Show loader toast immediately so user knows something is happening
        loader_show(&h, &mode);

        let ctx   = nlp::analyze(&text);
        let state = h.state::<AppState>();

        // Single DB lock for all pre-generation reads
        let (api_key, profile, memory) = {
            if let Ok(db) = state.db.lock() {
                let key = Zeroizing::new(keychain::get_api_key(&db).unwrap_or_default());
                let prof = db::get_voice_profile(&db).unwrap_or_default();
                let names: Vec<String> = ctx.detected_entities.iter()
                    .map(|(n, _)| n.clone()).collect();
                let mem = db::get_entities_context(&db, &names).unwrap_or_default();
                (key, prof, mem)
            } else {
                (Zeroizing::new(String::new()), vec![], vec![])
            }
        };

        // 4. Build system prompt
        let app_exe      = capture::get_active_app();
        let app_category = app_exe.as_deref().map(capture::classify_app);
        let system_prompt = nlp::prompt::build_prompt(&h, &ctx, &mode, None, &profile, &memory, &[], None, app_category, None, &[]);

        let state     = h.state::<AppState>();
        let mode_enum = get_current_ai_mode(&state);
        // 6. Generate silently
        match ai::generate_silent(&api_key, &system_prompt, &text, mode_enum, &state.worker_secret, &state.device_id).await {
            Ok(result) => {
                // 7. Hide loader (shows ✓ Done · Ctrl+Z to undo for 2.5s), then inject
                loader_hide_with_hint(&h, true, true);
                inject::inject_text(result.clone());

                // 8. Save history (skip if incognito or sensitive data)
                use std::sync::atomic::Ordering;
                let incognito = state.incognito.load(Ordering::Relaxed);
                if !incognito && contains_sensitive_data(&text).is_none() {
                    match state.db.lock() {
                        Ok(db) => {
                            let preview = safe_truncate(&text, 1000);
                            let out_str = safe_truncate(&result, 2000);
                            let hash    = content_hash(&preview, &mode);
                            if let Err(e) = db::save_history(
                                &db, &preview, &mode, &out_str,
                                ctx.tone, ctx.formality, None, Some(&hash),
                            ) { eprintln!("[db] save_history failed (silent): {}", e); }
                            let _ = db::observe_session_v2(
                                &db, &text, ctx.tone, ctx.formality,
                                ctx.contraction_rate, ctx.avg_sentence_len, ctx.emoji_count,
                            );
                            let words: Vec<&str> = text.split_whitespace().collect();
                            let opener = words.first().map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase());
                            let closer  = words.last().map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase());
                            for (name, etype) in &ctx.detected_entities {
                                let _ = db::record_entity_mention(&db, etype, name, ctx.tone, ctx.formality);
                                if let Some(ref op) = opener {
                                    if !op.is_empty() { let _ = db::record_contact_pattern(&db, name, "opener", op); }
                                }
                                if let Some(ref cl) = closer {
                                    if !cl.is_empty() { let _ = db::record_contact_pattern(&db, name, "closer", cl); }
                                }
                            }
                            if !ctx.language.candidate_languages.is_empty() {
                                for (name, _) in &ctx.detected_entities {
                                    let _ = db::record_contact_language(&db, name, &ctx.language.candidate_languages);
                                }
                            }
                        }
                        Err(e) => eprintln!("[db] mutex poisoned (silent): {}", e),
                    }
                }
            }
            Err(e) => {
                loader_hide(&h, false);
                eprintln!("Silent transform failed (mode={}): {}", mode, e);
            }
        }
    });
}

// ── App entry point ────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ── Crash hook: write panic info to %APPDATA%\SnapText\crash_log.txt ──
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        let ts  = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let line = format!("[{}] PANIC: {}\n", ts, msg);
        if let Ok(appdata) = std::env::var("APPDATA") {
            let dir = std::path::Path::new(&appdata).join("SnapText");
            let _   = std::fs::create_dir_all(&dir);
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true).append(true).open(dir.join("crash_log.txt"))
            {
                use std::io::Write;
                let _ = f.write_all(line.as_bytes());
            }
        }
        eprintln!("{}", line);
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();

            // ── Database ────────────────────────────────────────────────
            let app_dir = handle.path().app_data_dir()
                .expect("Failed to resolve app data dir");
            let conn = db::init_db(&app_dir).expect("Failed to initialize database");

            // Read or generate worker secret (migrates hardcoded value on first run)
            let worker_secret = db::get_config(&conn, "app_secret").unwrap_or_else(|_| {
                let default = "snptxt_v1_8f3a2c7e9d1b4506".to_string();
                let _ = db::set_config(&conn, "app_secret", &default);
                default
            });

            // Generate a stable UUID device ID (replaces old FNV hash approach)
            let device_id = ai::get_or_create_device_id(&conn);

            // Data retention: auto-delete history older than configured days (default 90)
            let retention_days = db::get_config(&conn, "retention_days")
                .ok()
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(90);
            db::cleanup_old_history(&conn, retention_days);

            // Load adaptive correction overrides: if the user has corrected
            // a suggested mode → chosen mode at least 3 times, pre-select
            // the chosen mode in the future instead of the suggested one.
            let correction_overrides = {
                let mut map = std::collections::HashMap::new();
                if let Ok(pairs) = db::get_top_corrections(&conn) {
                    for (suggested, chosen) in pairs {
                        map.insert(suggested, chosen);
                    }
                }
                std::sync::RwLock::new(map)
            };

            handle.manage(AppState {
                db: Arc::new(Mutex::new(conn)),
                worker_secret,
                device_id,
                incognito: std::sync::atomic::AtomicBool::new(false),
                correction_overrides,
            });

            // ── Tray ────────────────────────────────────────────────────
            let quit_i     = MenuItem::with_id(&handle, "quit",      "Quit SnapText",    true, None::<&str>)?;
            let show_i     = MenuItem::with_id(&handle, "show",      "Show Overlay",     true, None::<&str>)?;
            let incog_i    = MenuItem::with_id(&handle, "incognito", "Private Mode: OFF", true, None::<&str>)?;
            let menu       = Menu::with_items(&handle, &[&show_i, &incog_i, &quit_i])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("SnapText — Alt+K")
                .on_menu_event({
                    let h = handle.clone();
                    move |app, event| {
                        if event.id == "quit" {
                            h.exit(0);
                        } else if event.id == "show" {
                            show_overlay(&h);
                        } else if event.id == "incognito" {
                            use std::sync::atomic::Ordering;
                            let state = h.state::<AppState>();
                            let now_on = !state.incognito.load(Ordering::Relaxed);
                            state.incognito.store(now_on, Ordering::Relaxed);
                            h.emit("incognito_changed", now_on).ok();
                        }
                    }
                })
                .build(app)?;

            // ── Global hotkeys ───────────────────────────────────────────
            //
            //  Alt+K          → show overlay (manual mode, preview before insert)
            //  Alt+Shift+K    → silent: capture → structure as AI Prompt → inject
            //  Alt+Shift+L    → silent: capture → rewrite to English → inject
            //
            // If a hotkey is already registered by another app, we log the error
            // and continue — the other hotkeys still work.
            //
            let k_main   = Shortcut::new(Some(Modifiers::ALT),                    Code::KeyK);
            let k_prompt = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyK);
            let k_fix    = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyL);

            // Alt+K — toggle overlay
            if let Err(e) = handle.global_shortcut().on_shortcut(k_main, {
                let h = handle.clone();
                move |_app, _shortcut, event| {
                    if event.state() != ShortcutState::Pressed { return; }
                    if let Some(window) = h.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            show_overlay(&h);
                        }
                    }
                }
            }) {
                eprintln!("⚠ Alt+K hotkey already taken by another app: {}. SnapText overlay will not open via this shortcut.", e);
            }

            // Alt+Shift+K — silent Prompt transform
            if let Err(e) = handle.global_shortcut().on_shortcut(k_prompt, {
                let h = handle.clone();
                move |_app, _shortcut, event| {
                    if event.state() != ShortcutState::Pressed { return; }
                    run_silent(&h, "Prompt");
                }
            }) {
                eprintln!("⚠ Alt+Shift+K hotkey already taken by another app: {}. Use Alt+K overlay instead.", e);
            }

            // Alt+Shift+L — silent rewrite to English
            if let Err(e) = handle.global_shortcut().on_shortcut(k_fix, {
                let h = handle.clone();
                move |_app, _shortcut, event| {
                    if event.state() != ShortcutState::Pressed { return; }
                    run_silent(&h, "Correct");
                }
            }) {
                eprintln!("⚠ Alt+Shift+L hotkey already taken by another app: {}. Use Alt+K overlay instead.", e);
            }

            // Register — errors here are non-fatal, log and continue
            if let Err(e) = handle.global_shortcut().register(k_main)   { eprintln!("Alt+K register failed: {}", e); }
            if let Err(e) = handle.global_shortcut().register(k_prompt) { eprintln!("Alt+Shift+K register failed: {}", e); }
            if let Err(e) = handle.global_shortcut().register(k_fix)    { eprintln!("Alt+Shift+L register failed: {}", e); }

            // ── Clipboard monitor (floating pill fallback) ──────────
            start_clipboard_monitor(handle.clone());

            // ── Auto-updater: silent background check on startup ────
            {
                let h = handle.clone();
                tauri::async_runtime::spawn(async move {
                    // Wait 10s so startup I/O completes first
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    use tauri_plugin_updater::UpdaterExt;
                    if let Ok(updater) = h.updater() {
                        if let Ok(Some(update)) = updater.check().await {
                            h.emit("update_available", serde_json::json!({
                                "version": update.version,
                                "body": update.body.unwrap_or_default(),
                            })).ok();
                        }
                    }
                });
            }

            // ── Auto-show on first run so user sees setup screen ────
            {
                let state = handle.state::<AppState>();
                let is_first_run = if let Ok(conn) = state.db.lock() {
                    db::get_config(&conn, "first_run_done").is_err()
                } else { true };

                if is_first_run {
                    if let Some(window) = handle.get_webview_window("main") {
                        let _ = window.center();
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_captured_text,
            generate_ai_response,
            inject_result,
            store_api_key,
            has_api_key,
            delete_api_key,
            hide_window,
            list_available_models,
            analyze_text,
            record_intent_correction,
            get_voice_profile,
            get_communication_score,
            get_device_id,
            get_history,
            record_reply_feedback,
            get_config_value,
            set_config_value,
            get_worker_usage,
            get_ai_mode,
            set_ai_mode,
            get_hardware_stats,
            pill_clicked,
            hide_pill,
            get_audit_log,
            toggle_incognito,
            get_incognito,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}