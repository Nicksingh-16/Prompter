/// Stage 3 + Deep Intent Engine (Doc 3)
///
/// Layer 1: Multi-signal extraction → SignalVector
/// Layer 2: Weighted scoring matrix → IntentResult with ranked confidence
///
/// Layers 3 (AI classifier) and 4 (adaptive weights) are wired in lib.rs.

use std::collections::HashMap;
use serde::Serialize;
use super::language::LanguageContext;
use super::features::FeaturesOutput;

// ── Intent enum ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Intent {
    Email,
    Chat,
    Prompt,
    Knowledge,
    Report,
    Social,
    General,
}


// ── Keyword tables ─────────────────────────────────────────────────────────

const EMAIL_KEYWORDS: &[&str] = &[
    "dear", "hi", "hello", "regards", "sincerely", "subject:", "please",
    "kindly", "attached", "as discussed", "following up", "pursuant",
    "re:", "fwd:", "cc:", "bcc:", "best regards", "warm regards",
    "hope this", "find attached", "please find", "looking forward",
];
const CHAT_KEYWORDS: &[&str] = &[
    "lol", "btw", "tbh", "ngl", "gonna", "wanna", "kinda", "yep", "nope",
    "hey", "sup", "brb", "omg", "wtf", "haha", "lmao", "fr", "imo", "tbf",
    "rn", "idk", "irl", "fyi", "thx", "ty", "np",
];
const PROMPT_KEYWORDS: &[&str] = &[
    "act as", "role:", "persona", "system prompt", "instruction:",
    "respond as", "write a prompt", "create a prompt", "prompt engineering",
];
const KNOWLEDGE_KEYWORDS: &[&str] = &[
    "teach", "learn", "explain", "how to", "guide", "steps",
    "walkthrough", "tutorial", "instruction", "help me understand",
    "scares me", "don't know", "rookie", "senior", "onsite",
];
const REPORT_KEYWORDS: &[&str] = &[
    "summary", "update", "status", "report", "findings", "analysis",
    "conclusion", "recommendation", "overview", "results", "q1", "q2",
    "q3", "q4", "kpi", "metrics", "performance", "revenue", "churn",
    "quarterly", "monthly", "weekly",
];
const SOCIAL_KEYWORDS: &[&str] = &[
    "tweet", "post", "thread", "caption", "hashtag", "viral", "followers",
    "linkedin", "instagram", "share", "reel", "story", "hook", "engage",
    "#", "like", "comment",
];

pub const GREETING_WORDS: &[&str] = &[
    "dear", "hi", "hello", "hey", "greetings", "good morning",
    "good afternoon", "good evening", "to whom",
];

const CLOSER_WORDS: &[&str] = &[
    "regards", "sincerely", "cheers", "thanks", "thank you", "best",
    "warm", "yours", "respectfully", "cordially", "take care",
    "talk soon", "looking forward",
];

// ── Layer 1: SignalVector ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SignalVector {
    // Surface
    pub keyword_scores: HashMap<Intent, f32>,
    pub greeting_found: bool,
    pub greeting_position: Option<usize>, // char offset
    pub signoff_found: bool,
    pub at_symbol: bool,
    pub subject_line: bool,
    // Structural
    pub has_bullet_points: bool,
    pub has_numbered_list: bool,
    pub paragraph_count: usize,
    pub has_salutation_structure: bool,
    // Behavioral (from FeaturesOutput)
    pub sentence_count: usize,
    pub avg_sentence_len: f32,
    pub question_count: usize,
    pub exclamation_count: usize,
    pub fragment_count: usize,
    pub ellipsis_count: usize,
    pub word_count: usize,
    // Linguistic (from FeaturesOutput)
    pub formality: i32,
    pub contraction_rate: f32,
    pub hedge_word_count: usize,
    pub urgency_word_count: usize,
    pub passive_voice_count: usize,
    pub imperative_count: usize,
}

