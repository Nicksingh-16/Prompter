# Moat Features

> **Antigravity** — Implementation Prompt #002  
> *Personal voice, context memory, tone mirror, team voice, analytics*

## Overview

This document is the complete implementation specification for the five moat-building features of Antigravity. These features transform the app from a text tool into a personal communication OS that users cannot replace and companies want to acquire.

  
> 💡 **KEY INSIGHT**
>
> Build these features in the exact order listed. Each one increases the cost of switching away from Antigravity. By Feature 3, users have something irreplaceable. By Feature 5, companies have a procurement reason.

| **\#** | **Feature** | **Core Value** |
| Feature 1 | Personal Voice Engine | Learns individual style over time — local only |
| Feature 2 | Context Memory | Remembers people, projects, recurring topics |
| Feature 3 | Tone Mirror | Pre-send emotional impact prediction |
| Feature 4 | Communication Score | Weekly analytics — habit formation loop |
| Feature 5 | Team Shared Voice | B2B unlock — company-wide tone consistency |

## Feature 1 — Personal Voice Engine

## What it is

After 2--3 weeks of use, the app has observed enough of the user's writing patterns to model their specific communication style. The AI output then sounds like the user being polished — not like a generic AI rewrite. This is the hardest feature to replicate and the strongest lock-in.

## Data model — voice_profile table in SQLite

```sql
CREATE TABLE IF NOT EXISTS voice_profile (
id INTEGER PRIMARY KEY,
feature_type TEXT NOT NULL,
feature_key TEXT NOT NULL,
value TEXT,
count INTEGER DEFAULT 1,
last_seen DATETIME DEFAULT CURRENT_TIMESTAMP,
UNIQUE(feature_type, feature_key)
);
-- feature_type values:
-- "vocab" → words user uses that are not common English
-- "opener" → how user starts messages ("Hey", "Hi", "Hello")
-- "closer" → how user ends messages ("Thanks", "Regards", "Cheers")
-- "avg_sentence" → typical sentence length in words
-- "contraction" → usage rate of contractions (0.0--1.0)
-- "topic" → recurring topics (keywords seen 5+ times)
```

## What to observe per session — called after every successful generation

```rust
pub fn observe_session(app: &AppHandle, ctx: &TextContext, mode: &str) {
// 1. Extract opener: first word/phrase of normalized text
// if starts with "Hey", "Hi", "Hello", "Dear" → store as opener
// 2. Extract closer: last sentence if it is <= 5 words
// "Thanks", "Regards", "Cheers", "Talk soon" → store as closer
// 3. Collect non-stopword words with length > 5
// These are likely the user's vocabulary fingerprint
// Increment count for each in voice_profile WHERE feature_type = "vocab"
// 4. Record average sentence length for this session
// Update running average in "avg_sentence" row
// 5. Collect top_keywords → increment topic counts
// 6. Prune: delete rows where last_seen < 90 days ago
// Voice model should reflect recent patterns, not old ones
}
```

## Reading the voice profile — build_voice_block()

Called by the prompt builder. Returns a paragraph inserted into every Gemini prompt after the context block.

```rust
pub fn build_voice_block(app: &AppHandle) -> Option<String> {
// Requires minimum 20 sessions observed before activating
// Do not inject voice block if profile is too thin
// Load from SQLite:
// top 3 openers by count
// top 3 closers by count
// top 10 vocab words by count (these define their voice)
// avg_sentence length
// contraction rate
// Build string like:
// "User's writing style profile (learned from their actual messages):",
// "- They typically open with: Hey / Hi / Hello"
// "- They typically close with: Thanks / Cheers"
// "- Their vocabulary fingerprint includes: synergy, bandwidth, iterate"
// "- Average sentence length: 12 words (medium)"
// "- Contraction usage: high (casual register)"
// "Match this voice in the output. The result should sound like this",
// "specific person, not a generic AI."
}
```

## Frontend — Voice Profile Settings panel

Add a "My Voice" tab to the settings panel. Show:

- Sessions observed: 47
- Voice model status: Active / Building (need X more sessions)
- Your common openers: Hey, Hi, Hello
- Your common closers: Thanks, Best, Cheers
- Your topics: product, launch, team, client, deadline
- A toggle: "Apply my voice to AI output" (default: on)
- A button: "Reset voice model" — clears voice_profile table

  
> ⚠️ **IMPORTANT**
>
> Never show raw word counts or frequencies to the user. It feels creepy. Show it as "Your communication style" with human-readable labels. The data collection is transparent but the presentation must feel warm.

## Feature 2 — Context Memory

## What it is

The app remembers people the user writes about, projects they mention, and the tone they use with each person. When a new message mentions someone's name or a known project, the app automatically includes that relationship context in the Gemini prompt without the user asking.

## Data model — context_memory table

```sql
CREATE TABLE IF NOT EXISTS context_memory (
id INTEGER PRIMARY KEY,
entity_type TEXT NOT NULL, -- "person", "project", "company", "topic"
entity_name TEXT NOT NULL,
attribute TEXT NOT NULL, -- "typical_tone", "relationship", "last_topic"
value TEXT,
confidence REAL DEFAULT 1.0, -- decays over time
last_seen DATETIME DEFAULT CURRENT_TIMESTAMP,
seen_count INTEGER DEFAULT 1,
UNIQUE(entity_type, entity_name, attribute)
);
```

## Entity extraction — what to detect

Proper nouns in the user's text are candidates. Apply these heuristics in Rust from the normalized text:

- Capitalized words that are not sentence-starters and not in a common English word list → candidate person or project name
- Words following "with", "from", "to", "for", "hi", "dear", "hey" → very likely a person name
- Capitalized multi-word sequences (two or more consecutive caps) → likely a project name or company
- Email address domain → company name (extract domain, strip TLD)

```rust
// Minimum confidence threshold to store: seen_count >= 2
// This prevents one-time typos from polluting the memory
// What to store per person:
// typical_tone: average of tone scores from sessions mentioning them
// relationship: inferred from formality score when writing to them
// formality >= 7 → "formal/professional"
// formality 4-6 → "colleague"
// formality <= 3 → "close/casual"
// last_topic: most recent keywords from sessions mentioning them
```

## Memory injection into prompt

Check the current input text for known entity names. If found, inject a memory block:

```rust
// Example injected memory block:
"Relationship context from your history:",
"- Rahul: you typically write to Rahul at formality 7/10 (professional)",
"- Rahul: last topic with Rahul was: Q3 report, client presentation",
"- Project Phoenix: mentioned 12 times, usually in formal context",
"Match the appropriate tone for this relationship."
```

## Frontend — Memory panel

- Show a "My Connections" list: all known people with session count and relationship type
- Allow user to manually edit or delete any entry
- Show a "My Projects" list similarly
- Add toggle per entry: "Include in AI context" (default: on)

## Feature 3 — Tone Mirror

## What it is

Before the user sends anything, Tone Mirror shows a single-sentence prediction of how the recipient will emotionally receive the message. It runs entirely on the local NLP pipeline — no AI needed. It appears as a subtle label below the captured text preview in the main overlay.

## Always-on, zero friction

Tone Mirror is NOT a mode the user selects. It runs automatically on every text capture and displays inline. It must compute in under 2ms since it blocks the UI from showing.

## Verdict generation — deterministic rules

```rust
pub fn tone_verdict(ctx: &TextContext) -> ToneVerdict {
// Priority order — first match wins:
// 1. Friction phrases → highest concern
if !ctx.friction_phrases.is_empty() {
return ToneVerdict {
label: "Passive-aggressive risk",
color: "red",
detail: format!("'{}' may read as frustrated to the recipient.",
ctx.friction_phrases[0]),
}
}
// 2. Tone score
match ctx.tone {
-5..=-3 => ToneVerdict { label: "Reads as frustrated or demanding",
color: "red", detail: "..." },
-2..=-1 => ToneVerdict { label: "Slightly tense",
color: "amber", detail: "..." },
0..=1 => ToneVerdict { label: "Neutral and clear",
color: "green", detail: "..." },
2..=3 => ToneVerdict { label: "Warm and positive",
color: "green", detail: "..." },
4..=5 => ToneVerdict { label: "Very enthusiastic",
color: "blue", detail: "Consider toning down slightly for professional contexts." },
_ => ToneVerdict { label: "Neutral", color: "gray", detail: "" }
}
// 3. Formality mismatch check
// If intent = Email AND formality < 4 → warn about low formality
// If intent = Chat AND formality > 8 → note it feels stiff
}
```

