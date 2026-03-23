mod capture;
mod inject;
mod db;
mod keychain;
mod ai;
mod nlp;

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Emitter, State, menu::{Menu, MenuItem}, tray::TrayIconBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState, Shortcut, Modifiers, Code};

// ── App state ──────────────────────────────────────────────────────────────

pub struct AppState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
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
    println!(">>> AI: mode={} text_len={}", mode, text.len());

    let api_key = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        keychain::get_api_key(&conn).unwrap_or_default()
    };
    let profile = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db::get_voice_profile(&db).unwrap_or_default()
    };

    let ctx = nlp::analyze(&text);

    let memory = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let names: Vec<String> = ctx.detected_entities.iter().map(|(n, _)| n.clone()).collect();
        db::get_entities_context(&db, &names).unwrap_or_default()
    };

    let system_prompt = nlp::prompt::build_prompt(
        &app, &ctx, &mode,
        sub_mode.as_deref().or(custom_prompt.as_deref()),
        &profile, &memory,
    );

    let res = ai::generate_stream(app.clone(), &api_key, &system_prompt, &text).await;

    if let Ok(ref output) = res {
        let conn = state.db.lock().map_err(|e| format!("DB lock error: {}", e))?;
        let _ = db::save_history(
            &conn,
            &text[..text.len().min(200)],
            &mode,
            &output[..output.len().min(500)],
            ctx.tone,
            ctx.formality,
        );
        let _ = db::observe_session(&conn, &text, ctx.tone, ctx.formality, ctx.word_count);
        for (name, etype) in &ctx.detected_entities {
            let _ = db::record_entity_mention(&conn, etype, name, ctx.tone, ctx.formality);
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

#[tauri::command]
fn generate_local_response(
    state: State<'_, AppState>,
    mode: String,
    text: String,
    _sub_mode: Option<String>,
) -> String {
    let ctx    = nlp::analyze(&text);
    let result = nlp::local_engine::transform(&mode, &ctx);
    if let Ok(conn) = state.db.lock() {
        let _ = db::observe_session(&conn, &text, ctx.tone, ctx.formality, ctx.word_count);
        for (name, etype) in &ctx.detected_entities {
            let _ = db::record_entity_mention(&conn, etype, name, ctx.tone, ctx.formality);
        }
    }
    result
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
fn has_api_key(_state: State<'_, AppState>) -> bool {
    // Proxy mode: no user key needed, always return true to skip onboarding
    true
}

#[tauri::command]
async fn list_available_models(state: State<'_, AppState>) -> Result<String, String> {
    let api_key = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        keychain::get_api_key(&conn).unwrap_or_default()
    };
    ai::list_models(&api_key).await
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
        .map_err(|e| e.to_string())
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
fn get_device_id() -> String {
    ai::device_id()
}

// ── Show overlay (Alt+K) ───────────────────────────────────────────────────
// Captures selected text, shows the overlay window, lets user choose mode.

fn show_overlay(handle: &AppHandle) {
    let captured = capture::capture_text().unwrap_or_default();
    let ctx      = nlp::analyze(&captured);

    let payload = serde_json::json!({
        "text":    captured,
        "context": ctx,
    });
    handle.emit("text_captured", payload).ok();

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
            if let Some((intent, confidence, alts)) = ai::classify_intent(&key, &text_clone).await {
                let refined = serde_json::json!({
                    "intent":       intent,
                    "confidence":   confidence,
                    "alternatives": alts.iter().map(|(i, c)| {
                        serde_json::json!({ "intent": i, "confidence": c })
                    }).collect::<Vec<_>>()
                });
                h2.emit("intent_refined", refined).ok();
            }
        });
    }
}

// ── Silent run (Alt+Shift+K / Alt+Shift+L) ────────────────────────────────
// Captures → generates → injects. Window never shown. Fully background.

