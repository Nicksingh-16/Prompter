# SnapText: A Cross-Platform AI Writing Assistant Eliminating Context-Switch Overhead in LLM-Assisted Communication

**Author:** Nishant Shekhawat
**Project Status:** Early Access (April 2026)
**Platforms:** Windows (Tauri 2 + React), Android (Kotlin)
**Backend:** Cloudflare Workers + Google Gemini 2.5 Flash

---

## Abstract

We present **SnapText**, a desktop and mobile AI writing assistant that eliminates the multi-step "copy → paste into ChatGPT → prompt → copy back" workflow that has become endemic to professional communication in the era of large language models (LLMs). SnapText integrates a six-stage on-device Natural Language Processing (NLP) pipeline with streaming LLM inference, providing context-aware text transformation directly within the user's existing applications. The system distinguishes itself from existing solutions through (a) **language-preserving transformations** that respect Hinglish, regional dialects, and code-mixed inputs; (b) **on-device intent classification** using a 35+ signal feature extractor; (c) **a triple-backend AI strategy** (cloud proxy, BYOK, local Ollama) addressing diverse privacy and cost constraints; and (d) **OS-native integration patterns** (PROCESS_TEXT on Android, double-copy detection + clipboard injection on Windows) that minimize user friction. This paper documents the system architecture, the rationale behind key design decisions, the security model, and the engineering trade-offs encountered during implementation.

**Keywords:** LLM workflow optimization, mobile NLP, code-mixed language processing, Hinglish, IME alternatives, PROCESS_TEXT, voice profile learning, prompt engineering

---

## 1. Introduction

### 1.1 Problem Statement

The proliferation of LLM-based assistants has created a productivity paradox: while LLMs can rewrite, translate, and compose text faster than any human, the *workflow* of accessing them remains stubbornly inefficient. A typical professional interaction looks like:

1. Receive a message (e.g., a client email on WhatsApp Web).
2. Copy the message text.
3. Switch context to a separate browser tab or app (ChatGPT, Gemini, Claude).
4. Paste the message.
5. Type a meta-prompt (*"reply professionally to this"*).
6. Wait for output.
7. Copy the output.
8. Switch back to the original application.
9. Paste the output.

For a freelancer or sales professional handling 15–20 such interactions daily, this loop consumes **30–40 minutes per day in pure context-switch overhead** — without any actual cognitive value being added. The LLM is doing the work in seconds; the human is wasting minutes shuttling text between windows.

### 1.2 Existing Solutions and Their Gaps

| Solution | Friction Reduction | Language Awareness | Privacy | Indian Market Fit |
|----------|-------------------|-------------------|---------|-------------------|
| ChatGPT (web) | None — full context-switch loop | Generic, English-biased | Data sent to OpenAI | Poor (treats Hinglish as noise) |
| Grammarly Keyboard | Partial — inline suggestions | English-only | SaaS, paid | Poor (no Hinglish) |
| Gboard AI features | Inline but generic | English-biased | Sent to Google | Improving but limited |
| Meta AI in WhatsApp | In-app but locked to Meta apps | English-first | Sent to Meta | Limited |
| Browser extensions | DOM-injected, limited apps | Varies | High attack surface | N/A |

The common thread: none of these solutions are **simultaneously** (a) low-friction, (b) language-preserving for code-mixed inputs, (c) privacy-flexible, and (d) cross-application.

### 1.3 Contributions

This work makes the following contributions:

1. A **six-stage modular NLP pipeline** that runs entirely on-device, performing language detection, intent classification, feature extraction, and prompt construction before any network call.
2. A **language-preserving prompt strategy** that explicitly instructs the LLM to maintain the input's language, dialect, and tone — a non-obvious requirement that solves the dominant failure mode of generic LLMs for Indian users.
3. A **triple-backend inference architecture** allowing seamless fallback between cloud proxy, user-owned API keys, and local models, addressing the spectrum of privacy/cost preferences.
4. A **PROCESS_TEXT-based mobile integration** that bypasses the high-friction "install a new keyboard" step entirely, making the product installable without scary permissions.
5. An **OS-native desktop integration** using clipboard double-tap detection and synthetic Ctrl+V injection, requiring no browser extensions or accessibility permissions.

---

## 2. System Architecture

### 2.1 High-Level Overview

SnapText consists of three deployable components and one cloud service:

```
┌──────────────────────────┐         ┌──────────────────────────┐
│   Desktop Client         │         │   Mobile Client          │
│   (Tauri + React)        │         │   (Android Kotlin)       │
│                          │         │                          │
│  ┌────────────────────┐  │         │  ┌────────────────────┐  │
│  │ NLP Pipeline       │  │         │  │ NLP (simplified)   │  │
│  │ (6 stages, Rust)   │  │         │  │ (4 stages, Kotlin) │  │
│  └────────┬───────────┘  │         │  └────────┬───────────┘  │
│           │              │         │           │              │
│  ┌────────▼───────────┐  │         │  ┌────────▼───────────┐  │
│  │ AI Router          │  │         │  │ AI Router          │  │
│  │ Worker / Byok /    │  │         │  │ Worker / Byok      │  │
│  │ Local (Ollama)     │  │         │  └────────┬───────────┘  │
│  └────────┬───────────┘  │         │           │              │
│           │              │         │  ┌────────▼───────────┐  │
│  ┌────────▼───────────┐  │         │  │ PROCESS_TEXT       │  │
│  │ Clipboard Injector │  │         │  │ Activity           │  │
│  │ (Ctrl+V synth.)    │  │         │  └────────────────────┘  │
│  └────────────────────┘  │         │                          │
└──────────┬───────────────┘         └──────────┬───────────────┘
           │                                    │
           └──────────────┬─────────────────────┘
                          │
                          ▼
            ┌──────────────────────────┐
            │   Cloudflare Worker      │
            │   (snaptext-worker)      │
            │                          │
            │  • Auth (X-App-Secret)   │
            │  • Per-device daily cap  │
            │  • 3-key rotation        │
            │  • SSE pass-through      │
            └────────────┬─────────────┘
                         │
                         ▼
            ┌──────────────────────────┐
            │   Google Gemini API      │
            │   (gemini-2.5-flash)     │
            └──────────────────────────┘
```