impl Default for SignalVector {
    fn default() -> Self {
        SignalVector {
            keyword_scores: HashMap::new(),
            greeting_found: false,
            greeting_position: None,
            signoff_found: false,
            at_symbol: false,
            subject_line: false,
            has_bullet_points: false,
            has_numbered_list: false,
            paragraph_count: 1,
            has_salutation_structure: false,
            sentence_count: 1,
            avg_sentence_len: 0.0,
            question_count: 0,
            exclamation_count: 0,
            fragment_count: 0,
            ellipsis_count: 0,
            word_count: 0,
            formality: 5,
            contraction_rate: 0.0,
            hedge_word_count: 0,
            urgency_word_count: 0,
            passive_voice_count: 0,
            imperative_count: 0,
        }
    }
}

pub fn position_weight(offset: usize, total_len: usize) -> f32 {
    if total_len == 0 {
        return 1.0;
    }
    let pct = offset as f32 / total_len as f32;
    if pct < 0.05 {
        4.0 // first 5% — very strong signal
    } else if pct < 0.15 {
        2.5 // first 15% — strong
    } else if pct < 0.30 {
        1.5 // first 30% — moderate
    } else if pct > 0.85 {
        2.0 // last 15% — sign-off zone
    } else {
        1.0 // middle — neutral
    }
}

fn score_keywords(text: &str) -> HashMap<Intent, f32> {
    let lower = text.to_lowercase();
    let len = lower.len();
    let mut scores: HashMap<Intent, f32> = HashMap::new();

    macro_rules! scan {
        ($table:expr, $intent:expr, $base:expr) => {
            for kw in $table {
                if let Some(offset) = lower.find(kw) {
                    let w = position_weight(offset, len);
                    *scores.entry($intent.clone()).or_insert(0.0) += $base * w;
                }
            }
        };
    }

    scan!(EMAIL_KEYWORDS,  Intent::Email,  1.0);
    scan!(CHAT_KEYWORDS,   Intent::Chat,   1.0);
    scan!(PROMPT_KEYWORDS, Intent::Prompt, 1.0);
    scan!(REPORT_KEYWORDS, Intent::Report, 1.0);
    scan!(SOCIAL_KEYWORDS, Intent::Social, 1.0);
    scan!(KNOWLEDGE_KEYWORDS, Intent::Knowledge, 1.2);

    scores
}

pub fn detect_salutation_structure(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 3 {
        return false;
    }
    let first = lines[0].trim().to_lowercase();
    let last = lines[lines.len() - 1].trim().to_lowercase();
    let has_opener = GREETING_WORDS.iter().any(|g| first.starts_with(g));
    let has_closer = CLOSER_WORDS
        .iter()
        .any(|c| last.starts_with(c) || last.ends_with(c));
    has_opener && has_closer && lines.len() >= 3
}



pub fn extract_signals(text: &str, features: &FeaturesOutput, _lang: &LanguageContext) -> SignalVector {
    let lower = text.to_lowercase();

    let keyword_scores = score_keywords(text);

    // Greeting detection
    let greeting_position = GREETING_WORDS
        .iter()
        .filter_map(|g| lower.find(g))
        .min();
    let greeting_found = greeting_position.is_some();

    // Sign-off detection
    let signoff_found = CLOSER_WORDS.iter().any(|c| lower.contains(c));

    let at_symbol = text.contains('@');
    let subject_line = lower.starts_with("subject:")
        || lower.starts_with("re:")
        || lower.starts_with("fwd:");

    let has_salutation_structure = detect_salutation_structure(text);

    SignalVector {
        keyword_scores,
        greeting_found,
        greeting_position,
        signoff_found,
        at_symbol,
        subject_line,
        has_bullet_points: features.has_bullet_points,
        has_numbered_list: features.has_numbered_list,
        paragraph_count: features.paragraph_count,
        has_salutation_structure,
        sentence_count: features.sentence_count,
        avg_sentence_len: features.avg_sentence_len,
        question_count: features.question_count,
        exclamation_count: features.exclamation_count,
        fragment_count: features.fragment_count,
        ellipsis_count: features.ellipsis_count,
        word_count: features.word_count,
        formality: features.formality,
        contraction_rate: features.contraction_rate,
        hedge_word_count: features.hedge_word_count,
        urgency_word_count: features.urgency_word_count,
        passive_voice_count: features.passive_voice_count,
        imperative_count: features.imperative_count,
    }
}

// ── Layer 2: Weighted scoring matrix ──────────────────────────────────────

