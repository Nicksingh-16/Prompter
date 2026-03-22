/// Stage 4 — Feature Extraction
///
/// Sentence splitting, keyword TF-IDF approximation, extractive summary,
/// tone scoring, and all behavioral/structural signals used by the Deep Intent Engine.

use std::collections::HashMap;
use super::data::stopwords::STOPWORDS;
use super::intent::GREETING_WORDS;

// ── Signal constants ───────────────────────────────────────────────────────

const POSITIVE_WORDS: &[&str] = &[
    "good", "great", "excellent", "happy", "excited", "love", "wonderful",
    "fantastic", "appreciate", "thank", "thanks", "pleased", "glad",
    "delighted", "amazing", "perfect", "brilliant", "awesome", "outstanding",
    "superb", "enjoy", "beautiful", "helpful", "success", "effective",
];

const NEGATIVE_WORDS: &[&str] = &[
    "bad", "terrible", "urgent", "problem", "issue", "frustrated", "angry",
    "disappointed", "wrong", "failed", "broken", "worst", "horrible",
    "unacceptable", "complaint", "unfortunately", "regret", "difficult",
    "impossible", "fail", "error", "mistake", "concern", "worried",
];

const FRICTION_PHRASES: &[&str] = &[
    "as i mentioned",
    "as i said",
    "as discussed",
    "per my last email",
    "i thought i was clear",
    "i already told",
    "once again",
    "for the last time",
    "as previously stated",
    "i have already",
];

const CONTRACTION_MARKERS: &[&str] = &[
    "n't", "'re", "'ve", "'ll", "'m", "'d", "won't", "can't", "don't",
    "isn't", "aren't", "wasn't", "weren't", "haven't", "hasn't",
    "hadn't", "won't", "wouldn't", "couldn't", "shouldn't", "didn't",
    "doesn't", "i'm", "you're", "we're", "they're", "i've", "you've",
    "we've", "they've", "i'll", "you'll", "he'll", "she'll", "we'll",
];

const HEDGE_WORDS: &[&str] = &[
    "maybe", "perhaps", "possibly", "probably", "i think", "i believe",
    "i feel", "i guess", "sort of", "kind of", "somewhat", "relatively",
    "seems", "appears", "apparently", "might", "could be", "not sure",
];

const URGENCY_WORDS: &[&str] = &[
    "asap", "urgent", "urgently", "immediately", "deadline", "critical",
    "priority", "rush", "time-sensitive", "by eod", "by end of day",
    "as soon as possible", "right away", "right now", "emergency",
];

const PASSIVE_INDICATORS: &[&str] = &[
    " was ", " were ", "is being ", "are being ", "has been ", "have been ",
    "had been ", "will be ", "being ",
];

const IMPERATIVE_STARTERS: &[&str] = &[
    "write", "create", "make", "generate", "help", "draft", "compose",
    "build", "describe", "explain", "list", "give", "find", "show",
    "tell", "provide", "check", "review", "fix", "add", "remove",
    "update", "change", "translate", "summarize", "convert", "send",
    "forward", "schedule", "prepare", "ensure", "verify", "confirm",
    "please", "kindly",
];

// Abbreviations that should NOT trigger a sentence split
const ABBREVS: &[&str] = &[
    "Mr.", "Mrs.", "Ms.", "Dr.", "Prof.", "Sr.", "Jr.", "Inc.", "Ltd.",
    "Corp.", "etc.", "vs.", "i.e.", "e.g.", "Jan.", "Feb.", "Mar.",
    "Apr.", "Jun.", "Jul.", "Aug.", "Sep.", "Oct.", "Nov.", "Dec.",
    "approx.", "dept.", "est.",
];