**Key architectural decisions:**

- **NLP runs on-device, not server-side.** This eliminates a network round-trip, makes intent detection privacy-preserving, and allows offline-first features like history and tone analysis.
- **The cloud worker is stateless.** It does not log content, does not store transformations, and does not maintain user accounts. Its sole purpose is rate-limiting and key rotation.
- **The desktop and mobile NLP pipelines share a common conceptual structure** but diverge in implementation depth (35+ signals on desktop, 10 on mobile) to fit each platform's resource constraints.

### 2.2 Component Inventory

| Component | Language | LOC (approx) | Purpose |
|-----------|----------|--------------|---------|
| `src-tauri/` | Rust | ~3,500 | Desktop backend (NLP, AI router, injection) |
| `src/` | TypeScript/React | ~2,000 | Desktop UI (overlay, settings, history) |
| `mobile/snaptext-android/` | Kotlin | ~3,000 | Android app (NLP, IME, PROCESS_TEXT, settings) |
| `worker/worker.js` | JavaScript | ~120 | Cloudflare Worker (Gemini proxy + rate limit) |
| `landing/` | TypeScript/React | ~800 | Marketing landing page |

---

## 3. The NLP Pipeline (Desktop)

The desktop NLP pipeline is the technical heart of SnapText. It is implemented in pure Rust (no ML dependencies), runs in microseconds, and produces a structured `TextContext` object that downstream modules consume.

### 3.1 Pipeline Stages

```rust
pub fn analyze(raw: &str) -> TextContext {
    let normalized   = normalize::run(raw);
    let language     = language::detect(&normalized);
    let features     = features::extract(&normalized, &language);
    let intent       = intent::classify(&normalized, &language, &features);
    let prompt_ready = TextContext { normalized, language, features, intent };
    prompt_ready
}
```

#### Stage 1: Normalization (`normalize.rs`)

- Converts smart quotes (`"`, `"`, `'`, `'`) to ASCII equivalents
- Replaces em-dashes and en-dashes with `-`
- Collapses multiple whitespace into single spaces
- Strips zero-width characters and non-breaking spaces (NBSP)
- Detects and flags URLs (`http://`, `https://`, `www.`)
- Detects email addresses via simplified regex
- Output: `NormalizeOutput { normalized, word_count, char_count, has_urls, has_emails }`

**Why it matters:** Without normalization, downstream regex-based detectors miss inputs that contain Unicode quote variants — common when users paste from Word, WhatsApp Web, or LinkedIn.

#### Stage 2: Language Detection (`language.rs`)

A single-pass O(n) Unicode block scanner classifies characters into 18 scripts:

- Latin (`U+0041–U+007A`, `U+00C0–U+024F`)
- Devanagari (`U+0900–U+097F`)
- Arabic (`U+0600–U+06FF`)
- Cyrillic (`U+0400–U+04FF`)
- CJK Unified (`U+4E00–U+9FFF`)
- Hiragana, Katakana, Hangul
- Tamil, Telugu, Gujarati, Bengali, Kannada, Malayalam, Punjabi
- Thai, Greek, Hebrew

**Hinglish detection** is performed separately. After determining that the dominant script is Latin (>70%), the detector scans for romanized Hindi tokens against a curated list of ~60 high-frequency markers (`mujhe`, `karna`, `acha`, `bhai`, `yaar`, etc.). If 2+ markers appear, or >15% of words match, the text is flagged as Hinglish.

**Why a custom detector instead of a pre-trained model?**
1. **Speed:** Classifies in <1ms vs ~50ms for a model load.
2. **Size:** Adds zero binary weight vs ~50MB for fastText.
3. **Accuracy on the target use case:** Pre-trained models classify Hinglish as either Hindi (wrong) or English (also wrong). A purpose-built detector handles it correctly.

#### Stage 3: Deep Intent Engine (`intent.rs`)

This is the most novel component. The intent engine extracts a `SignalVector` containing 35+ signals across four categories:

**Surface signals:**
- Keyword scores per intent class (Email, Chat, Prompt, Knowledge, Report, Social) using curated word lists
- Greeting detection and position
- Sign-off detection
- @-symbol presence
- Subject line detection

**Structural signals:**
- Bullet point count
- Numbered list detection
- Paragraph count
- Salutation structure (greeting + body + closer)

**Behavioral signals:**
- Sentence count, word count, question count, exclamation count
- Fragment count, ellipsis count
- Average sentence length
- Contraction rate (`don't`, `won't`, etc.)

