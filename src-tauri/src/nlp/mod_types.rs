/// TextContext — the single unified output struct of the NLP pipeline.
/// Lives in mod_types.rs to avoid the circular import between mod.rs and prompt.rs.

use serde::Serialize;
use super::language::LanguageContext;
use super::intent::IntentResult;
use super::LengthBucket;

#[derive(Debug, Clone, Serialize)]
pub struct TextContext {
    /// Raw input as captured from the clipboard
    pub original: String,
    /// Unicode-normalized version used by downstream stages
    pub normalized: String,
    pub word_count: usize,
    pub char_count: usize,
    /// Script detection + mixed-language info
    pub language: LanguageContext,
    /// Deep Intent result with ranked candidates
    pub intent_result: IntentResult,
    /// 0–10 formality score
    pub formality: i32,
    /// Top N significant keywords (TF-IDF approx, stopword-filtered)
    pub keywords: Vec<String>,
    /// All detected sentences
    pub sentences: Vec<String>,
    /// Top 3 sentences for extractive summary
    pub top_sentences: Vec<String>,
    /// -5 (very negative) to +5 (very positive)
    pub tone: i32,
    /// Detected passive-aggressive friction phrases
    pub friction_phrases: Vec<String>,
    pub has_urls: bool,
    pub has_emails: bool,
    pub length_bucket: LengthBucket,
    /// Suggested App mode string ("Email", "Summarize", etc.)
    pub suggested_mode: String,
    /// Detected people, projects, or companies for context memory
    pub detected_entities: Vec<(String, String)>, // (name, type)
    /// Voice DNA signals
    pub emoji_count: usize,
    pub contraction_rate: f32,
    pub avg_sentence_len: f32,
}

impl TextContext {
    pub fn empty() -> Self {
        use super::intent::IntentResult;
        TextContext {
            original: String::new(),
            normalized: String::new(),
            word_count: 0,
            char_count: 0,
            language: LanguageContext::default(),
            intent_result: IntentResult::default_general(),
            formality: 5,
            keywords: vec![],
            sentences: vec![],
            top_sentences: vec![],
            tone: 0,
            friction_phrases: vec![],
            has_urls: false,
            has_emails: false,
            length_bucket: LengthBucket::Short,
            suggested_mode: "Fix".into(),
            detected_entities: vec![],
            emoji_count: 0,
            contraction_rate: 0.0,
            avg_sentence_len: 0.0,
        }
    }
}
