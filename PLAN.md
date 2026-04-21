# SnapText — A-Grade Product Plan
# Goal: Best-in-class AI writing tool that big players can't replicate in 1-2 years

---

## The Moat Strategy

The product that wins is not the one with the most AI modes.
It's the one that after 90 days feels like you hired a personal assistant
who has been watching you communicate for 3 months.

Five pillars no big player can replicate fast:
1. Voice Fingerprint — your writing DNA, built over time
2. Relationship Graph — how you talk to each specific person
3. App Context Awareness — behavior adapts per app (WhatsApp vs Gmail vs Slack)
4. India-First Language Depth — Hinglish, Gujarati Roman, Indian professional context
5. Security & Privacy — encrypted at rest, zero telemetry, local-first option

---

## Current Security Audit

### What exists (good)
- X-App-Secret header auth on Cloudflare Worker
- CORS restricted to tauri://localhost only
- Per-device daily cap via KV storage
- Prompt injection sanitization (NFKC + case-insensitive patterns)
- SHA-256 content hashing for deduplication

### Critical gaps
| Gap | Risk | Fix |
|-----|------|-----|
| SQLite stored in plaintext | Anyone with file access reads all your messages | Encrypt with DPAPI-derived key |
| XOR key obfuscation | Not real encryption — trivially reversible | Windows Credential Manager (keyring crate) |
| No sensitive data detection | Password/CC/Aadhaar sent to Gemini and stored in history | Regex detection → block storage + warn user |
| Captured text never zeroed | Sensitive text sits in memory for app lifetime | zeroize crate after use |
| No data retention limit | History grows forever, GDPR exposure | Auto-delete after 90 days (configurable) |
| No incognito mode | Every transform stored | Private session toggle |
| No audit log | User can't verify what was sent | Per-call log (mode + char count, not content) |
| Worker: no body size limit | Abuse vector for large payloads | 10KB hard cap in Worker |
| DevTools enabled in dev build | Accidental exposure in production | Disable in release tauri.conf.json |

---

## Week 1 — Foundation: Clean Code + Security Core

### Day 1: Dead Code Removal

Delete entirely:
- `generate_local_response()` — lib.rs:166 (deprecated Tauri command)
- `get_mode_history()` — db.rs:429 (never called)
- `get_history_without_embeddings()` — db.rs:170 (never called)
- `update_embedding()` — db.rs:186 (never called)
- `cosine_similarity()` — embedding.rs:159 (dead, no neural embeddings)
- `bytes_to_vec()` / `vec_to_bytes()` — embedding.rs (test-only)
- `local_engine::transform()` — nlp/local_engine.rs (deprecated)
- `intent_weight_overrides` table + all functions — db.rs:47 (feedback loop broken)
- `subIntent` state — App.tsx (declared, never used)
- `.toggle-switch` CSS class — index.css (never referenced)

Simplify:
- Language detector: keep Latin, Hinglish, RTL, CJK — remove 13 unused scripts (~80 lines)
- features.rs: keep friction phrase detection, remove numeric tone scoring (~40% reduction)
- intent.rs: remove alternative tracking, only top intent used (~30% reduction)
- Voice profile: remove per-session vocab tracking, keep openers/closers/formality only

### Day 2: Security — Encrypted Storage + Key Management

**SQLite Encryption (sqlcipher or AES wrapper):**
- Derive encryption key from Windows DPAPI at startup (machine + user bound)
- Key never stored — regenerated from DPAPI on each launch
- History, voice_profile, context_memory all encrypted at rest
- Zero config for user — transparent encryption

**Replace XOR with Windows Credential Manager:**
```rust
// Replace keychain.rs XOR with keyring crate
use keyring::Entry;
let entry = Entry::new("snaptext", "byok_api_key")?;
entry.set_password(&api_key)?;  // stored in Windows Credential Store
let key = entry.get_password()?;
```

**Memory zeroization after use:**
```rust
use zeroize::Zeroize;
// In capture.rs — zero the captured text after it's been used
let mut text = capture_text()?;
// ... use text ...
text.zeroize(); // cleared from memory
```

