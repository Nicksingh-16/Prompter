# Deep Intent Engine

> **Antigravity** — Implementation Prompt #003  
> *Multi-signal · Confidence-aware · AI-assisted · User-adaptive*

## 1. Why Shallow Intent Fails

The original intent classifier asks one question per signal: "is this word present?" That produces a boolean for each keyword match, sums them, and picks the highest-scoring intent. This is fragile for three reasons:

- It treats all signals equally. "dear" at the start of a message is a strong email signal. "dear" in "oh dear, what a mess" is noise. Position, context, and weight are all ignored.
- It cannot express uncertainty. "I think you should reach out to the client regarding the project" could be an email draft, a prompt request, or a casual chat. The old system picks one and bets everything on it.
- It never learns. If a user always writes short punchy messages but the system keeps suggesting "Summarize" because their text is long, it will keep being wrong forever. There is no feedback loop.

  
> 💡 **INSIGHT**
>
> The goal of the new engine is not to be correct — it is to make the user feel understood. Those are different targets. "Correct" means the highest-probability class. "Understood" means the right suggestion appears first, the alternatives make sense, and when the user overrides the suggestion, it never makes that mistake again for them.

## 2. Four-Layer Architecture

The new engine is a pipeline of four independent layers. Each layer can run without the next one. The first two layers always run synchronously before the UI appears. Layers 3 and 4 are asynchronous enhancements.

| **Layer** | **Description** |
| Layer 1 — Multi-signal extraction | Rust · synchronous · \<1ms · always runs · extracts 20+ signals from 4 signal families |
| Layer 2 — Weighted scoring matrix | Rust · synchronous · \<0.5ms · always runs · produces ranked IntentResult with confidence floats |
| Layer 3 — AI classifier | Gemini · async · \~400ms · fires in background if confidence \< 0.75 · updates UI if result differs |
| Layer 4 — User-adaptive weights | SQLite · sync · \<0.5ms · reads per-user weight overrides · personalizes after 20+ corrections |

> 🎯 **KEY DESIGN**
>
> Layers 1 and 2 run in the same thread as the hotkey handler, before the window shows. The user sees the suggestion instantly. Layers 3 and 4 are non-blocking — if they produce a better result, they update the UI. If not, nothing changes. Zero perceived latency.

## 3. Layer 1 — Multi-Signal Extraction

## 3.1 Signal families

Each signal family extracts features from a different dimension of the text. Together they give a rich, multi-dimensional fingerprint that no single keyword list could produce.

| **Family** | **What it reads** | **How it works** |
| Surface signals | Keywords & phrases | Exact keyword matches weighted by position (start-of-text signals score 2x) |
| Structural signals | Shape of the text | Presence of greeting, sign-off, subject line, bullet points, numbered lists, quoted text, code blocks |
| Behavioral signals | How it was written | Sentence count, avg sentence length, question marks, exclamations, ellipsis, fragments (no verb) |
| Linguistic signals | What language does | Formality score, contraction rate, passive voice count, hedge words ("maybe", "I think"), urgency words |

## 3.2 The SignalVector struct

Layer 1 produces a SignalVector — a flat struct of all extracted features. This is the only thing Layer 2 sees. It never touches the original text again.

```rust
pub struct SignalVector {
// Surface
pub keyword_scores: HashMap<Intent, f32>, // raw keyword match scores
pub greeting_found: bool, // "dear", "hi", "hello", "hey"
pub greeting_position: Option<usize>, // char offset (0 = very start)
pub signoff_found: bool, // "regards", "thanks", "cheers", "best"
pub at_symbol: bool, // @ present
pub subject_line: bool, // starts with "Subject:" or "Re:"
// Structural
pub has_bullet_points: bool, // lines starting with - * •
pub has_numbered_list: bool, // lines starting with 1. 2. etc
pub has_quoted_block: bool, // lines starting with >
pub paragraph_count: usize,
pub has_salutation_structure: bool, // greeting + body + signoff pattern
// Behavioral
pub sentence_count: usize,
pub avg_sentence_len: f32, // in words
pub question_count: usize,
pub exclamation_count: usize,
pub fragment_count: usize, // sentences with no main verb (approx)
pub ellipsis_count: usize,
pub word_count: usize,
// Linguistic
pub formality: u8, // 0--10 from existing scorer
pub tone: i8, // -5 to +5 from existing scorer
pub contraction_rate: f32, // 0.0--1.0
pub hedge_word_count: usize, // "maybe", "perhaps", "I think"
pub urgency_word_count: usize, // "asap", "urgent", "immediately", "deadline"
pub passive_voice_count: usize, // "was done", "is being"
pub imperative_count: usize, // sentences starting with verb
// Language
pub is_mixed_language: bool,
pub primary_script: Script,
}
```

