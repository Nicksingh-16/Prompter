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
    true // Proxy mode: no user key ever needed
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

#[tauri::command]
async fn get_worker_usage() -> Option<serde_json::Value> {
    ai::get_worker_usage().await.map(|(used, cap)| {
        serde_json::json!({ "used": used, "cap": cap })
    })
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
    if let Some(w) = handle.get_webview_window("loader") {
        handle.emit("loader_state", serde_json::json!({
            "state": if success { "done" } else { "error" },
            "label": if success { "Done" } else { "Failed" },
        })).ok();
        // Brief pause so user sees the result state before it disappears
        std::thread::sleep(std::time::Duration::from_millis(700));
        let _ = w.hide();
    }
}

// ── Show overlay (Alt+K) ───────────────────────────────────────────────────

fn show_overlay(handle: &AppHandle) {
    let captured = capture::capture_text().unwrap_or_default();
    let ctx      = nlp::analyze(&captured);

    handle.emit("text_captured", serde_json::json!({
        "text":    captured,
        "context": ctx,
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
            if let Some((intent, confidence, alts)) = ai::classify_intent(&key, &text_clone).await {
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

        // 3. Load voice profile + context memory
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

        // 4. Build system prompt
        let system_prompt = nlp::prompt::build_prompt(&h, &ctx, &mode, None, &profile, &memory);

        // 5. Get API key (empty = proxy handles it)
        let api_key = {
            if let Ok(db) = state.db.lock() {
                keychain::get_api_key(&db).unwrap_or_default()
            } else { String::new() }
        };

        // 6. Generate silently
        match ai::generate_silent(&api_key, &system_prompt, &text).await {
            Ok(result) => {
                // 7. Hide loader (shows ✓ Done briefly), then inject
                loader_hide(&h, true);
                inject::inject_text(result.clone());

                // 8. Save history
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
                loader_hide(&h, false);
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
            let quit_i = MenuItem::with_id(&handle, "quit", "Quit SnapText", true, None::<&str>)?;
            let show_i = MenuItem::with_id(&handle, "show", "Show Overlay",  true, None::<&str>)?;
            let menu   = Menu::with_items(&handle, &[&show_i, &quit_i])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("SnapText — Alt+K")
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
            get_worker_usage,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}