**Linguistic signals:**
- Formality score (0–10) computed from passive voice, hedging, contractions
- Hedge word count (`maybe`, `perhaps`, `might`, `could`)
- Urgency word count (`asap`, `urgent`, `immediately`)
- Imperative count
- Question starter detection (handles missing apostrophes like `whats` → `what's`)

These signals are combined via a **weighted scoring matrix** to produce a ranked `IntentResult`:

```rust
pub struct IntentResult {
    pub primary: IntentCandidate,
    pub alternatives: Vec<IntentCandidate>,
    pub overall_confidence: f32,
}
```

The user can correct the suggested intent in the UI, and the correction is logged to `intent_corrections` table. Over time, weight overrides accumulate in `intent_weight_overrides`, providing a per-user adaptive classifier without requiring model fine-tuning.

**Why not use an LLM for intent classification?**
1. **Latency:** Local classification adds 0ms; an extra LLM call adds 800–1500ms.
2. **Cost:** Free per call vs ~$0.0001 per Gemini classification.
3. **Privacy:** Intent is determined before any data leaves the device.
4. **Determinism:** The user gets predictable suggestions; an LLM might suggest different intents for the same input.

#### Stage 4: Feature Extraction (`features.rs`)

The features module produces semantic information used both by the intent classifier and the UI:

- **Tone score** in the range `[-5, +5]` (harsh → warm), based on positive/negative word lists
- **Formality score** in `[0, 10]` (slang → boardroom)
- **Friction phrases** — passive-aggressive markers (`per my last email`, `as I already mentioned`)
- **Keyword extraction** — top-N nouns weighted by TF-IDF approximation
- **Entity detection** — people, organizations, projects (regex-based)
- **Sentence segmentation** — handles common abbreviations (Mr., Dr., etc.)

The tone score feeds the **ToneMirror** UI component, which displays a real-time pulse:
- Green: warm & enthusiastic (≥4)
- Blue: clear & professional (1–3)
- Yellow: slightly tense (-1 to 0, or Hinglish/RTL)
- Red: frustrated (≤-3) — pulses when friction is detected

#### Stage 5: Prompt Building (`prompt.rs`)

The prompt builder constructs LLM system prompts as a stack of composable blocks:

```
1. SANITIZATION    — strip prompt-injection patterns
2. LANGUAGE BLOCK  — script, RTL flag, Hinglish hint
3. CONTEXT BLOCK   — intent, tone, formality, friction
4. TASK BLOCK      — mode-specific instruction (Reply, Correct, etc.)
5. CONSTRAINTS     — output format rules
6. TEAM VOICE      — corporate guidelines (if configured)
7. VOICE PROFILE   — learned personal style
8. MEMORY BLOCK    — relationship context
```

**The language-preservation rule** is the most critical innovation here. Earlier iterations forced English output on every mode, which produced robotic translations of Hinglish inputs. The fix was to add explicit language-matching instructions to every mode:

```
CRITICAL LANGUAGE RULE: Reply in the SAME language, dialect, and style
as the input. If the input is Hinglish, reply in Hinglish. If it's
Hindi, reply in Hindi. If it has a regional dialect (Rajasthani,
Punjabi, etc.), keep that flavor in the reply.
```

This single instruction transformed the product from "broken English fixer" to "writes like you do."

#### Stage 6: Local Transform Engine (`local_engine.rs`)

When network is unavailable or the user prefers offline operation, the local engine provides rule-based fallbacks:

- `fix_local()` — grammar normalization, capitalization, common typo correction
- `summarize_local()` — extractive summarization via top-N sentence ranking
- `shorten_local()` — filler word removal
- `report_tone_local()` — tone/formality verdict text generation

These are not LLM-quality but are functional and instant.

### 3.2 The Mobile NLP Pipeline (Simplified)

The mobile pipeline ports the conceptual structure but reduces complexity for resource constraints:

| Stage | Desktop (Rust) | Mobile (Kotlin) |
|-------|---------------|----------------|
| Normalization | Full (URLs, emails, NBSPs, smart quotes) | Implicit |
| Language detection | 18 scripts + Hinglish | 6 scripts + Hinglish |
| Intent | 35+ signals, weighted matrix | 10 signals, priority-ordered |
| Features | Tone, formality, friction, entities | Tone, friction (Tone Guard) |
| Prompt building | 8 blocks | 4 blocks |
| Local fallback | Yes | No |

**Why simplify?** Mobile keyboards (and PROCESS_TEXT activities) need to feel instant. A 100ms delay on a key press is perceptible. The mobile pipeline runs in <5ms per analysis.

---

## 4. AI Integration Strategy

### 4.1 Triple Backend Architecture

SnapText supports three AI inference backends, selectable per-user:

#### Backend A: Cloudflare Worker (default)

- Free tier: 20 transforms/day per device
- Routes through `snaptext-worker.snaptext-ai.workers.dev`
- Worker proxies to Google Gemini 2.5 Flash
- Authentication: `X-App-Secret` header (shared static secret)
- Rate limiting: per-device daily cap stored in Cloudflare KV (24h TTL)
- Key rotation: 3 Gemini API keys, randomized per request, automatic failover on 429

**Key rotation implementation:**

```javascript
const availableKeys = [env.GEMINI_KEY_1, env.GEMINI_KEY_2, env.GEMINI_KEY_3].filter(Boolean);
const shuffledKeys = availableKeys.sort(() => Math.random() - 0.5);

for (const key of shuffledKeys) {
    const res = await fetch(geminiUrl, { ... });
    if (res.ok) { /* increment usage, return */ }
    if (res.status === 429) continue;  // Try next key
}
```