## 3.3 Positional weighting — the critical improvement over shallow intent

The single biggest upgrade over keyword counting. A greeting word at position 0 is 4x more meaningful than the same word mid-text. Implement this as a multiplier applied to keyword matches based on their character offset:

```rust
pub fn position_weight(offset: usize, total_len: usize) -> f32 {
let pct = offset as f32 / total_len as f32;
match pct {
p if p < 0.05 => 4.0, // first 5% of text → very strong signal
p if p < 0.15 => 2.5, // first 15% → strong signal
p if p < 0.30 => 1.5, // first 30% → moderate boost
p if p > 0.85 => 2.0, // last 15% → sign-off zone, strong
_ => 1.0, // middle → neutral weight
}
}
// Usage in keyword scanner:
for (keyword, base_score, intent) in &KEYWORD_TABLE {
if let Some(offset) = text.find(keyword) {
let weight = position_weight(offset, text.len());
*scores.entry(intent).or_insert(0.0) += base_score * weight;
}
}
```

## 3.4 Structural pattern detection

These are the patterns that single-word keywords completely miss. A message with a greeting at the start AND a signoff at the end AND a body paragraph is almost certainly an email — even if it contains zero email-specific words.

```rust
pub fn detect_salutation_structure(text: &str) -> bool {
let lines: Vec<&str> = text.lines().collect();
if lines.len() < 3 { return false; }
let first_line = lines[0].trim().to_lowercase();
let last_line = lines[lines.len()-1].trim().to_lowercase();
let has_opener = GREETING_WORDS.iter().any(|g| first_line.starts_with(g));
let has_closer = CLOSER_WORDS.iter().any(|c| last_line.starts_with(c)
|| last_line.ends_with(c));
let has_body = lines.len() >= 3;
has_opener && has_closer && has_body
}
pub fn detect_prompt_structure(text: &str) -> bool {
// Prompts often have imperatives at the start
// "Write me a", "Create a", "Make this", "Help me"
// AND often lack a personal greeting
// AND tend to be medium length (10-80 words)
let words: Vec<&str> = text.split_whitespace().collect();
let starts_imperative = IMPERATIVE_STARTERS.iter()
.any(|imp| text.to_lowercase().starts_with(imp));
let no_greeting = !GREETING_WORDS.iter()
.any(|g| text.to_lowercase().starts_with(g));
let medium_length = words.len() >= 5 && words.len() <= 100;
starts_imperative && no_greeting && medium_length
}
```

## 4. Layer 2 — Weighted Scoring Matrix

## 4.1 From signals to scores

Layer 2 takes the SignalVector and runs it through a scoring matrix. Each intent has a set of rules. Each rule maps one or more signals to a score contribution. The contributions are summed and normalized to 0.0--1.0.

  
> 💡 **INSIGHT**
>
> Think of this as a decision table, not a decision tree. Every rule for every intent runs on every input. There is no early exit. This means the engine always produces a score for every intent — which is what enables the ranked multi-suggestion output.

## 4.2 The scoring rules — implement as a function per intent

Each function takes the SignalVector and returns a raw f32 score. Normalize all scores at the end.

