# SnapText — What We've Built

## Overview

SnapText is an AI-powered writing assistant that eliminates the ChatGPT copy-paste loop. Instead of switching apps to get AI help with messages, SnapText works right where you type — select text, tap, done.

Two platforms: **Desktop (Windows)** and **Mobile (Android)**.

---

## Desktop App (Windows)

**Stack:** Tauri 2 (Rust backend) + React/TypeScript frontend

### How It Works
1. User types in any app (WhatsApp Web, Gmail, Slack, etc.)
2. Presses `Alt+K` or double-copies text
3. SnapText overlay appears with AI-transformed text
4. User clicks "Insert" — text gets injected back into the original app via simulated Ctrl+V

### Core Features

**AI Modes (10):**
| Mode | What It Does |
|------|-------------|
| Reply | Composes a reply to a received message |
| Correct | Rewrites broken English/Hinglish into clean English |
| Professional | Polishes tone to sound confident |
| Casual | Makes text sound natural and conversational |
| Email | Turns notes into structured emails |
| Translate | Converts between languages |
| Prompt | Turns rough ideas into structured AI prompts |
| Summarize | Extracts key points from long text |
| Knowledge | Provides explanations and answers |
| Strategist | Executive-level tone critique |

**NLP Engine (6-Stage Pipeline):**
1. **Normalize** — Cleans up Unicode, smart quotes, extra spaces
2. **Language Detect** — Identifies 18 scripts (Latin, Devanagari, Arabic, CJK, etc.) + Hinglish detection
3. **Intent Classify** — 35+ signals to auto-detect what the user wants (Email? Reply? Code help?)
4. **Feature Extract** — Tone score (-5 to +5), formality score (0-10), friction phrase detection, keyword extraction
5. **Prompt Build** — Constructs mode-specific system prompts with all context
6. **Local Fallback** — Offline grammar fixes and summarization when AI is unavailable

**Voice Profile Learning:**
- Learns user's writing style over time (openers, closers, vocabulary)
- Tracks relationship context (who you talk to, typical tone with each person)
- Feeds into prompts so AI output sounds like YOU, not generic

**ToneMirror:**
- Real-time tone feedback (green = warm, yellow = tense, red = harsh)
- Friction phrase detection ("per my last email", "as I already mentioned")
- Pulses when passive-aggressive language detected

**Communication Score:**
- Weekly stats: average tone, formality trends, most-contacted entities
- History of last 100 transformations

**AI Backend (3 modes):**
| Mode | How | Cost |
|------|-----|------|
| Worker (default) | Cloudflare Worker proxies to Gemini API | Free, 20/day |
| BYOK | User's own Gemini API key, direct calls | User pays Gemini |
| Local | Ollama (phi3:mini or gemma2) on user's machine | Free, offline |

**Hotkeys:**
- `Alt+K` — Open overlay
- `Alt+Shift+K` — Silent Prompt transform (no UI)
- `Alt+Shift+L` — Silent English correction (no UI)
- Double-copy — Floating pill UI near cursor

### Desktop Files
```
src-tauri/src/
  lib.rs          — 20 Tauri commands (capture, generate, inject, history, etc.)
  ai.rs           — Streaming AI (Worker/Byok/Local) with key rotation
  inject.rs       — Clipboard save/restore + Ctrl+V simulation
  capture.rs      — Text capture from clipboard
  db.rs           — SQLite (history, voice profile, context memory, config)
  keychain.rs     — API key storage (XOR obfuscation)
  ollama.rs       — Local Ollama integration
  nlp/
    mod.rs        — Pipeline coordinator
    normalize.rs  — Stage 1
    language.rs   — Stage 2 (18 scripts)
    intent.rs     — Stage 3 (35+ signals)
    features.rs   — Stage 4 (tone, formality, friction, entities)
    prompt.rs     — Stage 5 (10 mode prompts)
    local_engine.rs — Stage 6 (offline fallback)

src/
  App.tsx         — React UI (ToneMirror, SuggestionBar, HistoryPanel, Settings)
  index.css       — Dark theme, glass cards, animations

worker/
  worker.js       — Cloudflare Worker (Gemini proxy, 20/day cap, key rotation)
```

---

## Mobile App (Android)

**Stack:** Native Android (Kotlin), IME Service + Jetpack Compose settings

### How It Works
**Method 1 — Keyboard:**
1. User switches to SnapText keyboard
2. Types normally, taps the sparkle button
3. AI auto-detects intent, shows mode chips
4. Taps a mode — AI transforms text, streams result
5. Taps "Insert" to replace text or "Copy" to clipboard

**Method 2 — Select & Transform (PROCESS_TEXT):**
1. User selects text in ANY app (WhatsApp, Chrome, etc.)
2. Taps "Reply with SnapText" in the context menu
3. Bottom sheet appears with AI result
4. Taps "Insert" to replace selection

### Core Features

**AI Modes (8):**
Reply, Correct, Professional, Casual, Email, Translate, Expand (GhostWriter), Summarize