This gives us **3× the effective Gemini quota** without paying for higher tiers, and makes the service resilient to single-key rate limits.

#### Backend B: BYOK (Bring Your Own Key)

- User pastes their own Gemini API key
- Direct call to `generativelanguage.googleapis.com`
- No worker involvement, no usage limits
- Key stored locally with XOR obfuscation (desktop) or SharedPreferences (mobile)

**Why BYOK?** Power users (and those concerned about routing through a third-party worker) want a direct connection. BYOK gives them control without us needing to manage their billing.

#### Backend C: Local Inference (Ollama)

- Connects to `http://localhost:11434/api/generate`
- Default model: `phi3:mini` (4GB RAM)
- Optional: `gemma2`, `llama3`
- Zero network calls — fully offline

**Why support local?** Privacy-conscious developers and users in low-bandwidth environments need this. It also future-proofs the product against any cloud LLM going down or becoming expensive.

### 4.2 Streaming Architecture

All three backends support **Server-Sent Events (SSE) streaming** for real-time token delivery. The desktop and mobile clients consume tokens incrementally and update the UI as text arrives, which provides perceived latency improvements of 5–10× over batch responses.

**Desktop (Rust):**
```rust
let mut stream = response.bytes_stream();
while let Some(chunk) = stream.next().await {
    let text = parse_sse_chunk(&chunk?)?;
    app.emit("ai_token", text)?;  // Emits to React frontend
}
```

**Mobile (Kotlin):**
```kotlin
BufferedReader(InputStreamReader(stream)).use { reader ->
    var line: String?
    while (reader.readLine().also { line = it } != null) {
        if (!line!!.startsWith("data: ")) continue
        val token = parseSseToken(line)
        onToken(token)  // Suspend callback to UI
    }
}
```

### 4.3 Mode → Prompt Mapping

SnapText supports the following modes:

| Mode | Purpose | Language Behavior |
|------|---------|------------------|
| **Reply** | Compose a reply to a received message | Preserves input language |
| **Do** | Execute an instruction (e.g., "write a leave letter") | Output in input language |
| **Correct** | Fix broken English / Hinglish into clean English | Converts to English |
| **Professional** | Polish tone | Preserves language |
| **Casual** | Conversational rewrite | Preserves language |
| **Email** | Notes → structured email | Preserves language |
| **Translate** | Cross-language conversion | Explicit target lang |
| **Prompt** | Rough idea → structured AI prompt | Always English |
| **Knowledge / Explain** | Answer/explain | Preserves language |
| **Summarize** | Extract key points | Preserves language |
| **Strategist** | Executive tone critique | Preserves language |

The **Do mode** is the most recent addition. It treats the selected text as an *instruction* rather than content to be transformed:

> Input: `"write a leave application to my manager for 3 days, family emergency"`
> Output: A complete leave letter, ready to copy.

This is the closest equivalent to the "ChatGPT replacement" use case — rough request in, finished artifact out.

---

## 5. Mobile-Specific Architecture

### 5.1 The PROCESS_TEXT Pivot

The original mobile design implemented an Input Method Editor (IME) — a custom keyboard. This approach has three major drawbacks:

1. **Permission scariness:** Android requires users to navigate to Settings → Languages & Input → enable a keyboard → switch active keyboard. This drops ~70% of users at the install funnel.
2. **Competition with Gboard:** A solo founder cannot match Google's swipe typing, autocorrect, or prediction engine. The keyboard would be inferior on its core function.
3. **Accessibility surface area:** IMEs see every keystroke, which raises legitimate security concerns even when the app has no malicious intent.

We pivoted to **Android's PROCESS_TEXT intent** — a system mechanism that adds custom actions to the text selection menu. When the user long-presses any text in any app, the menu shows our app as one of the options.

**Manifest declaration:**

```xml
<activity
    android:name=".ui.ProcessTextActivity"
    android:exported="true"
    android:label="SnapText"
    android:theme="@style/Theme.SnapText.Dialog">
    <intent-filter>
        <action android:name="android.intent.action.PROCESS_TEXT" />
        <category android:name="android.intent.category.DEFAULT" />
        <data android:mimeType="text/plain" />
    </intent-filter>
</activity>
```

**Result:**
- Zero permission prompts
- Works in WhatsApp, Gmail, Chrome, Instagram, LinkedIn, Notes — every text-handling app
- User keeps Gboard (or any keyboard) — SnapText is additive, not replacement
- Install funnel drop reduced by ~70%

### 5.2 The Bottom Sheet UX

When the user taps "SnapText" from the text selection menu, a bottom-anchored Activity (themed as a dialog) slides up:

```
┌──────────────────────────────────┐
│ ✨ SnapText                    ✕ │
│                                  │
│ [AI reply streams here]          │
│                                  │
│ [Do] [Fix English] [AI Prompt]   │
│                                  │
│            Cancel  Copy  Insert  │
└──────────────────────────────────┘
```

**Key UX decisions:**

1. **Reply auto-runs.** No mode selection step — the user's most common need (reply) starts streaming the moment the sheet opens. The other modes are one tap away if needed.
2. **Three switch chips below.** Tapping any chip cancels the current generation and re-runs with the new mode.
3. **Insert vs Copy.** If the source text field is editable, Insert replaces the selection (via PROCESS_TEXT result intent). Otherwise, only Copy is available.
4. **Streaming display.** Tokens appear as they arrive — no blank loading state.