fn score_email(sv: &SignalVector) -> f32 {
    let mut score = 0.0_f32;
    if sv.has_salutation_structure { score += 3.0; }
    if sv.subject_line             { score += 2.5; }
    if sv.signoff_found            { score += 1.5; }
    if sv.at_symbol                { score += 1.0; }
    if sv.greeting_found {
        let pw = sv.greeting_position
            .map(|p| if p < 5 { 2.0 } else { 0.5 })
            .unwrap_or(0.5);
        score += 1.5 * pw;
    }
    score += sv.keyword_scores.get(&Intent::Email).copied().unwrap_or(0.0);
    if sv.paragraph_count >= 2     { score += 0.5; }
    if sv.formality >= 6           { score += 0.5; }
    if sv.urgency_word_count > 0   { score += 0.3; }
    // Penalties
    if sv.fragment_count > 3       { score -= 1.0; }
    if sv.exclamation_count > 4    { score -= 0.5; }
    score.max(0.0)
}

fn score_chat(sv: &SignalVector) -> f32 {
    let mut score = 0.0_f32;
    score += sv.keyword_scores.get(&Intent::Chat).copied().unwrap_or(0.0);
    if sv.fragment_count > 2            { score += 1.5; }
    if sv.formality <= 3                { score += 1.5; }
    if sv.exclamation_count > 2         { score += 0.8; }
    if sv.ellipsis_count > 1            { score += 0.6; }
    if sv.avg_sentence_len < 8.0        { score += 1.0; }
    if sv.sentence_count <= 3           { score += 0.5; }
    if sv.has_salutation_structure      { score -= 2.0; }
    if sv.contraction_rate > 0.3        { score += 0.8; }
    if sv.word_count < 30               { score += 0.5; }
    score.max(0.0)
}

fn score_prompt(sv: &SignalVector) -> f32 {
    let mut score = 0.0_f32;
    score += sv.keyword_scores.get(&Intent::Prompt).copied().unwrap_or(0.0);
    if sv.imperative_count > 0          { score += 2.0; }
    if sv.question_count == 0           { score += 0.5; }
    if sv.hedge_word_count > 1          { score += 0.8; }
    if sv.has_salutation_structure      { score -= 2.0; }
    if sv.has_bullet_points             { score += 1.0; }
    score.max(0.0)
}

fn score_knowledge(sv: &SignalVector) -> f32 {
    let mut score = 0.0_f32;
    score += sv.keyword_scores.get(&Intent::Knowledge).copied().unwrap_or(0.0);
    if sv.question_count > 0            { score += 1.0; }
    if sv.word_count > 100              { score += 1.0; }
    if sv.paragraph_count >= 2          { score += 0.5; }
    score.max(0.0)
}

fn score_social(sv: &SignalVector) -> f32 {
    let mut score = 0.0_f32;
    score += sv.keyword_scores.get(&Intent::Social).copied().unwrap_or(0.0);
    if sv.word_count < 60               { score += 1.0; }
    if sv.word_count < 30               { score += 1.0; } // stacks
    if sv.exclamation_count > 1         { score += 0.8; }
    if sv.has_salutation_structure      { score -= 2.5; }
    if sv.formality <= 4                { score += 0.5; }
    score.max(0.0)
}

fn score_report(sv: &SignalVector) -> f32 {
    let mut score = 0.0_f32;
    score += sv.keyword_scores.get(&Intent::Report).copied().unwrap_or(0.0);
    if sv.has_bullet_points             { score += 1.5; }
    if sv.has_numbered_list             { score += 1.5; }
    if sv.word_count > 150              { score += 1.0; }
    if sv.paragraph_count >= 3          { score += 0.8; }
    if sv.formality >= 7                { score += 1.0; }
    if sv.passive_voice_count > 1       { score += 0.5; }
    score.max(0.0)
}

// ── IntentResult output structs ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct IntentCandidate {
    pub intent: Intent,
    pub confidence: f32,        // 0.0–1.0 normalized
    pub label: &'static str,    // "Email", "Casual chat", etc.
    pub mode: &'static str,     // maps to App mode
    pub reason: &'static str,   // one-liner why this was suggested
}

#[derive(Debug, Clone, Serialize)]
pub struct IntentResult {
    pub primary: IntentCandidate,
    pub alternatives: Vec<IntentCandidate>,
    pub overall_confidence: f32, // primary.conf - second.conf
}