**Language-Aware Replies:**
- All modes preserve input language (Hinglish in = Hinglish out)
- Only "Correct" mode converts to English (that's its purpose)
- Dialect-aware: Rajasthani, Punjabi flavors maintained in Reply mode

**Keyboard:**
- QWERTY layout with shift, symbols, backspace
- Long-press backspace for continuous delete
- Long-press Enter for "Say It Better" instant rewrite
- Voice input with visual feedback (mic turns red, shows partial transcription)
- Tools row: Clipboard Hub, Templates

**Tone Guard:**
- 3dp colored strip at top of keyboard (green/yellow/red)
- Emoji indicator in AI bar
- Suggestion text shows warnings ("This can sound passive-aggressive")
- Checks 600ms after last keystroke (not on every key)
- 25+ friction phrases, harsh word detection, ALL-CAPS detection

**Clipboard Hub:**
- Auto-captures everything copied
- Categorizes: Links, Emails, Phones, Code, Addresses, Numbers, Text
- Pin important items
- Tap to paste, long-press to transform with AI
- 30-item history, persisted to SharedPreferences

**Quick Templates:**
- 15 built-in templates across Work, Social, Errands, Follow-ups
- Placeholder syntax: `{name}`, `{date}`, `{time}`, etc.
- Tap template → fill placeholders → insert
- Usage tracking (most-used float to top)

**Analytics Tracker:**
- Daily transform count, tone scores
- Weekly summary (streak, top mode, tone trend)
- Shown in Settings dashboard

**Onboarding:**
- 3-step flow: Welcome → Enable keyboard → Set as default
- Compose-based UI with dark theme

**Settings:**
- AI Provider toggle (Cloud free / BYOK)
- Feature toggles: Tone Guard, Smart Reply, Translate
- Language selector for translate target
- Analytics dashboard (streak, transforms, tone trend)

### Mobile Files
```
app/src/main/java/com/snaptext/keyboard/
  SnapTextIME.kt           — Main keyboard service (all UI + feature wiring)
  SnapTextApp.kt            — Application class (OkHttp singleton)
  ui/
    SettingsActivity.kt     — Onboarding + settings (Compose)
    ProcessTextActivity.kt  — "Reply with SnapText" context menu action
  ai/
    WorkerClient.kt         — API client (Worker + direct Gemini, SSE streaming)
  clipboard/
    ClipboardHub.kt         — Smart clipboard manager with categories
  data/
    Preferences.kt          — SharedPreferences wrapper
    TemplateManager.kt      — Parameterized template system
    AnalyticsTracker.kt     — Usage/tone tracking
  nlp/
    IntentDetector.kt       — 10-signal intent classifier
    LanguageDetector.kt     — 6-script + Hinglish detection
    PromptBuilder.kt        — System prompt builder (11 modes)
    ToneAnalyzer.kt         — Real-time tone scoring

app/src/main/res/
  layout/
    keyboard_view.xml       — Main keyboard (5 rows + panels)
    ai_bar.xml              — Mode chips + tone indicators
    ai_result_panel.xml     — Streaming result display
    clipboard_panel.xml     — Clipboard hub
    template_panel.xml      — Quick templates
  values/
    colors.xml              — 40+ colors (dark theme, mode colors, tone colors)
    strings.xml             — All user-facing text
    dimens.xml              — Key heights, padding values
    themes.xml              — Material dark themes
  drawable/
    bg_key.xml, bg_key_special.xml, bg_ai_button.xml, etc.
```

---

## Shared Infrastructure

**Cloudflare Worker** (`worker/worker.js`):
- Proxies all AI calls through `snaptext-worker.snaptext-ai.workers.dev`
- Per-device daily cap: 20 transforms (tracked via KV storage, 24h TTL)
- 3-key rotation for Gemini API (auto-fallback on 429 rate limit)
- Endpoints: `POST /generate` (streaming), `GET /usage` (quota check)
- Auth: `X-Device-ID` (FNV-1a hash) + `X-App-Secret`

**Gemini API:**
- Model: `gemini-2.5-flash`
- Streaming via SSE (Server-Sent Events)
- Temperature: 0.7, Max tokens: 2048 (mobile) / 4096 (desktop)

---

## Tech Specs

| | Desktop | Mobile |
|---|---------|--------|
| Language | Rust + TypeScript | Kotlin |
| Framework | Tauri 2 | Android IME |
| UI | React + CSS | XML layouts + Compose |
| Database | SQLite (rusqlite) | SharedPreferences (JSON) |
| HTTP | reqwest (async) | OkHttp 4 |
| AI | Gemini 2.5 Flash | Gemini 2.5 Flash |
| NLP signals | 35+ | 10 |
| Script detection | 18 scripts | 6 scripts |
| Voice learning | Yes (SQLite) | No (planned) |
| Offline fallback | Yes (local engine) | No |
| Min OS | Windows 10+ | Android 7.0 (API 24) |

---

## Permissions

**Desktop:** No special permissions (runs as regular app)

**Android:**
- `INTERNET` — API calls
- `VIBRATE` — Haptic feedback on key press
- `RECORD_AUDIO` — Voice input
- `BIND_INPUT_METHOD` — Keyboard service