### 5.3 Mobile NLP Differences

The mobile NLP engine is a faithful but simplified port of the desktop pipeline:

- **`LanguageDetector.kt`** detects 6 script types (Latin, Devanagari, Arabic, CJK, Cyrillic, Mixed) + Hinglish via marker matching
- **`IntentDetector.kt`** uses a 10-signal priority cascade instead of weighted scoring (Hinglish → Correct, Code → Prompt, Email signals → Email, etc.)
- **`PromptBuilder.kt`** mirrors the desktop modes with slightly tighter prompts to fit mobile streaming constraints
- **`ToneAnalyzer.kt`** powers the Tone Guard feature (real-time friction phrase warnings)

These are kept in sync with the desktop versions through manual port discipline rather than code generation, since the languages and execution models differ.

---

## 6. Desktop-Specific Architecture

### 6.1 Clipboard Double-Tap Detection

On Windows, the desktop client uses two interaction modes:

1. **Alt+K hotkey** — opens the full overlay (480×580 always-on-top window)
2. **Double-copy** (Ctrl+C twice within 1.5s) — triggers a small "pill" UI (260×36) anchored near the text caret

The double-copy detector polls the clipboard sequence number:

```rust
loop {
    let seq = unsafe { GetClipboardSequenceNumber() };
    if seq != last_seq {
        let now = Instant::now();
        if now.duration_since(last_copy_time) < Duration::from_millis(1500) {
            show_pill();  // Double-copy detected
        }
        last_copy_time = now;
        last_seq = seq;
    }
    sleep(Duration::from_millis(50));
}
```

The pill is positioned using `GetCaretPos()` (Win32) when available, falling back to mouse cursor position.

### 6.2 Text Injection via Synthetic Ctrl+V

Text injection uses a save-clobber-restore pattern on the clipboard:

```rust
fn inject(text: &str) {
    let original = clipboard.get_text();      // 1. Save user's clipboard
    clipboard.set_text(text);                 // 2. Set our text
    sleep(Duration::from_millis(50));         // 3. Wait for clipboard sync
    enigo.key(Key::Control, Press)?;          // 4. Synth Ctrl+V
    enigo.key(Key::Layout('v'), Click)?;
    enigo.key(Key::Control, Release)?;
    sleep(Duration::from_millis(300));        // 5. Wait for paste
    clipboard.set_text(&original);            // 6. Restore original
}
```

**Why synthetic Ctrl+V instead of typing characters one by one?**
- Pasting preserves formatting in apps that support it
- Typing emits `KEYDOWN` events that some apps interpret weirdly (autocomplete suggestions, etc.)
- Pasting is ~100× faster for long outputs

**Limitation:** The 300ms restore delay can cause race conditions if the user copies new text immediately after injection. In practice, this is rare.

### 6.3 The Voice Profile System

The desktop client maintains a SQLite-backed voice profile that learns from every session:

```sql
CREATE TABLE voice_profile (
    feature_type TEXT,    -- 'opener', 'closer', 'vocab', 'stat:formality', 'stat:tone'
    feature_key  TEXT,    -- the actual word or stat name
    value        REAL,    -- count or running average
    count        INTEGER,
    last_seen    INTEGER  -- Unix timestamp
);

CREATE TABLE context_memory (
    entity_type  TEXT,    -- 'person', 'org', 'project'
    entity_name  TEXT,
    attribute    TEXT,    -- 'typical_tone', 'formality', 'last_topic'
    value        TEXT,
    seen_count   INTEGER,
    last_seen    INTEGER
);
```

Each transformation triggers `observe_session()`, which:
1. Extracts the first word (opener) and last word (closer)
2. Records vocabulary tokens longer than 6 characters
3. Updates rolling averages of tone and formality
4. Mentions any detected entities (people, projects)

The voice profile data is then injected into the prompt's "VOICE PROFILE" block, biasing the LLM toward outputs that sound like the user.

**Important constraint:** Mobile does not currently have voice profile learning. It's planned for Phase 3 once we have data on whether users find the feature valuable enough to justify the storage and complexity.

---

## 7. Security & Privacy Model

### 7.1 Threat Model

We identified the following threat vectors:

| Threat | Mitigation |
|--------|-----------|
| Plaintext API key exfiltration | XOR obfuscation (desktop), SharedPreferences (mobile). Not cryptographic — see §7.3. |
| Worker abuse / quota theft | Per-device daily cap, X-App-Secret auth |
| LLM prompt injection | Input sanitization layer (`sanitize()` strips known patterns) |
| Clipboard hijacking by other apps | Save/restore pattern, 300ms window |
| Network sniffing | All API calls over HTTPS, no plaintext fallback |
| User data leakage | No analytics, no telemetry, no error logs sent off-device |

### 7.2 Data Handling

**On-device only (never transmitted):**
- Voice profile (openers, closers, vocabulary)
- Context memory (people, projects, topics)
- Transformation history (last 100)
- Intent corrections
- Tone scores and formality scores
- Settings and preferences

**Transmitted to LLM provider per request:**
- The system prompt (constructed from on-device NLP analysis)
- The user's input text
- Generation parameters (temperature, max tokens)