```rust
fn score_email(sv: &SignalVector) -> f32 {
let mut score = 0.0_f32;
// Strong structural evidence
if sv.has_salutation_structure { score += 3.0; }
if sv.subject_line { score += 2.5; }
if sv.signoff_found { score += 1.5; }
if sv.at_symbol { score += 1.0; }
// Greeting position bonus
if sv.greeting_found {
let pos_weight = sv.greeting_position.map(|p|
if p < 5 { 2.0 } else { 0.5 }
).unwrap_or(0.5);
score += 1.5 * pos_weight;
}
// Keyword matches from surface layer
score += sv.keyword_scores.get(&Intent::Email).copied().unwrap_or(0.0);
// Paragraph structure typical of email
if sv.paragraph_count >= 2 { score += 0.5; }
if sv.formality >= 6 { score += 0.5; }
if sv.urgency_word_count > 0 { score += 0.3; }
// Penalties
if sv.fragment_count > 3 { score -= 1.0; } // fragments → not email
if sv.exclamation_count > 4 { score -= 0.5; } // too casual
score.max(0.0)
}
fn score_chat(sv: &SignalVector) -> f32 {
let mut score = 0.0_f32;
score += sv.keyword_scores.get(&Intent::Chat).copied().unwrap_or(0.0);
if sv.fragment_count > 2 { score += 1.5; } // fragments are normal in chat
if sv.formality <= 3 { score += 1.5; }
if sv.exclamation_count > 2 { score += 0.8; }
if sv.ellipsis_count > 1 { score += 0.6; }
if sv.avg_sentence_len < 8.0 { score += 1.0; } // short sentences = chat
if sv.sentence_count <= 3 { score += 0.5; }
if sv.has_salutation_structure { score -= 2.0; } // structure → not chat
if sv.contraction_rate > 0.3 { score += 0.8; }
score.max(0.0)
}
fn score_prompt(sv: &SignalVector) -> f32 {
let mut score = 0.0_f32;
score += sv.keyword_scores.get(&Intent::Prompt).copied().unwrap_or(0.0);
if sv.imperative_count > 0 { score += 2.0; } // imperatives = commands
if sv.question_count == 0 { score += 0.5; } // prompts rarely have ?
if sv.hedge_word_count > 1 { score += 0.8; } // "maybe make it...", "I think"
if sv.has_salutation_structure { score -= 2.0; }
if sv.has_bullet_points { score += 1.0; } // structured prompt
score.max(0.0)
}
fn score_social(sv: &SignalVector) -> f32 {
let mut score = 0.0_f32;
score += sv.keyword_scores.get(&Intent::Social).copied().unwrap_or(0.0);
// Social posts are typically short, punchy, no greeting
if sv.word_count < 60 { score += 1.0; }
if sv.word_count < 30 { score += 1.0; } // stacks
if sv.exclamation_count > 1 { score += 0.8; }
if sv.has_salutation_structure { score -= 2.5; }
if sv.formality <= 4 { score += 0.5; }
score.max(0.0)
}
fn score_report(sv: &SignalVector) -> f32 {
let mut score = 0.0_f32;
score += sv.keyword_scores.get(&Intent::Report).copied().unwrap_or(0.0);
if sv.has_bullet_points { score += 1.5; }
if sv.has_numbered_list { score += 1.5; }
if sv.word_count > 150 { score += 1.0; }
if sv.paragraph_count >= 3 { score += 0.8; }
if sv.formality >= 7 { score += 1.0; }
if sv.passive_voice_count > 1 { score += 0.5; } // reports use passive
score.max(0.0)
}
```

## 4.3 IntentResult — the output struct

The scoring matrix produces a ranked list of candidates with calibrated confidence values. This replaces the old single-value Intent enum entirely.