// ── Output struct ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FeaturesOutput {
    pub word_count: usize,
    pub sentences: Vec<String>,
    pub top_sentences: Vec<String>,
    pub keywords: Vec<String>,
    pub tone: i32,                    // -5 to +5
    pub friction_phrases: Vec<String>,
    pub formality: i32,               // 0–10
    pub contraction_rate: f32,       // 0.0–1.0
    pub sentence_count: usize,
    pub avg_sentence_len: f32,
    pub question_count: usize,
    pub exclamation_count: usize,
    pub fragment_count: usize,
    pub ellipsis_count: usize,
    pub hedge_word_count: usize,
    pub urgency_word_count: usize,
    pub passive_voice_count: usize,
    pub imperative_count: usize,
    pub has_bullet_points: bool,
    pub has_numbered_list: bool,
    pub paragraph_count: usize,
    pub detected_entities: Vec<(String, String)>,
}

impl Default for FeaturesOutput {
    fn default() -> Self {
        FeaturesOutput {
            word_count: 0,
            sentences: vec![],
            top_sentences: vec![],
            keywords: vec![],
            tone: 0,
            friction_phrases: vec![],
            formality: 5,
            contraction_rate: 0.0,
            sentence_count: 0,
            avg_sentence_len: 0.0,
            question_count: 0,
            exclamation_count: 0,
            fragment_count: 0,
            ellipsis_count: 0,
            hedge_word_count: 0,
            urgency_word_count: 0,
            passive_voice_count: 0,
            imperative_count: 0,
            has_bullet_points: false,
            has_numbered_list: false,
            paragraph_count: 1,
            detected_entities: vec![],
        }
    }
}

// ── Main entry point ───────────────────────────────────────────────────────

pub fn extract(text: &str) -> FeaturesOutput {
    if text.is_empty() {
        return FeaturesOutput::default();
    }

    let lower = text.to_lowercase();
    let words: Vec<&str> = text.split_whitespace().collect();
    let word_count = words.len();

    // Structural signals from lines
    let lines: Vec<&str> = text.lines().collect();
    let has_bullet_points = lines.iter().any(|l| {
        let t = l.trim();
        t.starts_with("- ") || t.starts_with("* ") || t.starts_with("• ") || t.starts_with("→ ")
    });
    let has_numbered_list = lines.iter().any(|l| {
        let t = l.trim();
        t.len() > 2 && t.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
            && (t.starts_with("1.") || t.starts_with("2.")
                || t.chars().nth(1).map(|c| c == '.' || c == ')').unwrap_or(false))
    });

    // Paragraph count: blank-line separated blocks
    let paragraph_count = text
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .count()
        .max(1);

    // Sentence splitting
    let sentences = split_sentences(text);
    let sentence_count = sentences.len();
    let avg_sentence_len = if sentence_count == 0 {
        0.0
    } else {
        word_count as f32 / sentence_count as f32
    };

    // Question / exclamation / ellipsis / fragment counts
    let question_count = sentences.iter().filter(|s| s.trim_end().ends_with('?')).count();
    let exclamation_count = sentences.iter().filter(|s| s.trim_end().ends_with('!')).count();
    let ellipsis_count = text.matches("...").count() + text.matches('\u{2026}').count();
    // Fragment: sentence with < 4 words (approximate — no verb parsing)
    let fragment_count = sentences
        .iter()
        .filter(|s| s.split_whitespace().count() < 4)
        .count();

    // Linguistic signals
    let contraction_count = CONTRACTION_MARKERS
        .iter()
        .filter(|m| lower.contains(*m))
        .count();
    let contraction_rate = if word_count == 0 {
        0.0
    } else {
        (contraction_count as f32 / word_count as f32).min(1.0)
    };

    let hedge_word_count = HEDGE_WORDS.iter().filter(|h| lower.contains(*h)).count();
    let urgency_word_count = URGENCY_WORDS.iter().filter(|u| lower.contains(*u)).count();

    // Passive voice: crude scan for "was/were/has been" + -ed pattern
    let passive_voice_count = PASSIVE_INDICATORS
        .iter()
        .filter(|p| lower.contains(*p))
        .count();

    // Imperatives: sentences that START with a known imperative starter
    let imperative_count = sentences
        .iter()
        .filter(|s| {
            let sl = s.trim().to_lowercase();
            IMPERATIVE_STARTERS.iter().any(|imp| sl.starts_with(imp))
        })
        .count();

    // Keywords (stopword-filtered TF-IDF approximation)
    let keywords = extract_keywords(text, 10);

    // Extractive summary (top 3 sentences)
    let top_sentences = extractive_summary_sentences(&sentences, &keywords, 3);

    // Tone score
    let (tone, friction_phrases) = compute_tone(&lower);

    // Formality score
    let formality = compute_formality(
        word_count,
        &sentences,
        contraction_count,
        exclamation_count,
        has_bullet_points,
        &lower,
    );

    let detected_entities = extract_entities(text, &sentences, &lower);

    FeaturesOutput {
        word_count,
        sentences,
        top_sentences,
        keywords,
        tone,
        friction_phrases,
        formality,
        contraction_rate,
        sentence_count,
        avg_sentence_len,
        question_count,
        exclamation_count,
        fragment_count,
        ellipsis_count,
        hedge_word_count,
        urgency_word_count,
        passive_voice_count,
        imperative_count,
        has_bullet_points,
        has_numbered_list,
        paragraph_count,
        detected_entities,
    }
}