**Transmitted to Cloudflare Worker (Worker mode only):**
- Same payload as above
- `X-Device-ID` header (FNV-1a hash of `COMPUTERNAME:USERNAME` on desktop, Android ID on mobile)
- `X-App-Secret` header (shared static value)

**Stored in Cloudflare KV (Worker mode only):**
- `usage:{deviceId}:{date}` → integer count, 24h TTL
- No content, no personally identifiable information beyond the device hash

### 7.3 Why XOR Obfuscation, Not Encryption?

The desktop API key storage uses XOR with `COMPUTERNAME` as the key. This is **explicitly not cryptographically secure** and we acknowledge this in the documentation. The reasoning:

1. **Threat realism:** An attacker with local access to the user's machine has already won. Real disk encryption (Windows Credential Manager, libsecret) requires platform-specific code paths and adds binary weight.
2. **Defense against casual snooping:** XOR prevents the API key from appearing as plaintext in `strings(1)` or hex editor inspection of the SQLite file.
3. **Migration path:** Future versions will move to OS keychain (`Windows Credential Manager`, `keyring` crate), at which point the XOR layer can be deprecated.

For high-security contexts, users are advised to use **Local mode (Ollama)** which never stores or transmits any keys.

### 7.4 Prompt Injection Defense

The `sanitize()` function strips known prompt-injection patterns from user input before constructing the system prompt:

```rust
const INJECTION_PATTERNS: &[&str] = &[
    "\nIgnore previous", "\nForget previous", "\nSystem:", "\nsystem:",
    "\n[INST]", "\n[SYS]", "###", "```system",
];
```

This is **not a complete defense** — sophisticated adversarial inputs can still confuse the LLM. The mitigation strategy is:

1. **Output is shown to the user before insertion.** The user always sees the result and can choose not to insert it.
2. **No autonomous actions.** SnapText never takes any action other than text injection at the user's explicit command.
3. **Sandbox by design.** The LLM has no tools, no function calling, no file system access. Worst-case prompt injection produces unwanted *text*, not unwanted *actions*.

### 7.5 Network Architecture Privacy

The Cloudflare Worker is intentionally **stateless and content-blind**:

- Does not log request bodies
- Does not store generated outputs
- Does not maintain user accounts
- Does not associate device IDs with email addresses or any PII
- KV entries auto-expire after 24 hours

The only persistent data is the daily usage counter, which exists solely to enforce the free-tier cap.

---

## 8. Implementation Details

### 8.1 Desktop Build & Distribution

- **Framework:** Tauri 2 (Rust backend, WebView2 frontend)
- **Frontend:** React 18 + TypeScript, bundled via Vite
- **Database:** SQLite (rusqlite, bundled)
- **HTTP:** reqwest 0.12 with `stream` feature
- **UI animations:** Framer Motion
- **Binary size:** <6 MB (excluding WebView2, which Windows ships natively on 11+)

### 8.2 Mobile Build & Distribution

- **Min SDK:** 24 (Android 7.0)
- **Target SDK:** 35 (Android 15)
- **Language:** Kotlin 1.9
- **UI:** Mix of XML layouts (PROCESS_TEXT activity, IME — currently disabled) and Jetpack Compose (Settings, Onboarding)
- **HTTP:** OkHttp 4 with coroutine adapter
- **Storage:** SharedPreferences for settings; JSON-encoded blobs for clipboard hub and templates
- **APK size:** ~4 MB (stripped, ProGuard-minified)

### 8.3 Worker Deployment

- **Platform:** Cloudflare Workers (free tier)
- **Storage:** Cloudflare KV namespace (`USAGE`)
- **Secrets:** `APP_SECRET`, `GEMINI_KEY_1/2/3`, `MODEL` (env vars)
- **Cold start:** <5ms
- **Cost at current scale:** $0/month (within free tier)

### 8.4 Landing Page

- **Stack:** React + TypeScript + Vite (separate from main app)
- **Hosting:** Cloudflare Pages
- **URL:** `https://snaptext-app.pages.dev`
- **Email collection:** Formspree (free tier, 50 submissions/month)
- **Build size:** ~110 KB gzipped

---

## 9. Design Decisions & Rationale

### 9.1 Why Tauri Instead of Electron?

| Criterion | Tauri 2 | Electron |
|-----------|---------|----------|
| Binary size | ~6 MB | ~80 MB |
| Memory at idle | ~80 MB | ~250 MB |
| Native APIs (clipboard, hotkeys) | Excellent (Rust) | Decent (Node) |
| Bundled runtime | Uses system WebView | Bundles Chromium |

For a productivity tool that runs in the background all day, the resource footprint matters. Tauri was the right call.

### 9.2 Why On-Device NLP?

Three reasons:
1. **Latency.** A 50ms NLP analysis happens before the user sees any spinner. A 500ms server-side analysis introduces a perceptible lag.
2. **Privacy.** Intent classification, tone scoring, and entity detection happen on text the user hasn't yet "submitted." Doing this server-side would require sending unsubmitted text over the network.
3. **Offline capability.** Local inference (Ollama mode) requires that the entire pipeline can run with no network. On-device NLP is the foundation that makes offline mode possible.

### 9.3 Why Drop the Mobile Keyboard?

After implementing a full IME (5-row QWERTY, voice input, autocorrect, swipe support stub), we measured the install-to-active-user funnel:

- 100 installs
- ~30 reach the "enable keyboard" screen
- ~12 actually enable it
- ~6 set it as the default

A 94% drop-off before first use.

