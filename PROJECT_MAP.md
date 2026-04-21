# PROJECT_MAP — SnapText

## Directory Structure

```
├── src/                    # React frontend (Vite + TypeScript)
│   ├── main.tsx            ← Frontend entry point
│   ├── App.tsx             ← Main UI component (overlay, modes, streaming)
│   ├── loader.ts           # Toast notification handler
│   └── index.css           # Dark theme, glassmorphic styles
│
├── src-tauri/              # Rust backend (Tauri v2)
│   ├── src/
│   │   ├── main.rs         ← Backend entry point
│   │   ├── lib.rs          # App setup, hotkeys, Tauri commands
│   │   ├── ai.rs           # AI routing (Worker / BYOK / Ollama)
│   │   ├── capture.rs      # Clipboard read (Ctrl+C simulation)
│   │   ├── inject.rs       # Clipboard write (Ctrl+V simulation)
│   │   ├── db.rs           # SQLite (history, voice profile, config)
│   │   ├── keychain.rs     # API key encryption (XOR)
│   │   ├── ollama.rs       # Local model support
│   │   └── nlp/            # 5-stage NLP pipeline
│   │       ├── mod.rs      # Pipeline orchestrator
│   │       ├── intent.rs   # Intent classifier (35 signals)
│   │       ├── language.rs # 18-script detection + Hinglish
│   │       ├── features.rs # Tone, formality, keywords
│   │       ├── prompt.rs   # Dynamic system prompt builder
│   │       ├── normalize.rs# Unicode cleanup
│   │       └── local_engine.rs # Rule-based offline fallback
│   ├── tauri.conf.json     # Window config, CSP, app metadata
│   └── Cargo.toml          # Rust dependencies
│
├── worker/                 # Cloudflare Worker (API proxy)
│   ├── worker.js           ← Worker entry point (key rotation, rate limit)
│   └── wrangler.toml       # Worker config + KV binding
│
├── index.html              # Vite HTML shell (main window)
├── loader.html             # Toast HTML shell
├── package.json            # Node dependencies
└── vite.config.ts          # Vite build config (dual entry points)
```

## Main Modules

| Module | Role |
|--------|------|
| `src/App.tsx` | Overlay UI, mode selection, streaming display |
| `src-tauri/src/lib.rs` | Hotkey registration, Tauri command handlers |
| `src-tauri/src/ai.rs` | AI call routing: Worker → Gemini proxy, BYOK → direct, Local → Ollama |
| `src-tauri/src/nlp/` | Text analysis: normalize → language → features → intent → prompt |
| `src-tauri/src/db.rs` | SQLite persistence: history, voice profile, context memory |
| `worker/worker.js` | Cloudflare proxy: 3-key rotation, 20/day device limit |

## Entry Points

- **Desktop app:** `src-tauri/src/main.rs` → `lib.rs::run()`
- **Frontend:** `src/main.tsx` → renders `<App />`
- **API proxy:** `worker/worker.js` → `fetch()` handler
