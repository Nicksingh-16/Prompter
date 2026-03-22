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
        keychain::get_api_key(&conn).map_err(|e| {
            println!(">>> KEYCHAIN ERROR: {}", e);
            format!("API Key Error: {}", e)
        })?
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
        &app,
        &ctx,
        &mode,
        sub_mode.as_deref().or(custom_prompt.as_deref()),
        &profile,
        &memory,
    );

    let res = ai::generate_stream(app.clone(), &api_key, &system_prompt, &text).await;

    if res.is_ok() {
        let conn = state.db.lock().map_err(|e| format!("DB lock error: {}", e))?;
        let _ = db::save_history(
            &conn,
            &text[..text.len().min(200)],
            &mode,
            "Generated successfully",
            ctx.tone,
            ctx.formality,
        );
        let _ = db::observe_session(&conn, &text, ctx.tone, ctx.formality, ctx.word_count);
        for (name, etype) in &ctx.detected_entities {
            let _ = db::record_entity_mention(&conn, etype, name, ctx.tone, ctx.formality);
        }
    }

    res
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
    // Tiny delay to ensure window is gone from OS focus stack
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
    let ctx = nlp::analyze(&text);
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
fn has_api_key(state: State<'_, AppState>) -> bool {
    let conn_res = state.db.lock();
    if let Ok(conn) = conn_res {
        keychain::get_api_key(&conn).is_ok()
    } else {
        false
    }
}

#[tauri::command]
async fn list_available_models(state: State<'_, AppState>) -> Result<String, String> {
    let api_key = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        keychain::get_api_key(&conn)?
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

// ── Overlay show logic ─────────────────────────────────────────────────────
//
// ORDER IS CRITICAL:
//   1. capture_text() FIRST  — reads selected text while previous app owns focus
//   2. nlp::analyze()        — <5ms, builds full TextContext
//   3. emit()                — sends payload to frontend
//   4. show() + set_focus()  — LAST, only now does our window steal focus
//
// Reversing 1 and 4 means our window grabs focus before the clipboard
// copy fires, so capture reads our own empty webview instead.

fn show_overlay(handle: &AppHandle) {
    // 1. Capture while previous app still has OS focus
    let captured = capture::capture_text().unwrap_or_default();
    println!(">>> OVERLAY: captured {} chars", captured.len());

    // 2. Run local NLP pipeline synchronously (<5ms)
    let ctx = nlp::analyze(&captured);
    println!(
        ">>> NLP: intent={:?} conf={:.2} lang={}",
        ctx.intent_result.primary.intent,
        ctx.intent_result.primary.confidence,
        ctx.language.candidate_languages,
    );

    // 3. Emit structured payload to frontend
    let payload = serde_json::json!({ "text": captured, "context": ctx });
    handle.emit("text_captured", payload).ok();

    // 4. Show window — AFTER capture and emit
    if let Some(window) = handle.get_webview_window("main") {
        let _ = window.center();
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.set_always_on_top(true);
    }

    // 5. Fire AI classifier async if local confidence is low (Layer 3)
    if nlp::intent::should_fire_ai_classifier(&ctx.intent_result) {
        let h2 = handle.clone();
        let text_clone = captured.clone();
        tauri::async_runtime::spawn(async move {
            let state = h2.state::<AppState>();
            let api_key_res = {
                if let Ok(conn) = state.db.lock() {
                    keychain::get_api_key(&conn)
                } else {
                    Err("DB Lock Error".to_string())
                }
            };

            if let Ok(api_key) = api_key_res {
                if let Some((intent, confidence, alts)) =
                    ai::classify_intent(&api_key, &text_clone).await
                {
                    println!(">>> AI CLASSIFIER: intent={} conf={:.2}", intent, confidence);
                    let refined = serde_json::json!({
                        "intent": intent,
                        "confidence": confidence,
                        "alternatives": alts.iter().map(|(i, c)| {
                            serde_json::json!({ "intent": i, "confidence": c })
                        }).collect::<Vec<_>>()
                    });
                    h2.emit("intent_refined", refined).ok();
                }
            }
        });
    }
}

// ── App entry point ────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    println!(">>> STARTUP: run() entered");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            println!(">>> STARTUP: setup() entered");
            let handle = app.handle().clone();

            // ── Database ────────────────────────────────────────────────
            let app_dir = handle.path().app_data_dir()
                .expect("Failed to resolve app data dir");
            let conn = db::init_db(&app_dir).expect("Failed to initialize database");
            handle.manage(AppState {
                db: Arc::new(Mutex::new(conn)),
            });
            println!(">>> STARTUP: database initialized");

            // ── Tray ────────────────────────────────────────────────────
            let quit_i = MenuItem::with_id(&handle, "quit", "Quit Antigravity", true, None::<&str>)?;
            let show_i = MenuItem::with_id(&handle, "show", "Show Overlay",     true, None::<&str>)?;
            let menu = Menu::with_items(&handle, &[&show_i, &quit_i])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("Antigravity — Alt+K")
                .on_menu_event({
                    let h = handle.clone();
                    move |_app, event| {
                        if event.id == "quit" {
                            h.exit(0);
                        } else if event.id == "show" {
                            show_overlay(&h);
                        }
                    }
                })
                .build(app)?;
            println!(">>> STARTUP: tray icon built");

            // ── Global hotkey Alt+K ─────────────────────────────────────
            //
            // Tauri v2 two-step registration:
            //   Step 1 — on_shortcut() binds the handler
            //   Step 2 — register() tells the OS to intercept the keypress
            //
            // The window starts HIDDEN (visible:false in tauri.conf.json).
            // This hotkey is the only entry point. It fires from any app,
            // any window state, including when overlay is completely hidden.

            let hotkey = Shortcut::new(Some(Modifiers::ALT), Code::KeyK);

            handle.global_shortcut().on_shortcut(hotkey, {
                let h = handle.clone();
                move |_app, _shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    println!(">>> HOTKEY: Alt+K fired");

                    let Some(window) = h.get_webview_window("main") else {
                        println!(">>> HOTKEY ERROR: 'main' window not found");
                        return;
                    };

                    if window.is_visible().unwrap_or(false) {
                        // Toggle: press Alt+K again to dismiss
                        println!(">>> HOTKEY: hiding overlay");
                        let _ = window.hide();
                    } else {
                        println!(">>> HOTKEY: showing overlay");
                        show_overlay(&h);
                    }
                }
            })?;

            if let Err(e) = handle.global_shortcut().register(hotkey) {
                println!(">>> HOTKEY WARNING: Alt+K registration failed — {}. (Check if another app is using it or a ghost version is running)", e);
            } else {
                println!(">>> STARTUP: Alt+K registered — ready");
            }

            // ── DO NOT call window.show() here ──────────────────────────
            // Window starts hidden. Only hotkey or tray "Show Overlay" opens it.

            println!(">>> STARTUP: setup() complete");
            Ok(())
        })
        .on_window_event(|window, event| {
            // Intercept close button — hide instead of destroying the window.
            // This keeps the process alive so the hotkey keeps working.
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}