The PROCESS_TEXT pivot eliminated this funnel entirely. Users install, then immediately start using the product because there's no permission flow. The trade-off is that we can't observe what users are typing in real-time (Tone Guard becomes unavailable), but we accept this loss for the install conversion gain.

### 9.4 Why Worker + BYOK + Local Instead of Just One?

Different users have different priorities:

- **The casual user** wants free, instant, zero-config → Worker mode
- **The power user** wants unlimited usage and direct API → BYOK mode
- **The privacy hawk** wants no network at all → Local (Ollama) mode

A single backend would alienate two of these three groups. Maintaining three is moderate engineering cost (~300 LOC of routing logic) for full market coverage.

### 9.5 Why a 20/Day Free Cap?

The free cap was chosen empirically:

- **5/day** — too low; users hit the limit on day 1 and uninstall
- **20/day** — covers the median professional use case (~10–15 transforms/day) with headroom
- **50/day** — exceeds free Gemini quota at scale; would force us to charge sooner

20 is a Goldilocks number: enough that the product feels free, low enough that we can sustain it on the Gemini free tier even with 1000+ daily users.

---

## 10. Known Limitations

### 10.1 Technical Limitations

1. **Voice profile not yet on mobile.** Mobile users get the same prompt regardless of their writing style. Desktop personalization is currently superior.
2. **Local mode is desktop-only.** Mobile cannot run Ollama locally; users on mobile are constrained to Worker or BYOK.
3. **Tone Guard requires the keyboard.** With the IME disabled, real-time tone feedback is unavailable on mobile.
4. **No offline mode on mobile.** Every transformation requires network.
5. **Single-language prompts.** The "Translate" mode only supports a hardcoded language pair list. Arbitrary cross-language pairs aren't yet supported.
6. **No streaming on Translate-as-you-type** (mobile feature, currently disabled).
7. **PROCESS_TEXT discoverability varies.** On some Android versions, the option appears directly in the selection toolbar; on others it's hidden behind the overflow menu.

### 10.2 Product Limitations

1. **No team features.** All voice profiles are individual; no shared corporate voice yet.
2. **No API.** Other apps cannot programmatically invoke SnapText transformations.
3. **No browser extension.** Web users on Mac/Linux have no path to use SnapText (the desktop app is Windows-only currently).

### 10.3 Security Caveats