fn run_silent(handle: &AppHandle, mode: &str) {
    let h    = handle.clone();
    let mode = mode.to_string();

    tauri::async_runtime::spawn(async move {
        // 1. Capture selected text
        let text = capture::capture_text().unwrap_or_default();
        if text.trim().is_empty() { return; }

        let ctx   = nlp::analyze(&text);
        let state = h.state::<AppState>();

        // 2. Load voice profile + context memory
        let profile = {
            if let Ok(db) = state.db.lock() {
                db::get_voice_profile(&db).unwrap_or_default()
            } else { vec![] }
        };
        let memory = {
            if let Ok(db) = state.db.lock() {
                let names: Vec<String> = ctx.detected_entities.iter()
                    .map(|(n, _)| n.clone()).collect();
                db::get_entities_context(&db, &names).unwrap_or_default()
            } else { vec![] }
        };

        // 3. Build system prompt via NLP pipeline (same as overlay path)
        let system_prompt = nlp::prompt::build_prompt(&h, &ctx, &mode, None, &profile, &memory);

        // 4. Get API key (empty string is fine — proxy ignores it)
        let api_key = {
            if let Ok(db) = state.db.lock() {
                keychain::get_api_key(&db).unwrap_or_default()
            } else { String::new() }
        };

        // 5. Generate silently (non-streaming, no window events)
        match ai::generate_silent(&api_key, &system_prompt, &text).await {
            Ok(result) => {
                // 6. Inject result directly into the source app
                // inject_text has a 300ms delay built in — that's intentional,
                // it gives the OS time to return focus to the source window
                // after the hotkey releases.
                inject::inject_text(result.clone());

                // 7. Save to history so it shows up in the overlay panel
                if let Ok(db) = state.db.lock() {
                    let _ = db::save_history(
                        &db,
                        &text[..text.len().min(200)],
                        &mode,
                        &result[..result.len().min(500)],
                        ctx.tone,
                        ctx.formality,
                    );
                    let _ = db::observe_session(&db, &text, ctx.tone, ctx.formality, ctx.word_count);
                    for (name, etype) in &ctx.detected_entities {
                        let _ = db::record_entity_mention(&db, etype, name, ctx.tone, ctx.formality);
                    }
                }
            }
            Err(e) => {
                // Silent failure — log to console, don't crash or show overlay
                eprintln!("Silent transform failed (mode={}): {}", mode, e);
            }
        }
    });
}

// ── App entry point ────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();

            // ── Database ────────────────────────────────────────────────
            let app_dir = handle.path().app_data_dir()
                .expect("Failed to resolve app data dir");
            let conn = db::init_db(&app_dir).expect("Failed to initialize database");
            handle.manage(AppState {
                db: Arc::new(Mutex::new(conn)),
            });

            // ── Tray ────────────────────────────────────────────────────
            let quit_i = MenuItem::with_id(&handle, "quit", "Quit Prompter", true, None::<&str>)?;
            let show_i = MenuItem::with_id(&handle, "show", "Show Overlay",  true, None::<&str>)?;
            let menu   = Menu::with_items(&handle, &[&show_i, &quit_i])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("Prompter — Alt+K")
                .on_menu_event({
                    let h = handle.clone();
                    move |_app, event| {
                        if event.id == "quit"      { h.exit(0); }
                        else if event.id == "show" { show_overlay(&h); }
                    }
                })
                .build(app)?;

            // ── Global hotkeys ───────────────────────────────────────────
            //
            //  Alt+K          → show overlay (manual mode selection)
            //  Alt+Shift+K    → silent: capture → Prompt → inject
            //  Alt+Shift+L    → silent: capture → Correct → inject
            //
            let k_main   = Shortcut::new(Some(Modifiers::ALT),                Code::KeyK);
            let k_prompt = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyK);
            let k_fix    = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyL);

            // Alt+K — toggle overlay
            handle.global_shortcut().on_shortcut(k_main, {
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
            })?;

            // Alt+Shift+K — silent Prompt transform + auto-inject
            handle.global_shortcut().on_shortcut(k_prompt, {
                let h = handle.clone();
                move |_app, _shortcut, event| {
                    if event.state() != ShortcutState::Pressed { return; }
                    run_silent(&h, "Prompt");
                }
            })?;

            // Alt+Shift+L — silent Correct/rewrite transform + auto-inject
            handle.global_shortcut().on_shortcut(k_fix, {
                let h = handle.clone();
                move |_app, _shortcut, event| {
                    if event.state() != ShortcutState::Pressed { return; }
                    run_silent(&h, "Correct");
                }
            })?;

            let _ = handle.global_shortcut().register(k_main);
            let _ = handle.global_shortcut().register(k_prompt);
            let _ = handle.global_shortcut().register(k_fix);

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
            generate_local_response,
            analyze_text,
            record_intent_correction,
            get_voice_profile,
            get_communication_score,
            get_device_id,
            get_history,
            get_config_value,
            set_config_value,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}