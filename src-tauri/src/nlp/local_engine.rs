/// Stage 6 — Local Transform Engine (Freemium)
///
/// Provides rule-based, non-AI text transformations for offline use
/// or when no API key is present.

use super::mod_types::TextContext;

// ── Fix: Rule-based grammar/punctuation ────────────────────────────────────

pub fn fix_local(text: &str) -> String {
    let mut res = text.trim().to_string();
    if res.is_empty() { return res; }

    // 1. Collapse multiple spaces
    while res.contains("  ") {
        res = res.replace("  ", " ");
    }

    // 2. Space after punctuation
    let punctuation = ['.', ',', '!', '?', ';', ':'];

    // More effective approach: regex or char iterator
    let mut fixed = String::with_capacity(res.len() + 10);
    let chars: Vec<char> = res.chars().collect();
    for i in 0..chars.len() {
        fixed.push(chars[i]);
        if punctuation.contains(&chars[i]) && i + 1 < chars.len() {
            let next = chars[i+1];
            if next.is_alphabetic() {
                fixed.push(' ');
            }
        }
    }
    
    // 3. Capitalize first letter if needed
    if let Some(first_char) = fixed.chars().next() {
        if first_char.is_lowercase() {
            let mut c = fixed.chars();
            fixed = first_char.to_uppercase().to_string() + c.next().map(|_| "").unwrap_or("") + &c.collect::<String>();
        }
    }
    // 4. Common typo fixes
    let typos = [
        (" i ", " I "), (" i'm ", " I'm "), (" i've ", " I've "), 
        (" dont ", " don't "), (" cant ", " can't "), (" wont ", " won't "),
        (" thats ", " that's "), (" its ", " it's "), (" youre ", " you're ")
    ];
    for (bad, good) in typos {
        fixed = fixed.replace(bad, good);
    }

    fixed
}

// ── Summarize: Top sentences + extractive logic ────────────────────────────

pub fn summarize_local(ctx: &TextContext) -> String {
    if ctx.top_sentences.is_empty() {
        return "Not enough content to summarize.".into();
    }
    
    let mut summary = ctx.top_sentences.join(" ");
    if !summary.ends_with('.') && !summary.ends_with('?') && !summary.ends_with('!') {
        summary.push('.');
    }
    summary
}

// ── Shorten: Remove filler words and adverbs ───────────────────────────────

pub fn shorten_local(text: &str) -> String {
    let fillers = [
        "basically", "actually", "literally", "honestly", "very", "really",
        "just", "quite", "rather", "somewhat", "perhaps", "maybe",
        "in my opinion", "at the end of the day", "to be honest",
    ];
    
    let mut res = text.to_string();
    for filler in fillers {
        let pattern_space = format!(" {} ", filler);
        res = res.replace(&pattern_space, " ");
        
        let pattern_start = format!("{} ", filler);
        if res.to_lowercase().starts_with(&pattern_start) {
             res = res[pattern_start.len()..].to_string();
        }
    }
    res.trim().to_string()
}

// ── Tone: Formality/Tone verdict from scores ───────────────────────────────

pub fn report_tone_local(ctx: &TextContext) -> String {
    let tone_verdict = match ctx.tone {
        3..=5 => "Positive and friendly",
        1..=2 => "Slightly positive",
        0 => "Neutral",
        -2..=-1 => "Tense/Negative",
        _ => "Highly negative or urgent",
    };

    let formality_verdict = match ctx.formality {
        8..=10 => "Highly professional",
        5..=7 => "Professional but standard",
        2..=4 => "Casual/Friendly",
        _ => "Very informal",
    };

    let friction = if ctx.friction_phrases.is_empty() {
        "".into()
    } else {
        format!("\nWarning: Detected friction phrases ({})", ctx.friction_phrases.join(", "))
    };

    format!(
        "Analysis: {}, {}.\nWord Count: {}.{}",
        tone_verdict, formality_verdict, ctx.word_count, friction
    )
}

// ── Entry Point ────────────────────────────────────────────────────────────

/// Core transform dispatcher for non-AI mode
pub fn transform(mode: &str, ctx: &TextContext) -> String {
    match mode {
        "Fix" => fix_local(&ctx.original),
        "Summarize" => summarize_local(ctx),
        "Shorten" => shorten_local(&ctx.original),
        "Professional" | "Casual" | "Strategist" | "Email" => {
            // These require AI for true high quality, so we provide the tone report offline
            report_tone_local(ctx)
        }
        _ => "Mode requires AI connection (Gemini key not found).".into()
    }
}