## UI implementation in App.tsx

Add a TonePill component that renders between the captured text preview and the mode selector. It should be visually subtle — a small colored dot + short label. Never a full modal or blocking element.

```rust
// Receive from text_captured event payload:
// { text, context: { tone, friction_phrases, formality, intent, ... } }
const TonePill = ({ verdict }) => (
<div style={{
display: "flex", alignItems: "center", gap: "6px",
fontSize: "11px", opacity: 0.75, marginBottom: "8px"
}}>
<span style={{
width: "6px", height: "6px", borderRadius: "50%",
background: TONE_COLORS[verdict.color]
}} />
<span>{verdict.label}</span>
{verdict.detail && <span style={{ opacity: 0.6 }}>— {verdict.detail}</span>}
</div>
)
```

  
> 💡 **KEY INSIGHT**
>
> Tone Mirror must never feel like a judgment. The language should be descriptive ("reads as") not prescriptive ("you sound angry"). Users will disable features that feel like they are being corrected. They will keep features that feel like a helpful mirror.

## Feature 4 — Communication Score

## What it is

A weekly report delivered via the system tray that shows the user measurable trends in their communication. It creates a habit loop: users open the app on Sunday to see their score, which reminds them the app exists, which leads to more usage during the week.

## Data to aggregate — already in history table

No new data collection needed. The history table already stores: timestamp, mode, input_preview, char_count. Add two columns:

```rust
ALTER TABLE history ADD COLUMN tone_score INTEGER DEFAULT 0;
ALTER TABLE history ADD COLUMN formality_score INTEGER DEFAULT 5;
-- Populate these when saving each history entry from the nlp context
```

## Weekly report data points — all computable from SQLite

| **Metric** | **Query** |
| Transformations this week | COUNT(\*) WHERE timestamp \> 7 days ago |
| Most used mode | GROUP BY mode, ORDER BY count DESC, LIMIT 1 |
| Average tone score | AVG(tone_score) for the week |
| Tone trend | Compare this week avg vs last week avg → ↑ ↓ → |
| Clarity trend | Proxy: formality score trend over 4 weeks |
| Your friction phrases | Aggregate from nlp analysis stored in history |
| Languages written in | From language context stored per session |
| Streak: days active this week | COUNT(DISTINCT date(timestamp)) WHERE 7 days |

## Delivery mechanism

Check once per day (on app startup) if a week has passed since last report. If yes, emit a tray notification. On click, open a small report window — not the main overlay, a separate webview window.

```rust
// In lib.rs setup():
let last_report = db::get_last_report_date(&handle);
let days_since = chrono::Utc::now() - last_report;
if days_since.num_days() >= 7 {
let report = db::compute_weekly_report(&handle);
app.emit("weekly_report_ready", report).ok();
// Show tray notification (platform-specific)
}
```

## Report UI — WeeklyReport.tsx

Design as a separate small window (400x520px). Key principles:

- Lead with the streak number — most motivating metric
- Show tone trend as a simple sparkline (5 dots, colored by score)
- One actionable insight: "Your most common friction phrase this week was 'as I mentioned'. Try removing it."
- A count of how many words the AI helped improve
- End with a forward-looking line: "You wrote in 2 languages this week."

## Feature 5 — Team Shared Voice

## What it is

A company administrator defines a shared voice profile — preferred phrases, forbidden phrases, tone guidelines, and a formality target. Every team member's app pulls from this shared profile, ensuring all outgoing communication sounds consistent.