```rust
#[derive(Debug, Clone, Serialize)]
pub struct IntentCandidate {
pub intent: Intent,
pub confidence: f32, // 0.0--1.0, normalized across all intents
pub label: &'static str, // human-readable: "Email", "Casual chat"
pub mode: &'static str, // maps to App mode: "Professional", "Casual", etc.
pub reason: &'static str, // one short reason: "greeting + sign-off detected"
}
#[derive(Debug, Clone, Serialize)]
pub struct IntentResult {
pub primary: IntentCandidate,
pub alternatives: Vec<IntentCandidate>, // top 2 alternatives, confidence > 0.15
pub all_scores: HashMap<Intent, f32>,
pub overall_confidence: f32, // how sure we are overall
// overall_confidence = primary.confidence - second.confidence
// high gap (>0.4) → very sure | low gap (<0.2) → ambiguous
}
pub fn normalize_scores(raw: HashMap<Intent, f32>) -> IntentResult {
let total: f32 = raw.values().sum();
if total == 0.0 {
return IntentResult::default_general();
}
let mut normalized: Vec<(Intent, f32)> = raw.iter()
.map(|(k, v)| (k.clone(), v / total))
.collect();
normalized.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
let primary_conf = normalized[0].1;
let second_conf = normalized.get(1).map(|(_, c)| *c).unwrap_or(0.0);
IntentResult {
primary: to_candidate(&normalized[0].0, primary_conf),
alternatives: normalized[1..].iter()
.filter(|(_, c)| *c > 0.15)
.take(2)
.map(|(i, c)| to_candidate(i, *c))
.collect(),
all_scores: raw,
overall_confidence: primary_conf - second_conf,
}
}
```

## 4.4 Confidence thresholds — what each level means

| **Condition** | **Label** | **UI behavior** |
| overall_confidence \> 0.45 | High confidence | Show primary only. No alternatives shown. Auto-apply in headless mode. |
| overall_confidence 0.25--0.45 | Medium confidence | Show primary + 1 alternative. Primary is pre-selected. |
| overall_confidence \< 0.25 | Low confidence | Show primary + 2 alternatives. Primary still pre-selected but visually marked as uncertain with a "?" badge. |
| primary.confidence \< 0.35 | No clear winner | Show 3 suggestions with equal visual weight. No pre-selection. Prompt user to choose. |

## 5. Layer 3 — Lightweight AI Classifier

## 5.1 When to fire it

Layer 3 is not always needed. Only fire the AI classifier when the local engine is uncertain. This saves API calls when the text is clear (saves money and latency) while improving accuracy on genuinely ambiguous inputs (where it matters most).

```rust
pub fn should_fire_ai_classifier(result: &IntentResult) -> bool {
// Fire if: local engine is uncertain OR text is long enough to be worth it
result.overall_confidence < 0.30
|| result.primary.confidence < 0.50
|| result.alternatives.len() >= 2 // multiple strong alternatives
}
```

  
> 🎯 **KEY DESIGN**
>
> The AI classifier fires asynchronously after the UI has already shown the local result. The user sees an instant suggestion from Layers 1+2, and if the AI classifier returns a different (better) result within 800ms, the UI updates smoothly. If the AI is slow or returns the same result, nothing changes. This is the "fast-then-smart" pattern.

## 5.2 The classifier prompt — minimal, JSON-only

This prompt is designed to be as short as possible to minimize tokens and latency. It asks for exactly what we need: a classification with confidence scores.