### Day 3: Security — Sensitive Data Detection + Audit Log

**Sensitive data detector (before any storage or AI call):**
```rust
// Patterns to detect and block
const SENSITIVE_PATTERNS: &[(&str, &str)] = &[
    (r"\d{4}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}", "credit card"),
    (r"\d{3}-\d{2}-\d{4}", "SSN"),
    (r"\d{12}", "Aadhaar"),
    (r"(?i)password\s*[:=]\s*\S+", "password"),
    (r"(?i)(api[_-]?key|secret|token)\s*[:=]\s*\S{10,}", "API key"),
    (r"[A-Za-z0-9+/]{40,}={0,2}", "base64 secret (likely key)"),
];
```
If detected:
- Block history storage entirely for this transform
- Show warning: "Sensitive data detected — not stored, not logged"
- Still allow transform (user's choice) but never persist

**Audit log table (metadata only, no content):**
```sql
CREATE TABLE audit_log (
    id         INTEGER PRIMARY KEY,
    timestamp  DATETIME DEFAULT CURRENT_TIMESTAMP,
    mode       TEXT,
    ai_mode    TEXT,    -- Worker/BYOK/Local
    char_count INTEGER, -- length of input, not content
    was_stored INTEGER  -- 0 if sensitive data blocked storage
);
```
User can export this as JSON from Settings. Shows exactly what modes were used, when, and whether content was stored. Builds trust.

**Worker hardening:**
```javascript
// Add to worker.js
const MAX_BODY_SIZE = 10 * 1024; // 10KB hard cap
if (request.headers.get("content-length") > MAX_BODY_SIZE) {
    return new Response("Payload too large", { status: 413 });
}
// Validate Content-Type
if (!request.headers.get("content-type")?.includes("application/json")) {
    return new Response("Bad Request", { status: 400 });
}
```

### Day 4: Security — Privacy Controls + Incognito Mode

**Incognito/Private session toggle:**
- Toggle in system tray menu: "Private Mode ON/OFF"
- When ON: transforms happen normally but nothing written to history, voice_profile, or audit_log
- Visual indicator: overlay header shows a lock icon
- Resets to OFF on app restart (never persists)
- Use case: transforming medical info, legal docs, personal messages you don't want stored

**Data retention auto-cleanup:**
```rust
// Run at startup, clean history older than retention_days (default 90)
fn cleanup_old_history(conn: &Connection, retention_days: i64) {
    conn.execute(
        "DELETE FROM history WHERE timestamp < datetime('now', ?)",
        params![format!("-{} days", retention_days)],
    ).ok();
}
```
Configurable in Settings: 30 / 60 / 90 / 180 days / Forever.

**Zero telemetry — explicit and verifiable:**
- No external analytics calls anywhere in codebase
- Settings page shows: "Zero Telemetry — SnapText never calls home. Verify in Settings > Privacy > Network Log"
- Network log (in-memory, not stored): shows every outbound HTTP call made in current session

### Day 5: NLP Simplification Complete + Security Review

- Complete all dead code removal from Day 1
- Run `cargo audit` — check all Rust dependencies for known CVEs
- Verify Tauri CSP in tauri.conf.json is production-strict
- Confirm devtools disabled in release build
- Test sensitive data detection with real patterns

---

## Week 2 — Intelligence: Real RAG + App Context

### Day 1-2: Real RAG for Reply Mode

**Add POST /embed to Cloudflare Worker:**
```javascript
// worker.js — new endpoint
if (url.pathname === "/embed") {
    const { text } = await request.json();
    const res = await fetch(
        `https://generativelanguage.googleapis.com/v1beta/models/text-embedding-004:embedContent?key=${key}`,
        { method: "POST", body: JSON.stringify({ content: { parts: [{ text }] } }) }
    );
    const data = await res.json();
    return new Response(JSON.stringify({ embedding: data.embedding.values }), { headers: CORS_HEADERS });
}
```

**Embed on save in db.rs:**
- After user inserts a Reply result → fire async embed call → store 768-dim vector as BLOB
- Non-blocking — runs after inject, user doesn't wait

**Cosine retrieval in Rust:**
```rust
// O(200 × 768) — ~2ms, no ML library needed
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 { 0.0 } else { dot / (mag_a * mag_b) }
}
```

**New Reply prompt with semantic examples:**
```
These are messages you personally handled that are semantically similar:

[Message they sent]: "Can we push the call to Friday?"
[Your reply]: "Sure, Friday works — what time suits you?"
---
[Message they sent]: "Need to reschedule our sync"
[Your reply]: "No problem, I'm free Thursday afternoon or Monday morning"
---
Now reply to: "Hey is there any way we can move tomorrow's meeting?"
```

### Day 3: App Context Detection

Detect active window process + title via Rust winapi:
```rust
// inject.rs or new context.rs
use windows::Win32::UI::WindowsAndMessaging::*;
fn get_active_app() -> AppContext {
    // GetForegroundWindow → GetWindowThreadProcessId → get process name
    // Also read window title for URL/tab context
}
```

| Detected App | Pill buttons shown | Default tone |
|-------------|-------------------|--------------|
| WhatsApp / Telegram | Reply, Casual, Translate, ··· | Casual short |
| Gmail / Outlook | Reply, Email, Professional, ··· | Formal long |
| Slack / Teams | Reply, Do, Correct, ··· | Professional-casual |
| Chrome / Edge (LinkedIn URL) | Professional, Do, Prompt, ··· | Professional |
| Chrome / Edge (Twitter URL) | Do, Casual, Prompt, ··· | Punchy short |
| VS Code | Prompt, Do, Correct, ··· | Technical |
| Word / Notion / Docs | Do, Professional, Summarize, ··· | Long-form |
| Default | Reply, Do, Correct, Prompt | Neutral |

### Day 4-5: Prompt Mode Domain Detection

Extend `detect_dev_input_type()` to cover natural language domains:

```rust
fn detect_natural_domain(text: &str) -> &'static str {
    let t = text.to_lowercase();
    if t.contains("email") || t.contains("mail") || t.contains("write to")
        { return "email_writing"; }
    if t.contains("tweet") || t.contains("post") || t.contains("caption") || t.contains(" # ")
        { return "social_media"; }
    if t.contains("negotiate") || t.contains("salary") || t.contains("raise") || t.contains("offer")
        { return "negotiation"; }
    if t.contains("explain") || t.contains("teach") || t.contains("how do i") || t.contains("learn")
        { return "teaching"; }
    if t.contains("story") || t.contains("creative") || t.contains("fiction") || t.contains("write a")
        { return "creative"; }
    if t.contains("analyze") || t.contains("report") || t.contains("research") || t.contains("findings")
        { return "analysis"; }
    if t.contains("pitch") || t.contains("investor") || t.contains("startup") || t.contains("deck")
        { return "pitch"; }
    "general"
}
```

Each domain gets a specialized prompt template (not the generic Role/Context/Task/Constraints).

Wire intent result into Prompt mode — if intent == Email → email-specific template, etc.

Specificity extraction: pull concrete nouns > 4 chars → inject into Role/Context directly.

---

## Week 3 — Personalization: Voice DNA + Relationship Graph

### Day 1-2: Deep Voice Learning

Replace shallow opener/closer tracking with full style fingerprinting:

```sql
CREATE TABLE voice_dna (
    feature     TEXT PRIMARY KEY,  -- 'avg_sentence_len', 'contraction_rate', 'emoji_never', etc.
    value       TEXT,
    sample_count INTEGER DEFAULT 0,
    updated_at  DATETIME
);
```

Track per transform:
- Average sentence length (your natural rhythm)
- Contraction rate (do you write "don't" or "do not"?)
- Emoji usage (never / sometimes / frequent)
- Hedging style ("I think" / "I believe" / direct)
- Passive vs active voice ratio
- Question style (rhetorical / direct / clarifying)
- Sign-off patterns per context (formal / casual)

After 50+ transforms, output should be measurably more like the user's own style.

### Day 3-4: Relationship Graph (Full Implementation)

Replace shallow entity mention tracking with real relationship intelligence:

```sql
CREATE TABLE relationships (
    contact_name     TEXT PRIMARY KEY,
    detected_in_apps TEXT,          -- JSON array: ["WhatsApp", "Gmail"]
    typical_greeting TEXT,          -- "hey bhai" / "Dear X" / "Hi"
    typical_closer   TEXT,          -- "Thanks" / "Regards" / "Thx"
    preferred_length TEXT,          -- "short" / "medium" / "long"
    language_mix     TEXT,          -- "hinglish" / "english" / "gujarati_roman"
    formality        REAL,          -- 0-10
    emoji_ok         INTEGER,       -- 0/1
    message_count    INTEGER,
    last_seen        DATETIME
);
```

For Reply mode — if contact known:
```
[Relationship context]: You typically address Rahul with "hey bhai",
keep messages under 3 lines, use Hinglish, no emoji.
Match this exactly.
```

### Day 5: Thread Context for Reply

Extend capture.rs to read conversation context above the selected message.
Use Windows Accessibility API (IUIAutomation) to read parent container text.

Gives Reply mode sight into:
- Last 3-5 messages of the thread
- Who said what
- Emotional arc of the conversation

Reply becomes context-aware not just message-aware.

---

## Week 4 — India-First + UI Overhaul

### Day 1-2: India Language Depth

| Language | What to add |
|----------|-------------|
| Hinglish | Deeper slang patterns, casual vs professional Hinglish distinction |
| Gujarati Roman | Expand "tmne/che/nathi/gamto" patterns — detect + reply natively |
| Hindi Devanagari | Proper reply in Devanagari, not transliterated |
| Indian English | "do the needful", "revert back", "prepone" — understand without rewriting unless asked |
| Indian professional | Leave application format, salary negotiation Indian style, WhatsApp forward patterns |

Add 8 new Indian professional Do mode templates:
- Leave application (formal Hindi or English)
- Salary negotiation message
- Client follow-up (Indian business style)
- WhatsApp business message
- LinkedIn post (Indian startup voice)
- College email to professor
- Landlord/tenant communication
- Escalation email

### Day 3: Communication Analytics Dashboard

Weekly summary shown in Settings:
- Total transforms this week
- Most used mode
- Average tone trend (getting more positive/formal?)
- Top contacts you reply to
- Streak (days in a row used)
- "Your writing is 23% more formal than last week"

Creates stickiness — users open Settings to see their communication patterns.

### Day 4-5: UI/Pill Overhaul

**Main Overlay:**
- ToneMirror text: 9.5px → 12px. Show exact friction phrase highlighted.
- History panel: full-card overlay → right-side slide-in drawer
- Mode bar: context-aware ordering based on detected app + intent
- Settings badge: CL/BK/LO always visible on settings button
- Private mode indicator: lock icon in header when incognito ON

**Pill:**
- Auto-hide: 5s → 8s. Restart 3s on mouse-leave.
- Fix label bug: HTML "Fix" → "Correct" to match data-mode
- Click feedback: spinner → check before pill closes
- Context-aware buttons: app-dependent (WhatsApp ≠ VS Code ≠ Gmail)
- Loading state: pill stays open with pulse dot while generating
- Tooltip on first use: "Quick AI transforms — Ctrl+C twice"

---

## Week 5 — Polish + Hardening

### Day 1-2: Performance

- Target < 300ms from trigger to first token streaming
- Pre-warm HTTP connection pool on app start (currently lazy-init)
- Pre-load voice profile and relationship graph into memory at startup (not per-call DB reads)
- Cache NLP analysis result for 30s — if same text triggered twice, skip recomputation

### Day 3: Security Final Pass

- `cargo audit` — zero known CVEs in dependency tree
- Penetration test the Cloudflare Worker (manual: auth bypass, payload injection, rate limit bypass)
- Verify all API keys never logged (grep codebase for any println! that could expose keys)
- Verify SQLite encryption is correct — open DB file in hex editor and confirm it's not plaintext
- Tauri CSP audit — no unsafe-inline, no wildcard sources
- Binary code signing setup (Tauri supports Windows Authenticode)

### Day 4-5: The "Can't Replicate" Test

Simulate a competitor trying to match SnapText's output for a 90-day user:
- Generic AI (Copilot/Gemini): produces generic text
- SnapText: knows you write to Rahul in Hinglish, keep messages short, start with "hey bhai"
- SnapText: knows this message pattern is similar to a rescheduling request you handled last month
- SnapText: knows you're in WhatsApp so keeps it under 3 lines
- SnapText: output is indistinguishable from you writing it

Document this gap explicitly in marketing.

---

## Security Architecture Summary

```
User types → App captures clipboard → Sensitive data scan
                                              ↓
                                    Sensitive? → Warn user, block history storage
                                              ↓
                                    NLP analysis (local, no network)
                                              ↓
                                    Build prompt (local)
                                              ↓
                         ┌──────────────────────────────────┐
                         │  AI Mode decision                 │
                         │  Local (Ollama) → stays on device │
                         │  Worker → Cloudflare (no raw keys)│
                         │  BYOK → direct to Gemini          │
                         └──────────────────────────────────┘
                                              ↓
                                    Result streams to UI
                                              ↓
                                    Incognito OFF → encrypt + store in SQLite
                                    Incognito ON  → display only, never written
                                              ↓
                                    Zeroize captured text from memory