## Architecture — simple file-based sync, no server required

Phase 1 (no infrastructure): The admin exports a voice_config.json file. Team members import it. Stored locally in app data dir. This is enough for teams of 2--20.

Phase 2 (with infrastructure): A simple REST endpoint serves the JSON. The app polls on startup. This requires a backend but is a thin HTTP server.

## voice_config.json schema

```rust
{
"org_name": "Acme Corp",
"version": "1.0",
"updated_at": "2025-01-01T00:00:00Z",
"formality_target": 7,
"tone_target": 1,
"preferred_phrases": [
{ "instead_of": "ASAP", "use": "by [date]" },
{ "instead_of": "per my last email", "use": "as I mentioned on [date]" },
{ "instead_of": "just wanted to", "use": "I am writing to" }
],
"forbidden_phrases": [
"per my last email", "as per", "going forward",
"circle back", "touch base", "synergy", "leverage"
],
"brand_voice_description": "We are direct, warm, and specific. We never use jargon. We write in plain English. We respect the recipient's time.",
"sign_off": "Best,",
"custom_instructions": "Always include a clear call to action if requesting something."
}
```

## Integration into prompt builder

When a team config is loaded, add a Team Voice block to every Gemini prompt:

```rust
"Company voice guidelines (apply to all output):",
"- Target formality: 7/10 (professional but not stiff)",
"- Brand voice: Direct, warm, and specific. No jargon. Plain English.",
"- Forbidden phrases: per my last email, circle back, touch base, synergy",
"- Preferred: use 'by [date]' instead of 'ASAP'",
"- Sign-off: Best,",
"- Custom: Always include a clear call to action if requesting something."
```

## Local enforcement — Rust-side check

Before sending to AI, and before injecting the result back, run a forbidden_phrase_check:

```rust
pub fn check_forbidden(text: &str, config: &VoiceConfig) -> Vec<String> {
config.forbidden_phrases.iter()
.filter(|phrase| text.to_lowercase().contains(phrase.to_lowercase().as_str()))
.cloned()
.collect()
}
// If violations found in the AI output, append to prompt:
// "IMPORTANT: The following phrases were found in your output and must
// be removed or replaced: [list]. Rewrite without them."
```

## Frontend — Team Voice panel in Settings

- Import team config button → file picker for voice_config.json
- Show org name, version, last updated
- Show forbidden phrase list — user can preview but not edit (admin-locked)
- Show a green "Team Voice Active" badge in the main overlay header when config is loaded
- Export button for admins: "Create Team Config" → guided wizard to build the JSON

  
> 💡 **KEY INSIGHT**
>
> For the B2B pitch: a company with 10 sales reps using this, each paying $20/month, is $200/month. A company with 50 customer support agents is $1000/month. The admin dashboard to manage this centrally is the $500/month enterprise tier. Price accordingly.

## Build Order & Milestones

Build in this exact sequence. Do not skip ahead. Each milestone must be stable before the next begins.

| **Timeline** | **Milestone** |
| Week 1--2 | Local NLP engine (Document 1). All five stages. Expose analyze_text command. Verify intent detection works on 20 test inputs. |
| Week 3 | Wire NLP into hotkey handler. Frontend receives context object. Auto-suggest mode based on detected intent. Tone Mirror pill visible in UI. |
| Week 4 | Tone Mirror complete. Local transforms complete (Fix, Shorten, Summarize in Rust). App now fully functional with zero API key. |
| Week 5--6 | Personal Voice Engine. observe_session() called after every generation. Voice profile panel in settings. build_voice_block() injected into prompts. |
| Week 7--8 | Context Memory. Entity extraction. Memory panel. Injection into prompts. |
| Week 9 | Communication Score. Weekly report computed. Tray notification. Report window UI. |
| Week 10+ | Team Shared Voice. voice_config.json schema. Import/export. Local forbidden phrase enforcement. B2B pricing and admin wizard. |

*End of Document 2 — Moat Features*