pub fn split_sentences(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![];
    }
    let placeholder = "\x01";
    let mut protected = text.to_string();
    for abbr in ABBREVS {
        protected = protected.replace(abbr, &abbr.replace('.', placeholder));
    }
    let mut sentences: Vec<String> = Vec::new();
    let mut start = 0;
    let chars: Vec<char> = protected.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        let c = chars[i];
        if matches!(c, '.' | '!' | '?') {
            let mut j = i + 1;
            while j < len && chars[j] == ' ' {
                j += 1;
            }
            let is_sentence_boundary = j >= len || (j > i + 1 && chars[j].is_uppercase());
            if is_sentence_boundary {
                let raw: String = chars[start..=i].iter().collect();
                let restored = raw.replace(placeholder, ".");
                let trimmed = restored.trim().to_string();
                if !trimmed.is_empty() {
                    sentences.push(trimmed);
                }
                start = j;
                i = j;
                continue;
            }
        }
        i += 1;
    }
    if start < len {
        let raw: String = chars[start..].iter().collect();
        let restored = raw.replace(placeholder, ".");
        let trimmed = restored.trim().to_string();
        if !trimmed.is_empty() {
            sentences.push(trimmed);
        }
    }
    if sentences.is_empty() {
        vec![text.trim().to_string()]
    } else {
        sentences
    }
}

pub fn extract_keywords(text: &str, top_n: usize) -> Vec<String> {
    let mut freq: HashMap<String, usize> = HashMap::new();
    for word in text.split_whitespace() {
        let clean: String = word
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>()
            .to_lowercase();
        if clean.len() > 2 && !STOPWORDS.contains(&clean.as_str()) {
            *freq.entry(clean).or_insert(0) += 1;
        }
    }
    let mut scored: Vec<(String, usize)> = freq.into_iter().collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.into_iter().take(top_n).map(|(w, _)| w).collect()
}

fn rank_sentences(sentences: &[String], keywords: &[String]) -> Vec<(usize, f32)> {
    sentences
        .iter()
        .enumerate()
        .map(|(i, sent)| {
            let lower = sent.to_lowercase();
            let kw_score = keywords
                .iter()
                .filter(|kw| lower.contains(kw.as_str()))
                .count() as f32;
            let pos_boost = if i == 0 || i + 1 == sentences.len() {
                0.5
            } else {
                0.0
            };
            (i, kw_score + pos_boost)
        })
        .collect()
}