```rust
pub fn build_classifier_prompt(text: &str) -> String {
format!(
"Classify this text into ONE primary category and provide confidence scores.\n\",
"Categories: Email, Chat, Prompt, Report, Social, General\n\",
"\nText:\n\"{}"\n\",
"Respond with ONLY valid JSON, no explanation, no markdown:\n",
"{{\"primary\": \"Email\", \"confidence\": 0.87, \"alternatives\": [",
"{{\"intent\": \"Chat\", \"confidence\": 0.09}},",
"{{\"intent\": \"Report\", \"confidence\": 0.04}}",
"], \"reason\": \"greeting and sign-off present\"}}",
text
)
}
// Parse response defensively — AI can return malformed JSON
pub fn parse_classifier_response(json: &str) -> Option<IntentResult> {
let clean = json.trim()
.trim_start_matches("```json")
.trim_end_matches("```")
.trim();
serde_json::from_str::<ClassifierResponse>(clean)
.ok()
.map(|r| r.into_intent_result())
}
```

## 5.3 Async flow in lib.rs

Fire the classifier in a Tokio task immediately after the window shows. Emit a separate event when it resolves. The frontend listens for this and updates the suggestion row if the result is meaningfully different.

```rust
// In the hotkey handler, after showing the window:
let ctx = nlp::analyze(&captured);
let result = ctx.intent_result.clone();
// Emit immediately with local result
app.emit("text_captured", payload_with_local_result).ok();
// Fire AI classifier in background if needed
if intent::should_fire_ai_classifier(&result) && keychain::get_api_key().is_ok() {
let app_clone = app.clone();
let text_clone = captured.clone();
let local_primary = result.primary.intent.clone();
tokio::spawn(async move {
if let Some(ai_result) = ai::classify_intent(&app_clone, &text_clone).await {
// Only emit update if AI disagrees with local result
if ai_result.primary.intent != local_primary {
app_clone.emit("intent_refined", ai_result).ok();
}
}
});
}
```

## 5.4 Frontend handling of intent_refined event

The frontend listens for this event and applies a smooth visual transition if the suggestion changes. Do not abruptly replace the UI — animate the change so the user understands what happened.

```rust
// In useEffect, add listener:
const unlistenRefined = listen("intent_refined", (event) => {
const refined = event.payload;
// Only update if user has NOT already clicked a mode
if (!userHasInteracted) {
setIntentResult(refined);
// Brief visual indicator: "AI refined this suggestion"
setShowRefinedBadge(true);
setTimeout(() => setShowRefinedBadge(false), 2000);
}
});
// Show a subtle "✦ refined" badge on the primary suggestion pill
// for 2 seconds after AI update. This teaches users that the system
// is actively thinking for them — it is a trust-building micro-interaction.
```

## 6. Layer 4 — User-Adaptive Weights

## 6.1 The core idea

Every time a user ignores the suggested intent and picks a different mode, that is a correction signal. After enough corrections, the system should stop making the same mistake for that user. This is personalization without any ML model — just a table of weight adjustments stored in SQLite.

## 6.2 SQLite schema

```sql
CREATE TABLE IF NOT EXISTS intent_corrections (
id INTEGER PRIMARY KEY,
timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
suggested_intent TEXT NOT NULL, -- what the engine suggested
chosen_intent TEXT NOT NULL, -- what the user actually picked
text_length INTEGER, -- word count of the input
formality_score INTEGER, -- to identify patterns
language_family TEXT,
confidence REAL -- engine confidence at time of correction
);
CREATE TABLE IF NOT EXISTS intent_weight_overrides (
id INTEGER PRIMARY KEY,
from_intent TEXT NOT NULL, -- when engine suggests this...
to_intent TEXT NOT NULL, -- ...user actually wants this
adjustment REAL NOT NULL, -- score modifier: positive = boost to_intent
sample_count INTEGER DEFAULT 0,
UNIQUE(from_intent, to_intent)
);
```

## 6.3 Recording a correction

Call this function whenever the user selects a mode that differs from the primary suggestion. Must be called in App.tsx when the user clicks a non-primary mode pill.

```rust
// In App.tsx, when user clicks a mode pill:
const handleModeSelect = (mode: string) => {
setSelectedMode(mode);
// If user picked something other than the primary suggestion:
if (intentResult && mode !== intentResult.primary.mode) {
invoke("record_intent_correction", {
suggestedIntent: intentResult.primary.intent,
chosenIntent: mode,
confidence: intentResult.primary.confidence,
});
}
};
// In lib.rs — Tauri command:
#[tauri::command]
fn record_intent_correction(
app: AppHandle,
suggested_intent: String,
chosen_intent: String,
confidence: f32,
) {
db::save_correction(&app, &suggested_intent, &chosen_intent, confidence);
db::recompute_weight_override(&app, &suggested_intent, &chosen_intent);
}
```

## 6.4 Computing and applying weight overrides

```rust
// Recompute override after each new correction:
pub fn recompute_weight_override(app: &AppHandle, from: &str, to: &str) {
let count = db::count_corrections(app, from, to);
// Only activate after 3+ corrections (avoid noise from accidents)
if count < 3 { return; }
// Adjustment grows with sample count, caps at 2.5
let adjustment = (count as f32 * 0.4).min(2.5);
db::upsert_weight_override(app, from, to, adjustment, count);
}
// Apply overrides in the scoring matrix (Layer 2):
pub fn apply_user_overrides(
app: &AppHandle,
mut scores: HashMap<Intent, f32>,
) -> HashMap<Intent, f32> {
let overrides = db::load_weight_overrides(app);
for ovr in overrides {
// When the from_intent is winning, boost the to_intent
let from_score = scores.get(&ovr.from_intent).copied().unwrap_or(0.0);
if from_score > 0.3 { // only apply when from_intent is actually scoring
*scores.entry(ovr.to_intent).or_insert(0.0) += ovr.adjustment;
}
}
scores
}
```

  
> 💡 **INSIGHT**
>
> Weight overrides create a deeply personal experience. After a few weeks, a user who always writes formal emails and never uses the "Casual" mode will find the system stops suggesting it. A user who always wants "Custom" for their complex messages will find it bubbling up to first position. The system silently learns without ever asking.

## 7. Multi-Suggestion UI in App.tsx

## 7.1 The SuggestionBar component

Replace the current static mode pill row with a dynamic SuggestionBar that reads from the IntentResult. The primary suggestion is visually dominant. Alternatives are visible but secondary. The static mode list is still accessible via a "More" control.

```typescript
interface SuggestionBarProps {
result: IntentResult | null;
selectedMode: string;
onSelect: (mode: string) => void;
isRefined: boolean;
}
const CONFIDENCE_LABEL = (c: number) =>
c > 0.75 ? null // high confidence: no badge
: c > 0.50 ? "likely" // medium: soft label
: "maybe"; // low: explicit uncertainty
const SuggestionBar = ({ result, selectedMode, onSelect, isRefined }) => {
if (!result) return <StaticModeRow onSelect={onSelect} selected={selectedMode} />;
const { primary, alternatives, overall_confidence } = result;
const confidenceLabel = CONFIDENCE_LABEL(overall_confidence);
return (
<div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
{/* Primary suggestion */}
<button
className={`mode-pill ${selectedMode === primary.mode ? "active" : ""}`}
onClick={() => onSelect(primary.mode)}
style={{ position: "relative" }}
>
{isRefined && <span className="refined-dot" />}
{primary.label}
{confidenceLabel && (
<span style={{ opacity: 0.6, fontSize: "10px", marginLeft: "4px" }}>
{confidenceLabel}
</span>
)}
</button>
{/* Alternatives */}
{alternatives.map(alt => (
<button
key={alt.intent}
className={`mode-pill ${selectedMode === alt.mode ? "active" : ""}`}
onClick={() => onSelect(alt.mode)}
style={{ opacity: 0.7 }}
title={`${Math.round(alt.confidence * 100)}% confidence`}
>
{alt.label}
</button>
))}
{/* Expand to full mode list */}
<button
className="mode-pill"
onClick={() => setShowAllModes(true)}
style={{ opacity: 0.4, padding: "5px 8px" }}
title="All modes"
>
···
</button>
</div>
);
};
```

## 7.2 The reason tooltip

Each suggestion pill shows a tooltip on hover explaining why it was suggested. This is the "mind-reading" transparency — users understand how the system thinks, which builds trust.

```rust
// reason comes from IntentCandidate.reason field
// Examples of reason strings:
// Email: "greeting at start + sign-off detected"
// Chat: "short, casual, high fragment count"
// Prompt: "starts with imperative verb, no greeting"
// Report: "bullet points + formal language + long text"
// Social: "very short, punchy, contains hashtag signals"
// Show as title attribute on the pill — no extra component needed
<button title={`${primary.label}: ${primary.reason}`}>
```

## 8. Intent-to-Mode Mapping Table

The scoring engine works in terms of Intents (Email, Chat, Prompt, etc.). The UI works in terms of Modes (Professional, Casual, Fix, etc.). This static mapping table connects them. Multiple intents can map to the same mode, and one intent can suggest different modes based on formality.

| **Intent condition** | **Suggested mode** | **Confidence modifier** | **Reason string** |
| Email + formality \>= 6 | Professional | high | "Formal email detected" |
| Email + formality \< 6 | Casual | medium | "Informal email detected" |
| Chat | Casual | high | "Casual message detected" |
| Prompt | Custom | high | "Prompt request detected" |
| Report + word_count \> 200 | Summarize | medium | "Long report or update" |
| Report + word_count \<= 200 | Professional | medium | "Short formal document" |
| Social | Casual | high | "Social post detected" |
| General + tone \< -1 | Fix | low | "Possibly a draft needing cleanup" |
| General + word_count \> 150 | Summarize | low | "Long text — summarize?" |
| General | Fix | low | "No strong pattern detected" |

## 9. Integration Checklist

Complete these steps in order. Each step is independently testable.

| **Step** | **Action** |
| Step 1 | Create SignalVector struct in nlp/intent.rs. All fields default to 0/false. Write extract_signals(text) → SignalVector. Unit test with 10 sample inputs. |
| Step 2 | Implement position_weight() and update keyword scanner to use it. Verify "dear" at position 0 scores 4x higher than "dear" in the middle. |
| Step 3 | Implement all 5 scoring functions (score_email, score_chat, score_prompt, score_social, score_report). Implement normalize_scores(). Return IntentResult. |
| Step 4 | Implement IntentResult and IntentCandidate structs. Derive Serialize. Verify JSON output matches expected schema. |
| Step 5 | Replace old Intent detection in nlp::analyze() with new IntentResult. Update TextContext struct to use IntentResult. Recompile — fix all type errors. |
| Step 6 | Update analyze_text Tauri command to include full IntentResult in JSON output. Update text_captured event payload to include intent context. |
| Step 7 | Implement intent_corrections and intent_weight_overrides SQLite tables in db.rs. Implement record_intent_correction() and apply_user_overrides(). |
| Step 8 | Add record_intent_correction Tauri command. Wire it to mode selection in App.tsx handleModeSelect(). |
| Step 9 | Implement AI classifier in ai.rs: classify_intent() async function. Implement build_classifier_prompt() and parse_classifier_response(). |
| Step 10 | Wire AI classifier into hotkey handler: spawn tokio task, emit intent_refined event. Add listener in App.tsx. Test with ambiguous input text. |
| Step 11 | Build SuggestionBar component in App.tsx. Replace static mode row. Test with high/medium/low confidence IntentResult values. |
| Step 12 | Add refined-dot CSS animation. Add tooltip reasons to pills. Polish visual transitions. Ship. |

## 10. Test Input Suite

Run the full pipeline on each of these inputs and verify the primary intent and confidence level match expectations. Fix scoring rules until all pass.

| **Input text** | **Expected primary intent** | **Expected confidence** |
| "Dear Mr. Sharma, I hope this email finds you well. I am writing regarding the Q3 budget proposal. Please find attached the revised figures. Best regards, Arjun" | Email | \> 0.85 (strong structural evidence) |
| "hey when r u free tmrw lol need to talk abt the thing" | Chat | \> 0.80 |
| "Write me a professional LinkedIn post about the product launch we had last week. Make it engaging and include a call to action." | Prompt | \> 0.75 |
| "Q3 Performance Summary:\\n- Revenue: ₹42L (↑12%)\\n- Churn: 3.2%\\n- NPS: 47\\nKey concerns: Support ticket volume up 28%" | Report | \> 0.70 |
| "bhai kal meeting thi client ke saath, unhone bola budget approve ho gaya finally" | Chat (Hinglish) | \> 0.65 |
| "I think maybe we should reach out to the client and let them know about the delay" | General/Email | \< 0.40 overall_confidence — ambiguous, show 2 alternatives |
| "As per our previous discussion, please ensure all deliverables are submitted by EOD Friday" | Email | \> 0.70 (formal tone, implicit email structure) |
| "Just wanted to quickly check in and see how things are going on your end!" | Chat | \> 0.65 (casual, hedge phrase, exclamation) |

*End of Document 3 — Deep Intent Engine*