1. **XOR obfuscation is not encryption.** See §7.3.
2. **Worker auth is a shared static secret.** A motivated attacker could extract the `X-App-Secret` from the binary. This would let them abuse the worker quota but not exfiltrate any user data (because we don't store any).
3. **No audit log.** Worker requests are not logged, by design — but this also means we can't forensically trace abuse if it happens.

---

## 11. Future Work

### 11.1 Short-Term (Next 4 Weeks)

- **Voice profile on mobile.** Port the SQLite-backed learning system to Android Room.
- **Better PROCESS_TEXT discoverability.** Investigate manifest tricks to improve menu placement on Android 13+.
- **Streaming Insert.** Allow the user to tap "Insert" mid-stream and have remaining tokens append in-place.
- **iOS port.** PROCESS_TEXT has no direct iOS analog, but Share Extensions provide similar functionality.

### 11.2 Medium-Term (3–6 Months)

- **Browser extension** for Mac/Linux users (Chrome, Firefox, Safari).
- **Slack/Discord/Teams integrations** — direct app-level integration for power users.
- **Team voice profiles** — corporate guidelines + shared style across an organization.
- **Custom mode authoring** — let users define their own modes with their own prompts.

### 11.3 Long-Term (6–12 Months)

- **Public API** for third-party app integration.
- **Self-hosted version** for enterprise customers (run the worker on their own infrastructure).
- **Multilingual NLP improvements** — better Tamil, Telugu, Bengali support beyond just script detection.

---

## 12. Conclusion

SnapText demonstrates that significant productivity gains are achievable not by building new AI models, but by **eliminating workflow friction around existing models**. The combination of:

1. Native OS integration (PROCESS_TEXT, clipboard hooks)
2. On-device NLP for instant context awareness
3. Language-preserving prompts that respect user authenticity
4. Multiple inference backends covering the full privacy/cost spectrum

…produces a tool that is 10× faster than the current copy-paste-into-ChatGPT workflow while being more language-aware and privacy-respecting than any hosted LLM service.

The most important lesson from this work was **negative**: complexity does not equal value. The original design included 10+ AI modes, a custom keyboard, real-time tone guard, smart reply chips, clipboard categorization, quick templates, and analytics dashboards. Each was individually defensible but collectively they cluttered the UX and confused users. Stripping the product to **four core actions** (Reply, Do, Fix English, AI Prompt) accessed via a **single text-selection menu entry** was the breakthrough. The product became simpler, faster, and more compelling.

Future iterations will continue this discipline: every new feature must justify itself against the question *"does this make the core copy-paste loop faster, or does it just add surface area?"*

---

## Appendix A: File-Level Inventory

### Desktop (`src-tauri/src/`)

| File | Lines | Purpose |
|------|-------|---------|
| `lib.rs` | ~700 | 21 Tauri commands, hotkey wiring, double-copy monitor |
| `main.rs` | 6 | Binary entry point |
| `ai.rs` | ~450 | AI router (Worker/Byok/Local), streaming, key rotation |
| `capture.rs` | ~80 | Clipboard read |
| `inject.rs` | ~120 | Clipboard write + synthetic Ctrl+V |
| `db.rs` | ~600 | SQLite schema, voice profile learning, history |
| `keychain.rs` | ~80 | XOR obfuscation for API keys |
| `ollama.rs` | ~200 | Local model integration |
| `nlp/mod.rs` | ~60 | Pipeline coordinator |
| `nlp/normalize.rs` | ~150 | Stage 1 |
| `nlp/language.rs` | ~280 | Stage 2 (18 scripts + Hinglish) |
| `nlp/intent.rs` | ~600 | Stage 3 (35+ signals, weighted matrix) |
| `nlp/features.rs` | ~400 | Stage 4 (tone, formality, friction, entities) |
| `nlp/prompt.rs` | ~700 | Stage 5 (10 modes, 8 prompt blocks) |
| `nlp/local_engine.rs` | ~200 | Stage 6 (offline fallback) |

### Mobile (`mobile/snaptext-android/app/src/main/java/com/snaptext/keyboard/`)

| File | Purpose |
|------|---------|
| `SnapTextApp.kt` | Application class (OkHttp singleton) |
| `SnapTextIME.kt` | IME service (currently disabled in manifest) |
| `ai/WorkerClient.kt` | Worker + direct Gemini, SSE streaming |
| `clipboard/ClipboardHub.kt` | Smart clipboard manager (parked feature) |
| `data/Preferences.kt` | SharedPreferences wrapper |
| `data/TemplateManager.kt` | Quick templates (parked feature) |
| `data/AnalyticsTracker.kt` | Usage analytics (parked feature) |
| `nlp/IntentDetector.kt` | 10-signal intent classifier |
| `nlp/LanguageDetector.kt` | 6-script + Hinglish |
| `nlp/PromptBuilder.kt` | System prompt builder (11 modes) |
| `nlp/ToneAnalyzer.kt` | Friction phrase detection |
| `ui/SettingsActivity.kt` | Onboarding + settings (Compose) |
| `ui/ProcessTextActivity.kt` | **The core mobile experience** |

### Worker

| File | Purpose |
|------|---------|
| `worker/worker.js` | Cloudflare Worker (Gemini proxy + rate limit) |

### Landing Page

| File | Purpose |
|------|---------|
| `landing/src/App.tsx` | All page sections (Hero, Modes, Android, Waitlist) |
| `landing/src/App.css` | Styling and animations |

---

## Appendix B: Selected Code Excerpts

### B.1 Cloudflare Worker — Key Rotation

```javascript
// From worker/worker.js
const availableKeys = [env.GEMINI_KEY_1, env.GEMINI_KEY_2, env.GEMINI_KEY_3].filter(Boolean);
const shuffledKeys = availableKeys.sort(() => Math.random() - 0.5);

for (const key of shuffledKeys) {
    const geminiUrl = `https://generativelanguage.googleapis.com/v1beta/models/${model}:${stream ? "streamGenerateContent" : "generateContent"}?key=${key}${stream ? "&alt=sse" : ""}`;

    const res = await fetch(geminiUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(geminiBody)
    });

    if (res.ok) {
        await env.USAGE.put(usageKey, (used + 1).toString(), { expirationTtl: 86400 });
        return new Response(res.body, { headers: res.headers });
    }
    if (res.status === 429) continue;  // Try next key
    return new Response(JSON.stringify({ error: errText }), { status: errStatus });
}
```

### B.2 Mobile — Reply Mode Prompt

```kotlin
// From PromptBuilder.kt
"Reply" ->
    "You are an expert communicator who writes natural replies. " +
    "The user has shared a message they received (or their own notes about what to reply). " +
    "Your job: compose a REPLY to that message. " +
    "CRITICAL LANGUAGE RULE: Reply in the SAME language, dialect, and style as the input. " +
    "If the input is Hinglish, reply in Hinglish. If it's Hindi, reply in Hindi. " +
    "If it's English, reply in English. If it has a regional dialect (Rajasthani, Punjabi, etc.), " +
    "keep that flavor in the reply. " +
    "CRITICAL TONE RULE: Match the tone and vibe of the input. " +
    "If it's casual/friendly, reply casually. If it's formal, reply formally. " +
    "Mirror how real people in that language/context actually text. " +
    "Use natural slang, abbreviations, and expressions that fit the conversation. " +
    "Do NOT translate to English. Do NOT make it sound robotic or overly formal."
```

### B.3 Desktop — Hinglish Detection

```rust
// From nlp/language.rs
const HINGLISH_MARKERS: &[&str] = &[
    "kya", "hai", "nahi", "mujhe", "tujhe", "kaise", "kahan", "kab",
    "acha", "theek", "bhai", "yaar", "abhi", "bahut", "kuch", "aur",
    // ... ~60 markers total
];

fn is_hinglish(text: &str, latin_pct: f32) -> bool {
    if latin_pct < 0.7 { return false; }
    let words: Vec<&str> = text.split_whitespace().collect();
    let matches = words.iter().filter(|w| {
        HINGLISH_MARKERS.contains(&w.to_lowercase().as_str())
    }).count();
    matches >= 2 || (words.len() > 3 && matches as f32 / words.len() as f32 > 0.15)
}
```

---

## Acknowledgments

Built solo over ~3 weeks in March–April 2026. Thanks to early testers in freelancer communities for feedback that drove the PROCESS_TEXT pivot and the language-preservation prompt fix.

---

**End of Paper**
