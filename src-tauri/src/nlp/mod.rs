/// NLP pipeline public API
///
/// # Architecture
/// Stage 1: normalize   — clean unicode, extract surface metadata
/// Stage 2: language    — 18-script detection, mixed-language, RTL
/// Stage 3+: intent     — Deep Intent Engine (Layer 1+2, weighted scoring)
/// Stage 4: features    — keywords, tone, formality, all behavioral signals
/// Stage 5: prompt      — structured Gemini prompt builder
///
/// `analyze(raw)` runs all stages in order and returns `TextContext`.
/// All stages are written to be **non-panicking** — they return defaults
/// on malformed input rather than unwrapping.

pub mod normalize;
pub mod language;
pub mod intent;
pub mod features;
pub mod prompt;
pub mod data;
pub mod thread;

// Required by prompt.rs without a circular import
pub mod mod_types;

pub use mod_types::TextContext;

use serde::Serialize;

// ── LengthBucket ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub enum LengthBucket {
    Short,   // < 50 words
    Medium,  // 50–199 words
    Long,    // 200+
}

// ── Pipeline entry point ───────────────────────────────────────────────────

/// Run the full NLP pipeline on raw captured text.
/// Never panics. Returns `TextContext::empty()` if input is empty.
pub fn analyze(raw: &str) -> TextContext {
    if raw.trim().is_empty() {
        return TextContext::empty();
    }

    // Stage 1: Normalize
    let norm = normalize::normalize(raw);

    // Stage 2: Language
    let lang = language::analyze(&norm.normalized);

    // Stage 4: Features (depends on normalized text)
    let feats = features::extract(&norm.normalized);

    // Stage 3: Deep Intent (uses features + language)
    let intent_result = intent::classify_deep(&norm.normalized, &feats, &lang);

    // Suggested mode string
    let suggested_mode = intent::suggest_mode(&intent_result, &feats);

    let length_bucket = match norm.word_count {
        0..=49   => LengthBucket::Short,
        50..=199 => LengthBucket::Medium,
        _        => LengthBucket::Long,
    };

    TextContext {
        original: raw.to_string(),
        normalized: norm.normalized,
        word_count: norm.word_count,
        char_count: norm.char_count,
        language: lang,
        intent_result,
        formality: feats.formality,
        keywords: feats.keywords,
        sentences: feats.sentences,
        top_sentences: feats.top_sentences,
        tone: feats.tone,
        friction_phrases: feats.friction_phrases,
        has_urls: norm.has_urls,
        has_emails: norm.has_emails,
        length_bucket,
        suggested_mode,
        detected_entities: feats.detected_entities,
        emoji_count: feats.emoji_count,
        contraction_rate: feats.contraction_rate,
        avg_sentence_len: feats.avg_sentence_len,
    }
}
