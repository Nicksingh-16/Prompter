/// Stage 2 — Script & Language Detection
///
/// O(n) Unicode block scan. No ML, no dictionary, no external data.
/// Determines primary/secondary script, mixed-language status, and RTL direction.

use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Script {
    Latin,      // English, Spanish, etc.
    Devanagari, // Hindi, Marathi, etc.
    Arabic,     // Arabic, Persian, Urdu
    Cyrillic,   // Russian, etc.
    CJK,        // Chinese (Hanzi)
    Hiragana,   // Japanese
    Katakana,
    Hangul,     // Korean
    Tamil,
    Telugu,
    Gujarati,
    Bengali,
    Kannada,
    Malayalam,
    Punjabi,
    Thai,
    Greek,
    Hebrew,
    Punctuation, // symbols, spaces, digits
    Unknown,
}

impl Script {
}

#[derive(Debug, Clone, Serialize)]
pub struct LanguageContext {
    pub primary_script: Script,
    pub primary_pct: f32,          // 0.0–1.0
    pub secondary_script: Option<Script>,
    pub secondary_pct: f32,
    pub is_mixed: bool,            // true when secondary_pct > 0.15
    pub is_rtl: bool,              // true for Arabic, Hebrew
    pub language_family: &'static str,     // e.g. "Indic"
    pub candidate_languages: &'static str, // e.g. "Hindi, Marathi, or Nepali"
    pub needs_romanization_hint: bool,
}

impl Default for LanguageContext {
    fn default() -> Self {
        LanguageContext {
            primary_script: Script::Latin,
            primary_pct: 1.0,
            secondary_script: None,
            secondary_pct: 0.0,
            is_mixed: false,
            is_rtl: false,
            language_family: "Latin",
            candidate_languages: "a European language (English, French, Spanish, or similar)",
            needs_romanization_hint: false,
        }
    }
}

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
        // ASCII printable symbols: space through '@' (0x0020–0x0040 = before 'A').
        // Note: 0x0041–0x024F is covered by the Latin arm above.
        0x0020..=0x0040 => Script::Punctuation,
        _ => Script::Unknown,
    }
}

/// O(n) scan: count characters per script block, compute ratios, build LanguageContext.
pub fn analyze(text: &str) -> LanguageContext {
    if text.is_empty() {
        return LanguageContext::default();
    }

    let mut counts: HashMap<Script, usize> = HashMap::new();
    let mut total = 0usize;

    for c in text.chars() {
        let s = detect_script(c);
        if !matches!(s, Script::Punctuation | Script::Unknown) {
            *counts.entry(s).or_insert(0) += 1;
            total += 1;
        }
    }

    if total == 0 {
        return LanguageContext::default();
    }

    let mut sorted: Vec<(Script, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let primary_script = sorted[0].0.clone();
    let primary_pct = sorted[0].1 as f32 / total as f32;

    let secondary = sorted.get(1).map(|(s, n)| (s.clone(), *n as f32 / total as f32));
    let secondary_script = secondary.as_ref().map(|(s, _)| s.clone());
    let secondary_pct = secondary.as_ref().map(|(_, p)| *p).unwrap_or(0.0);

    let is_mixed_script = primary_pct < 0.85 && secondary_pct > 0.15;

    // Detect romanized Hinglish — Hindi written in Latin script.
    // These words are all-Latin so script detection alone can't catch them.
    // We match against a compact list of high-frequency Hindi words that
    // almost never appear in native English text.
    let is_romanized_hinglish = if matches!(primary_script, Script::Latin) {
        let lower = text.to_lowercase();
        let hinglish_markers: &[&str] = &[
            "mujhe", "muje", "chahiye", "chahie", "abhi", "nahi", "nahin",
            "karna", "karo", "karta", "karti", "karke", "karna",
            "acha", "accha", "theek", "thik", "bilkul",
            "bhai", "yaar", "bhaiya", "didi",
            "iske", "uske", "inke", "unke", "isme", "usme",
            "samajh", "samjha", "batao", "batana", "bata",
            "matlab", "waise", "lekin", "aur", "toh", "tho",
            "hai", "hain", "tha", "thi", "the",
            "kyun", "kya", "kaise", "kaisa", "kitna",
            "bahut", "thoda", "thodi", "bohot",
            "apna", "apni", "apne", "mera", "meri", "mere",
            "tumhara", "tumhari", "aapka", "aapki",
            "dekho", "dekh", "suno", "sun",
            "hoga", "hogi", "hoge", "rahega", "rahegi",
        ];
        let word_count = lower.split_whitespace().count();
        let matches = hinglish_markers.iter()
            .filter(|&&m| lower.split_whitespace().any(|w| w.trim_matches(|c: char| !c.is_alphabetic()) == m))
            .count();
        // Need at least 1 match in short text, 2 in longer text
        matches >= 1 && (word_count <= 12 || matches >= 2)
    } else {
        false
    };

    let is_mixed = is_mixed_script || is_romanized_hinglish;
    let is_rtl = matches!(primary_script, Script::Arabic | Script::Hebrew);

    let (family, candidates, romanization_hint) = if is_romanized_hinglish && matches!(primary_script, Script::Latin) {
        ("Indic-Romanized", "Hindi (romanized / Hinglish)", true)
    } else {
        script_metadata(&primary_script)
    };

    LanguageContext {
        primary_script,
        primary_pct,
        secondary_script,
        secondary_pct,
        is_mixed,
        is_rtl,
        language_family: family,
        candidate_languages: candidates,
        needs_romanization_hint: romanization_hint,
    }
}

fn script_metadata(s: &Script) -> (&'static str, &'static str, bool) {
    match s {
        Script::Latin => (
            "Latin",
            "a European language (English, French, Spanish, German, Italian, Portuguese, Dutch, Polish, or similar)",
            false,
        ),
        Script::Devanagari => ("Indic", "Hindi, Marathi, or Nepali", true),
        Script::Arabic => ("Semitic", "Arabic, Urdu, or Farsi", true),
        Script::Cyrillic => ("Cyrillic", "Russian, Ukrainian, or Bulgarian", true),
        Script::CJK => ("CJK", "Chinese (Mandarin or Cantonese)", true),
        Script::Hiragana | Script::Katakana => ("CJK", "Japanese", true),
        Script::Hangul => ("CJK", "Korean", true),
        Script::Tamil => ("Indic", "Tamil", true),
        Script::Telugu => ("Indic", "Telugu", true),
        Script::Gujarati => ("Indic", "Gujarati", true),
        Script::Bengali => ("Indic", "Bengali or Assamese", true),
        Script::Kannada => ("Indic", "Kannada", true),
        Script::Malayalam => ("Indic", "Malayalam", true),
        Script::Punjabi => ("Indic", "Punjabi", true),
        Script::Thai => ("Southeast Asian", "Thai", true),
        Script::Greek => ("Greek", "Greek", true),
        Script::Hebrew => ("Semitic", "Hebrew", true),
        _ => ("Unknown", "an unrecognized language", false),
    }
}