```

### Key security properties
- **At rest**: SQLite encrypted via DPAPI-derived key (machine + user bound)
- **API keys**: Windows Credential Manager, never in binary or config files
- **In transit**: HTTPS only, Cloudflare handles TLS termination
- **Sensitive data**: auto-detected, blocked from storage, never sent to cloud AI without warning
- **Memory**: captured text zeroed after use via zeroize crate
- **Telemetry**: zero. Verifiable by user via in-app network log
- **Local option**: Ollama mode — zero network calls, fully air-gapped
- **Audit**: every AI call logged by metadata (mode + char count), never content

---

## Execution Timeline

| Week | Days | Deliverable |
|------|------|-------------|
| 1 | 1 | Dead code deleted, NLP simplified |
| 1 | 2-3 | SQLite encrypted, Windows Credential Manager, memory zeroization |
| 1 | 4 | Sensitive data detection, audit log, incognito mode |
| 1 | 5 | Worker hardening, data retention, security review |
| 2 | 1-2 | Real RAG — Gemini embeddings, cosine retrieval, new Reply prompt |
| 2 | 3 | App context detection — context-aware pill per app |
| 2 | 4-5 | Prompt mode domain detection + specificity extraction |
| 3 | 1-2 | Deep voice DNA learning |
| 3 | 3-4 | Full relationship graph |
| 3 | 5 | Thread context for Reply |
| 4 | 1-2 | India language depth + 8 Indian professional templates |
| 4 | 3 | Communication analytics dashboard |
| 4 | 4-5 | UI/Pill overhaul |
| 5 | 1-2 | Performance optimization (< 300ms to first token) |
| 5 | 3 | Security final pass + penetration test |
| 5 | 4-5 | End-to-end quality test — simulate 90-day user experience |

---

## Grade Milestones

| Milestone | Grade | What changed |
|-----------|-------|-------------|
| Today | B+ | Works, cross-app, Hinglish, fast |
| End Week 1 | A- | Clean code, encrypted, secure |
| End Week 2 | A | Real RAG, app context, better prompts |
| End Week 3 | A | Sounds like you, knows your relationships |
| End Week 4 | A+ | India-first depth, polished UI |
| End Week 5 | A+ unbeatable | Performance + security hardened + competitor gap documented |

---

## What Big Players Cannot Replicate (The 2-Year Gap)

| Moat | Why it takes 2+ years to replicate |
|------|-------------------------------------|
| Voice fingerprint | Requires months of your writing history. Can't fast-forward. |
| Relationship graph | Cross-app data they'll never have — WhatsApp + Gmail + Slack combined |
| India-first depth | Not a priority for a global product roadmap |
| Thread context | Architecture decision requiring accessibility API work |
| Data flywheel | Day 1 ≠ Day 180. Gap widens with every use. |
| Local-first security | Big players need your data on their servers. SnapText doesn't. |
| Encrypted + auditable | Enterprise trust they'd need years of compliance work to match |
