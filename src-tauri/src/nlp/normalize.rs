/// Stage 1 — Normalize & Clean
///
/// Converts messy real-world input (smart quotes, NBSPs, BOM, em-dashes)
/// into a clean, consistent string all downstream stages can rely on.

#[derive(Debug, Clone)]
pub struct NormalizeOutput {
    pub normalized: String,
    pub word_count: usize,
    pub char_count: usize,
    pub has_urls: bool,
    pub has_emails: bool,
}

impl NormalizeOutput {
    pub fn empty_from(raw: &str) -> Self {
        NormalizeOutput {
            normalized: raw.to_string(),
            word_count: raw.split_whitespace().count(),
            char_count: raw.chars().count(),
            has_urls: false,
            has_emails: false,
        }
    }
}

fn normalize_char(c: char) -> char {
    match c {
        // Smart quotes → ASCII
        '\u{2018}' | '\u{2019}' => '\'',
        '\u{201C}' | '\u{201D}' => '"',
        // Em/en dash → hyphen
        '\u{2013}' | '\u{2014}' => '-',
        // Non-breaking space, thin space, zero-width, BOM → space
        '\u{00A0}' | '\u{200B}' | '\u{FEFF}' | '\u{202F}' | '\u{FFFE}' => ' ',
        // Ellipsis → period (caller collapses runs)
        '\u{2026}' => '.',
        // Bullet points → dash
        '\u{2022}' | '\u{25AA}' | '\u{25CF}' => '-',
        _ => c,
    }
}

pub fn normalize(raw: &str) -> NormalizeOutput {
    if raw.is_empty() {
        return NormalizeOutput::empty_from(raw);
    }

    let char_count = raw.chars().count();

    // Cheap early checks before transformation
    let has_urls = raw.contains("http://") || raw.contains("https://") || raw.contains("www.");
    let has_emails = raw.contains('@')
        && raw.split_whitespace().any(|w| {
            let at = w.find('@');
            at.map(|i| i > 0 && i < w.len() - 1).unwrap_or(false)
        });

    // Apply character-level normalization then collapse whitespace
    let normalized: String = raw
        .chars()
        .map(normalize_char)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");


    let word_count = normalized.split_whitespace().count();

    NormalizeOutput {
        normalized,
        word_count,
        char_count,
        has_urls,
        has_emails,
    }
}