fn extractive_summary_sentences(
    sentences: &[String],
    keywords: &[String],
    top_n: usize,
) -> Vec<String> {
    if sentences.is_empty() {
        return vec![];
    }
    let mut ranked = rank_sentences(sentences, keywords);
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut indices: Vec<usize> = ranked.iter().take(top_n).map(|(i, _)| *i).collect();
    indices.sort_unstable();
    indices
        .iter()
        .filter_map(|i| sentences.get(*i))
        .cloned()
        .collect()
}

fn compute_tone(lower: &str) -> (i32, Vec<String>) {
    let pos = POSITIVE_WORDS.iter().filter(|w| lower.contains(*w)).count() as i32;
    let neg = NEGATIVE_WORDS.iter().filter(|w| lower.contains(*w)).count() as i32;
    let mut friction: Vec<String> = FRICTION_PHRASES
        .iter()
        .filter(|p| lower.contains(*p))
        .map(|p| p.to_string())
        .collect();
    friction.dedup();
    let friction_penalty = (friction.len() as i32 * 2).min(4);
    let raw = pos - neg - friction_penalty;
    let clamped = raw.clamp(-5, 5);
    (clamped, friction)
}

fn compute_formality(
    word_count: usize,
    sentences: &[String],
    contraction_count: usize,
    exclamation_count: usize,
    has_bullets: bool,
    lower: &str,
) -> i32 {
    let mut score: i32 = 5;
    score -= (contraction_count as i32).min(3);
    if word_count > 0 {
        let total_chars: usize = lower.split_whitespace().map(|w| w.len()).sum();
        let avg_len = total_chars as f32 / word_count as f32;
        if avg_len > 6.0 {
            score += 2;
        }
    }
    let proper_end = sentences
        .iter()
        .filter(|s| {
            let t = s.trim();
            t.ends_with('.') || t.ends_with('?') || t.ends_with('!')
        })
        .count();
    if sentences.len() > 0 && proper_end * 2 >= sentences.len() {
        score += 1;
    }
    if exclamation_count > 2 {
        score -= 1;
    }
    if lower.contains("please") || lower.contains("kindly") {
        score += 1;
    }
    if has_bullets {
        score -= 1;
    }
    let caps_words = lower
        .split_whitespace()
        .filter(|w| w.len() > 2 && w.chars().all(|c| c.is_ascii_uppercase()))
        .count();
    score -= (caps_words as i32).min(2);
    score.clamp(0, 10)
}

fn extract_entities(text: &str, sentences: &[String], lower: &str) -> Vec<(String, String)> {
    let mut entities = Vec::new();
    for sentence in sentences {
        let s_lower = sentence.to_lowercase();
        for greeting in GREETING_WORDS {
            if s_lower.starts_with(greeting) {
                let words: Vec<&str> = sentence.split_whitespace().collect();
                if words.len() > 1 {
                    let candidate = words[1].trim_matches(|c: char| !c.is_alphabetic());
                    if !candidate.is_empty() && candidate.chars().next().unwrap().is_uppercase() {
                         entities.push((candidate.to_string(), "person".to_string()));
                    }
                }
            }
        }
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    while i < words.len() {
        let w = words[i].trim_matches(|c: char| !c.is_alphabetic());
        if w.len() > 3 && w.chars().next().map_or(false, |c| c.is_uppercase()) {
            if i + 1 < words.len() {
                let w2 = words[i+1].trim_matches(|c: char| !c.is_alphabetic());
                if w2.len() > 3 && w2.chars().next().map_or(false, |c| c.is_uppercase()) {
                    let full = format!("{} {}", w, w2);
                    entities.push((full, "project".to_string()));
                    i += 2;
                    continue;
                }
            }
        }
        i += 1;
    }
    if let Some(at_idx) = lower.find('@') {
        let domain_part = &lower[at_idx+1..];
        if let Some(dot_idx) = domain_part.find('.') {
            let domain = &domain_part[..dot_idx];
            if !["gmail", "outlook", "hotmail", "yahoo", "icloud"].contains(&domain) {
                entities.push((domain.to_string(), "company".to_string()));
            }
        }
    }
    entities.dedup();
    entities
}