impl IntentResult {
    pub fn default_general() -> Self {
        IntentResult {
            primary: make_candidate(&Intent::General, 1.0),
            alternatives: vec![],
            overall_confidence: 1.0,
        }
    }
}

fn make_candidate(intent: &Intent, confidence: f32) -> IntentCandidate {
    let (label, mode, reason) = candidate_meta(intent);
    IntentCandidate {
        intent: intent.clone(),
        confidence,
        label,
        mode,
        reason,
    }
}

fn candidate_meta(intent: &Intent) -> (&'static str, &'static str, &'static str) {
    match intent {
        Intent::Email   => ("Email",        "Professional", "greeting + sign-off detected"),
        Intent::Chat    => ("Casual chat",  "Casual",       "short, casual, informal language"),
        Intent::Prompt    => ("AI Prompt",    "Prompt",       "technical prompt drafting instructions"),
        Intent::Knowledge => ("Knowledge",   "Knowledge",    "request for teaching or guidance"),
        Intent::Report    => ("Report",       "Summarize",    "structured, formal, long text"),
        Intent::Social    => ("Social post",  "Casual",       "very short, punchy, social signals"),
        Intent::General   => ("General",      "Professional", "no strong pattern detected"),
    }
}

pub fn normalize_scores(raw: HashMap<Intent, f32>) -> IntentResult {
    let total: f32 = raw.values().sum();
    if total < 0.01 {
        return IntentResult::default_general();
    }

    let mut normalized: Vec<(Intent, f32)> = raw
        .iter()
        .map(|(k, v)| (k.clone(), v / total))
        .collect();
    normalized.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let primary_conf = normalized[0].1;
    let second_conf = normalized.get(1).map(|(_, c)| *c).unwrap_or(0.0);

    let alternatives: Vec<IntentCandidate> = normalized[1..]
        .iter()
        .filter(|(_, c)| *c > 0.15)
        .take(2)
        .map(|(i, c)| make_candidate(i, *c))
        .collect();

    IntentResult {
        primary: make_candidate(&normalized[0].0, primary_conf),
        alternatives,
        overall_confidence: (primary_conf - second_conf).clamp(0.0, 1.0),
    }
}

pub fn should_fire_ai_classifier(result: &IntentResult) -> bool {
    result.overall_confidence < 0.30
        || result.primary.confidence < 0.50
        || result.alternatives.len() >= 2
}

// ── Public entry point ─────────────────────────────────────────────────────

/// Full Layer 1 + Layer 2 pipeline. Pure function — no DB, no network.
pub fn classify_deep(
    text: &str,
    features: &FeaturesOutput,
    lang: &LanguageContext,
) -> IntentResult {
    let sv = extract_signals(text, features, lang);

    let mut raw_scores: HashMap<Intent, f32> = HashMap::new();
    raw_scores.insert(Intent::Email,   score_email(&sv));
    raw_scores.insert(Intent::Chat,    score_chat(&sv));
    raw_scores.insert(Intent::Prompt,  score_prompt(&sv));
    raw_scores.insert(Intent::Social,  score_social(&sv));
    raw_scores.insert(Intent::Knowledge, score_knowledge(&sv));
    raw_scores.insert(Intent::Report,  score_report(&sv));
    // General always gets a small baseline so it appears in alternatives
    raw_scores.insert(Intent::General, 0.5);

    normalize_scores(raw_scores)
}



/// Map intent + context signals to a suggested App mode string.
pub fn suggest_mode(intent_result: &IntentResult, features: &FeaturesOutput) -> String {
    match &intent_result.primary.intent {
        Intent::Email => {
            if features.formality >= 6 { "Professional".into() } else { "Casual".into() }
        }
        Intent::Chat    => "Casual".into(),
        Intent::Prompt  => "Prompt".into(),
        Intent::Knowledge => "Knowledge".into(),
        Intent::Report  => {
            if features.word_count > 200 { "Summarize".into() } else { "Professional".into() }
        }
        Intent::Social  => "Casual".into(),
        Intent::General => {
            if features.tone < -1 { "Fix".into() }
            else if features.word_count > 150 { "Summarize".into() }
            else { "Fix".into() }
        }
    }
}
