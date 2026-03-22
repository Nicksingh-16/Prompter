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