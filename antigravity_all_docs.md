# Antigravity — Complete Implementation Docs

> Three implementation prompts for the full Antigravity architecture.  
> Self-contained — hand any doc directly to an AI coding assistant.

---

## Table of Contents

- [Doc 1 — Local NLP Engine](#local-nlp-engine)
- [Doc 2 — Moat Features](#moat-features)
- [Doc 3 — Deep Intent Engine](#deep-intent-engine)

---

# Local NLP Engine

> **Antigravity** — Implementation Prompt #001  
> *Zero-dependency, offline-first text intelligence layer*

## Overview & Design Philosophy

This document is the complete implementation specification for the Antigravity local NLP engine — a pure-Rust, zero-dependency text intelligence pipeline that runs entirely on the user's machine. No network call, no ML model download, no external binary. It must initialize in under 10ms and process any text input in under 5ms.

  
> 💡 **KEY INSIGHT**
>
> This engine is not a translator and not a language model. Its job is to deeply understand what the user wrote — their intent, language, tone, formality, and key topics — and hand that structured understanding to Gemini as a precision-engineered prompt. Gemini does the language work. This engine does the observation work.

## Architecture: Five Sequential Stages

The pipeline is a pure function: TextInput → TextContext. Each stage is stateless. No global mutable state. All stages run synchronously in under 5ms total on any modern CPU.

| **Stage** | **Name** | **What It Does** |
| Stage 1 | Normalize & Clean | Unicode fix, whitespace, script detection |
| Stage 2 | Script & Language | Unicode block scan, mix ratio, family detection |
| Stage 3 | Intent & Formality | Keyword scoring, heuristic classification |
| Stage 4 | Feature Extraction | Sentences, keywords, TF-IDF, tone score |
| Stage 5 | Context Assembly | Builds TextContext struct, selects prompt template |

## File Structure

Create a new Rust module: src-tauri/src/nlp/. All files live here. lib.rs imports this module as mod nlp.

```rust
src-tauri/src/nlp/
mod.rs ← public API, TextContext struct, pipeline entry point
normalize.rs ← Stage 1
language.rs ← Stage 2
intent.rs ← Stage 3
features.rs ← Stage 4
prompt.rs ← Stage 5, prompt template builder
data/
stopwords.rs ← embedded stopword list (generated const)
scripts.rs ← Unicode range table
```

  
> 🚨 **CRITICAL**
>
> Do NOT use any crates beyond what is already in Cargo.toml. No NLP crates, no regex crates. Pure Rust std only. The entire nlp/ module must compile with zero new dependencies.

## Stage 1 — Normalize & Clean (normalize.rs)

## Purpose

Takes raw user text and returns a clean, consistent string that all downstream stages can reliably process. Real user input is messy: smart quotes copied from Word, non-breaking spaces, double spaces, mixed line endings, BOM characters. This stage removes all of that.

## Implementation

```rust
pub fn normalize(raw: &str) -> String {
raw.chars()
.map(|c| normalize_char(c))
.collect::<String>()
.split_whitespace() // collapses all whitespace runs to single space
.collect::<Vec<_>>()
.join(" ")
.trim()
.to_string()
}
fn normalize_char(c: char) -> char {
match c {
// Smart quotes → ASCII
'\u{2018}' | '\u{2019}' => '\''
'\u{201C}' | '\u{201D}' => '"'
// Em/en dash → hyphen
'\u{2013}' | '\u{2014}' => '-'
// Non-breaking space, thin space, zero-width → space
'\u{00A0}' | '\u{200B}' | '\u{FEFF}' | '\u{202F}' => ' '
// Ellipsis → three dots
'\u{2026}' => '.' // emit once; caller handles run collapse
_ => c
}
}
```

## Also compute in this stage

- char_count: usize — raw character count before normalization
- word_count: usize — word count of normalized text
- has_urls: bool — check for "http" substring
- has_emails: bool — check for "@" with adjacent non-space chars
- dominant_case: enum { AllCaps, Lowercase, Mixed } — for detecting shouting or casual text

## Stage 2 — Script & Language Detection (language.rs)

## Core concept

Every Unicode character belongs to a named block with a defined code point range. By scanning all characters and counting which blocks they fall into, you can determine what language family a text belongs to — with no ML, no dictionary, no external data. This runs in O(n) where n is the number of characters.

## Script enum — implement all of these

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Script {
Latin, // U+0041--U+024F → EN FR ES DE IT PT NL RO PL etc.
Devanagari, // U+0900--U+097F → HI MR NE SA
Arabic, // U+0600--U+06FF → AR UR FA PS
Cyrillic, // U+0400--U+04FF → RU UK BG SR MK
CJK, // U+4E00--U+9FFF → ZH (Hanzi, shared)
Hiragana, // U+3040--U+309F → JA
Katakana, // U+30A0--U+30FF → JA (loanwords)
Hangul, // U+AC00--U+D7AF → KO
Tamil, // U+0B80--U+0BFF → TA
Telugu, // U+0C00--U+0C7F → TE
Gujarati, // U+0A80--U+0AFF → GU
Bengali, // U+0980--U+09FF → BN AS
Kannada, // U+0C80--U+0CFF → KN
Malayalam, // U+0D00--U+0D7F → ML
Punjabi, // U+0A00--U+0A7F → PA (Gurmukhi script)
Thai, // U+0E00--U+0E7F → TH
Greek, // U+0370--U+03FF → EL
Hebrew, // U+0590--U+05FF → HE IW
Punctuation, // digits, spaces, symbols — excluded from ratio
Unknown,
}
```

## detect_script function

```rust
pub fn detect_script(c: char) -> Script {
let cp = c as u32;
match cp {
0x0041..=0x024F => Script::Latin,
0x0900..=0x097F => Script::Devanagari,
0x0600..=0x06FF => Script::Arabic,
0x0400..=0x04FF => Script::Cyrillic,
0x4E00..=0x9FFF => Script::CJK,
0x3040..=0x309F => Script::Hiragana,
0x30A0..=0x30FF => Script::Katakana,
0xAC00..=0xD7AF => Script::Hangul,
0x0B80..=0x0BFF => Script::Tamil,
0x0C00..=0x0C7F => Script::Telugu,
0x0A80..=0x0AFF => Script::Gujarati,
0x0980..=0x09FF => Script::Bengali,
0x0C80..=0x0CFF => Script::Kannada,
0x0D00..=0x0D7F => Script::Malayalam,
0x0A00..=0x0A7F => Script::Punjabi,
0x0E00..=0x0E7F => Script::Thai,
0x0370..=0x03FF => Script::Greek,
0x0590..=0x05FF => Script::Hebrew,
0x0030..=0x0039 | 0x0020..=0x002F |
0x003A..=0x0040 | 0x005B..=0x0060 => Script::Punctuation,
_ => Script::Unknown,
}
}
```

## LanguageContext struct — output of this stage

```rust
pub struct LanguageContext {
pub primary_script: Script,
pub primary_pct: f32, // 0.0--1.0
pub secondary_script: Option<Script>,
pub secondary_pct: f32,
pub is_mixed: bool, // true when secondary_pct > 0.15
pub is_rtl: bool, // true for Arabic, Hebrew
pub language_family: &'static str, // e.g. "Indic", "Semitic", "CJK"
pub candidate_languages: &'static str, // e.g. "Hindi, Marathi, or Nepali"
pub needs_romanization_hint: bool, // true if non-Latin primary
}
```

## candidate_languages lookup — implement as static match

| **Script** | **candidate_languages value** |
| Latin | a European language (English, French, Spanish, German, Italian, Portuguese, Dutch, Polish, or similar) |
| Devanagari | Hindi, Marathi, or Nepali |
| Arabic | Arabic, Urdu, or Farsi |
| Cyrillic | Russian, Ukrainian, or Bulgarian |
| CJK | Chinese (Mandarin or Cantonese) |
| Hiragana / Katakana | Japanese |
| Hangul | Korean |
| Tamil | Tamil |
| Telugu | Telugu |
| Gujarati | Gujarati |
| Bengali | Bengali or Assamese |
| Kannada | Kannada |
| Malayalam | Malayalam |
| Punjabi | Punjabi |
| Thai | Thai |
| Greek | Greek |
| Hebrew | Hebrew |

> 💡 **KEY INSIGHT**
>
> Mixed detection rule: if primary_pct < 0.85 and secondary_pct > 0.15, set is_mixed = true. This catches Hinglish (Latin + Devanagari), code-switched Spanish-English, Arabizi (Arabic meaning written in Latin), and any other common mixing pattern.

## Stage 3 — Intent & Formality Detection (intent.rs)

## Intent classification

Each intent has a static keyword list. Score each word in the input against every list. The intent with the highest score wins. Minimum score of 2 required to claim an intent — below that, fall back to General.

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
Email, // formal or personal email composition
Chat, // casual message, WhatsApp, DM
Prompt, // user is trying to write an AI prompt
Report, // formal document, memo, status update
Social, // tweet, LinkedIn post, Instagram caption
General, // fallback — do not force a category
}
```

## Keyword signal tables — embed as static &\[&str\]

| **Intent** | **Signal keywords** |
| Email | "dear", "hi", "hello", "regards", "sincerely", "subject:", "@", "please", "kindly", "attached", "as discussed", "following up", "pursuant", "re:", "fwd:", "cc:", "bcc:" |
| Chat | "lol", "btw", "tbh", "ngl", "gonna", "wanna", "kinda", "yep", "nope", "hey", "sup", "brb", "omg", "wtf", "haha", "lmao", "fr", "imo", "tbf" |
| Prompt | "write", "create", "make", "generate", "help me", "can you", "i need", "please make", "draft", "compose", "build", "describe", "explain", "list", "give me" |
| Report | "summary", "update", "status", "report", "findings", "analysis", "conclusion", "recommendation", "overview", "results", "q1", "q2", "q3", "q4", "kpi", "metrics" |
| Social | "tweet", "post", "thread", "caption", "hashtag", "viral", "followers", "linkedin", "instagram", "share", "reel", "story", "hook" |

## Formality scoring — returns u8 from 0 to 10

Score starts at 5 (neutral). Apply these deltas and clamp to 0--10:

| **Signal** | **Delta** |
| Contractions found (won't, can't, I'm, etc.) | -1 per contraction, max -3 |
| Average word length \> 6 chars | +2 |
| Sentences end with proper punctuation | +1 |
| ALL CAPS words found | -1 per word, max -2 |
| Exclamation marks \> 2 | -1 |
| Intent is Email or Report | +1 |
| Intent is Chat or Social | -2 |
| Word "please" or "kindly" found | +1 |
| Slang words from Chat list found | -1 per word, max -3 |

## Stage 4 — Feature Extraction (features.rs)

## Sentence splitting

Split on sentence-ending punctuation followed by whitespace and an uppercase letter. Handle common abbreviations as exceptions.

```rust
// Exception list — do NOT split after these
const ABBREVS: &[&str] = &[
"Mr.", "Mrs.", "Ms.", "Dr.", "Prof.", "Sr.", "Jr.",
"Inc.", "Ltd.", "Corp.", "etc.", "vs.", "i.e.", "e.g.",
"Jan.", "Feb.", "Mar.", "Apr.", "Jun.", "Jul.", "Aug.",
"Sep.", "Oct.", "Nov.", "Dec.",
];
pub fn split_sentences(text: &str) -> Vec<String> {
// 1. Temporarily replace abbreviation dots with placeholder
// 2. Split on [.!?] followed by space + uppercase
// 3. Restore placeholders
// 4. Trim each sentence
// 5. Filter empty strings
}
```

## Keyword extraction — lightweight TF-IDF

No actual TF-IDF corpus needed. Use a stopword list as an inverse frequency approximation: words not in the stopword list get score 1.0, common words get 0.0. Then boost by frequency in the input text.

```rust
pub fn extract_keywords(text: &str, top_n: usize) -> Vec<String> {
let words = tokenize(text); // lowercase, strip punctuation
let mut freq: HashMap<String, usize> = HashMap::new();
for word in &words {
if !STOPWORDS.contains(word.as_str()) && word.len() > 2 {
*freq.entry(word.clone()).or_insert(0) += 1;
}
}
// sort by frequency descending, take top_n
let mut scored: Vec<(String, usize)> = freq.into_iter().collect();
scored.sort_by(|a, b| b.1.cmp(&a.1));
scored.into_iter().take(top_n).map(|(w, _)| w).collect()
}
```

## Sentence importance ranking — extractive summary

Score each sentence by how many top-keywords it contains. Return the top-scoring sentences in their original order. This is the basis for the local Summarize mode.

```rust
pub fn rank_sentences(sentences: &[String], keywords: &[String]) -> Vec<(usize, f32)> {
sentences.iter().enumerate().map(|(i, sent)| {
let lower = sent.to_lowercase();
let score = keywords.iter()
.filter(|kw| lower.contains(kw.as_str()))
.count() as f32;
// Boost first and last sentences slightly (common in human writing)
let position_boost = if i == 0 || i == sentences.len() - 1 { 0.5 } else { 0.0 };
(i, score + position_boost)
}).collect()
}
// Returns top_n sentence indices in original order
pub fn extractive_summary(text: &str, top_n: usize) -> String {
let sentences = split_sentences(text);
let keywords = extract_keywords(text, 8);
let mut ranked = rank_sentences(&sentences, &keywords);
ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
let mut top_indices: Vec<usize> = ranked.iter().take(top_n).map(|(i,_)| *i).collect();
top_indices.sort(); // restore original order
top_indices.iter().map(|i| sentences[*i].clone()).collect::<Vec<_>>().join(" ")
}
```

## Tone scoring — returns i8 from -5 to +5

```rust
const POSITIVE_WORDS: &[&str] = &[
"good", "great", "excellent", "happy", "excited", "love",
"wonderful", "fantastic", "appreciate", "thank", "thanks",
"pleased", "glad", "delighted", "amazing", "perfect", "brilliant",
];
const NEGATIVE_WORDS: &[&str] = &[
"bad", "terrible", "urgent", "problem", "issue", "frustrated",
"angry", "disappointed", "wrong", "failed", "broken", "worst",
"horrible", "unacceptable", "complaint", "unfortunately", "regret",
];
const FRICTION_PHRASES: &[&str] = &[
"as i mentioned", "as i said", "as discussed", "per my last email",
"i thought i was clear", "i already told", "once again", "for the last time",
];
// Score: +1 per positive word, -1 per negative, -2 per friction phrase
// Clamp result to -5..5
```

## Stage 5 — Context Assembly & Prompt Building (prompt.rs)

## The TextContext struct — the complete output of the pipeline

```rust
#[derive(Debug, Clone)]
pub struct TextContext {
// Raw
pub original: String,
pub normalized: String,
pub word_count: usize,
pub char_count: usize,
// Language
pub language: LanguageContext,
// Intent
pub intent: Intent,
pub formality: u8, // 0--10
// Features
pub keywords: Vec<String>,
pub sentences: Vec<String>,
pub top_sentences: Vec<String>, // extractive summary
pub tone: i8, // -5 to +5
pub friction_phrases: Vec<String>, // detected friction phrases
pub has_urls: bool,
pub has_emails: bool,
// Routing
pub length_bucket: LengthBucket, // Short(<50w) Medium(50-200w) Long(200w+)
pub suggested_mode: String, // Pre-selected mode for UI
}
#[derive(Debug, Clone)]
pub enum LengthBucket { Short, Medium, Long }
```

## Prompt template builder — the core of the AI path

The build_prompt function takes a TextContext and a user-selected mode, and returns a complete, structured prompt string ready to send to Gemini. This is what users never have to write themselves.

  
> 💡 **KEY INSIGHT**
>
> Every prompt template must include: (1) detected language info, (2) detected intent, (3) detected formality level, (4) key topics found, (5) the specific task, (6) output constraints. This structure consistently outperforms a bare instruction by 40--60% on Gemini.

```rust
pub fn build_prompt(ctx: &TextContext, mode: &str, custom: Option<&str>) -> String {
let lang_block = build_language_block(ctx);
let context_block = build_context_block(ctx);
let task_block = build_task_block(ctx, mode, custom);
let constraint_block = build_constraint_block(ctx, mode);
format!("{lang_block} {context_block} {task_block} {constraint_block} Text to transform: "{}"",
ctx.normalized)
}
```

## Language block — example outputs

For pure Hindi (Devanagari, 95%):

```rust
The user wrote in Hindi (Devanagari script, 95% confidence).
They are communicating in their native language.
```

For Hinglish (Latin 65% + Devanagari 35%):

```rust
The user wrote in Hinglish — a natural mix of Hindi and English
(Latin script 65%, Devanagari 35%). This is their authentic voice.
Do not over-formalize or strip the mixed-language character.
```

For unknown Latin (could be French, Spanish, Italian, Portuguese):

```rust
The user wrote in a Latin-script language.
Likely French, Spanish, Italian, or Portuguese — exact language unclear.
Identify the language from the text before processing.
```

## Context block — example output

```rust
Detected context:
- Intent: Email (signals: "dear", "regards" found)
- Formality level: 3/10 (casual, several contractions)
- Tone: slightly negative (frustration signals detected)
- Key topics: project deadline, client meeting, budget approval
- Length: medium (87 words)
```

## Task blocks — one per mode

| **Mode** | **Task instruction** |
| Professional | Rewrite as professional, clear, concise text. Preserve all facts. Match formality level 8/10. No filler words. |
| Casual | Rewrite as warm, natural, conversational text. Use contractions. Sound like a real human, not a corporate email. |
| Fix | Fix all grammar, spelling, and punctuation. Do NOT change the tone, style, or voice. Minimal changes only. |
| Expand | Expand with relevant detail and context. Do not add information the user did not imply. Stay on-topic. |
| Summarize | Condense to the essential points only. Use the detected key topics as anchors. Maximum 3 sentences unless text is very long. |
| Email | Rewrite as a complete, properly structured email. Include greeting, body paragraphs, and sign-off. Match detected formality. |
| Tone Mirror | Do not rewrite. Analyze and describe how this text will be perceived by the recipient. One sentence verdict + one explanation. |
| Custom | You are an expert text transformation engine. User instruction: "\[custom\]". Fulfill this at an elite level. |

## Constraint block — always appended

```rust
Output constraints:
- Output ONLY the transformed text. No preamble, no explanation.
- Do not add information not present or clearly implied in the original.
- Preserve all proper nouns, numbers, dates, and names exactly.
- If the original text is in a non-English language, the output should
be in fluent English unless the mode specifically requires otherwise.
- Do not add a subject line unless the mode is Email.
```

## Local Transforms — No API Key Path

When no Gemini API key is present, the app must still provide value. These transforms run entirely in Rust using the TextContext already computed.

| **Mode** | **Local implementation** |
| Fix (local) | Apply a list of \~50 common grammar rules: "i " → "I ", double space → single, missing period at end, common misspellings ("teh"→"the", "recieve"→"receive", "seperate"→"separate"). Implement as a Vec\<(&str, &str)\> of find-replace pairs. \~80 rules covers 70% of common mistakes. |
| Summarize (local) | Return the top 2 sentences from extractive_summary(). Always works, zero AI needed. Quality is lower than AI but still useful. |
| Shorten (local) | Remove adverbs (very, really, quite, basically, literally, actually, honestly). Remove filler phrases ("I think", "I believe", "in my opinion", "sort of", "kind of"). Trim trailing whitespace from each sentence. |
| Tone report | Use tone score (-5 to +5) and friction_phrases to generate a canned one-line verdict. e.g.: score \< -2 → "This reads as frustrated or impatient." friction found → "Phrase 'as I mentioned' may read as passive-aggressive." score \> 2 → "Warm and positive tone." |

> ⚠️ **IMPORTANT**
>
> Do NOT attempt local translation. Without AI, translation quality is unacceptable. For non-English text with no API key, show a clear message: "Translation requires an API key. Local mode available for Fix, Shorten, and Summarize."

## Integration into lib.rs

## Pipeline entry point in mod.rs

```rust
pub fn analyze(raw_text: &str) -> TextContext {
let normalized = normalize::normalize(raw_text);
let language = language::analyze(&normalized);
let (intent, formality) = intent::classify(&normalized);
let features = features::extract(&normalized);
TextContext {
original: raw_text.to_string(),
normalized: normalized.clone(),
word_count: features.word_count,
char_count: raw_text.chars().count(),
language,
intent,
formality,
keywords: features.keywords,
sentences: features.sentences,
top_sentences: features.top_sentences,
tone: features.tone,
friction_phrases: features.friction_phrases,
has_urls: features.has_urls,
has_emails: features.has_emails,
length_bucket: features.length_bucket,
suggested_mode: intent::suggest_mode(&features, &language),
}
}
```

## Expose to frontend via Tauri command

```rust
#[tauri::command]
fn analyze_text(text: String) -> serde_json::Value {
let ctx = nlp::analyze(&text);
serde_json::json!({
"intent": format!("{:?}", ctx.intent),
"formality": ctx.formality,
"tone": ctx.tone,
"keywords": ctx.keywords,
"summary": ctx.top_sentences.join(" "),
"language": ctx.language.candidate_languages,
"is_mixed": ctx.language.is_mixed,
"suggested_mode": ctx.suggested_mode,
"word_count": ctx.word_count,
})
}
```

## Call it in the hotkey handler

```rust
// In the Alt+Shift+S handler, BEFORE showing the window:
let captured = capture::capture_text().unwrap_or_default();
let ctx = nlp::analyze(&captured);
let suggested_mode = ctx.suggested_mode.clone();
// Emit both the text AND the analysis
app.emit("text_captured", serde_json::json!({
"text": captured,
"context": {
"suggested_mode": suggested_mode,
"intent": format!("{:?}", ctx.intent),
"language": ctx.language.candidate_languages,
"is_mixed": ctx.language.is_mixed,
"tone": ctx.tone,
"formality": ctx.formality,
"keywords": ctx.keywords,
}
})).ok();
```

*End of Document 1 — Local NLP Engine*

---

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

---

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