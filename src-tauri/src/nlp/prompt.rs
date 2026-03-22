use tauri::Manager;
use super::mod_types::TextContext;

// ── Anti-injection sanitization ────────────────────────────────────────────

const INJECTION_PATTERNS: &[&str] = &[
    "\nIgnore previous",
    "\nForget previous",
    "\nSystem:",
    "\nsystem:",
    "\n[INST]",
    "\n[SYS]",
    "###",
    "```system",
];

/// Strip known prompt-injection payloads from user text before embedding
/// it into any Gemini API request.
fn sanitize(text: &str) -> String {
    let mut out = text.to_string();
    for pattern in INJECTION_PATTERNS {
        out = out.replace(pattern, " [sanitized] ");
    }
    out
}

// ── Language context block ─────────────────────────────────────────────────

fn build_language_block(ctx: &TextContext) -> String {
    let lang = &ctx.language;
    let mut parts: Vec<String> = vec![
        format!(
            "The input text is written in {}.",
            lang.candidate_languages
        )
    ];
    if lang.is_mixed {
        parts.push(format!(
            "It contains mixed script content ({:.0}% primary, {:.0}% secondary).",
            lang.primary_pct * 100.0,
            lang.secondary_pct * 100.0
        ));
    }
    if lang.is_rtl {
        parts.push("The language uses right-to-left script.".into());
    }
    if lang.needs_romanization_hint {
        parts.push(
            "Preserve proper nouns, names, and cultural references. \
             Do not romanize non-Latin script text unless explicitly asked."
                .into(),
        );
    }
    parts.join(" ")
}

// ── Context block ──────────────────────────────────────────────────────────

fn build_context_block(ctx: &TextContext) -> String {
    let intent_label = &ctx.intent_result.primary.label;
    let conf = ctx.intent_result.primary.confidence;
    let tone_str = match ctx.tone {
        3..=5   => "positive and enthusiastic",
        1..=2   => "generally positive",
        0       => "neutral",
        -2..=-1 => "somewhat negative",
        _       => "tense or negative",
    };
    let formality_str = match ctx.formality {
        7..=10 => "highly formal and professional",
        4..=6  => "semi-formal",
        1..=3  => "informal and casual",
        _      => "very casual",
    };

    let friction_note = if ctx.friction_phrases.is_empty() {
        String::new()
    } else {
        format!(
            " The text contains tension indicators ({}). Handle diplomatically and de-escalate where appropriate.",
            ctx.friction_phrases.join(", ")
        )
    };

    format!(
        "The text appears to be {} (confidence: {:.0}%), has a {} tone, and reads as {}. \
         It is approximately {} words long.{}",
        intent_label,
        conf * 100.0,
        tone_str,
        formality_str,
        ctx.word_count,
        friction_note
    )
}

// ── Task block ─────────────────────────────────────────────────────────────

fn build_task_block(mode: &str, sub_mode: Option<&str>, ctx: &TextContext) -> String {
    match mode {
        "Email" => {
            let style = match sub_mode {
                Some("formal") =>
                    "highly formal and professional, using sophisticated language, \
                     proper salutations, and business etiquette",
                Some("personal") =>
                    "warm, personal, and friendly while remaining clear",
                _ => if ctx.formality >= 6 {
                    "professional and polished"
                } else {
                    "friendly and clear"
                },
            };
            format!(
                "Transform the following notes into a well-structured email that is {}. \
                 Include a proper subject line, greeting, body, and sign-off.",
                style
            )
        }
        "Summarize" => {
            if ctx.word_count > 300 {
                "Provide a comprehensive summary covering: (1) the main topic or request, \
                 (2) the 3 most important points, and (3) any action items or conclusions. \
                 Use bullet points for clarity."
                    .into()
            } else {
                "Summarize the key points into 1–3 concise sentences. \
                 Preserve the most important information and discard filler."
                    .into()
            }
        }
        "Correct" => {
            "Fix all grammar, spelling, punctuation, and structural issues. \
             Preserve the original tone, voice, and meaning exactly. \
             Do not add information or change the register unless it is fundamentally broken."
                .into()
        }
        "Translate" => {
            let target = match sub_mode {
                Some("hinglish") => "fluent, professional English from Hinglish (Hindi/English mix)",
                _ => "English (if not English) or Spanish (if already English)",
            };
            format!(
                "Translate the following text to {}. \
                 Maintain the original tone, formality level, and cultural nuances.",
                target
            )
        }
        "Prompt" => {
            "You are an expert Prompt Engineer. Transform the following rough notes or \
             instructions into a high-quality, professional AI prompt. \
             Use sections [Role], [Context], [Task], and [Constraints]. \
             Output only the enhanced prompt, no explanation."
                .into()
        }
        "Casual" => {
            "Rewrite this text in a natural, conversational tone. \
             Make it sound like a real person talking — use contractions, \
             keep sentences short, cut any stiff or corporate language."
                .into()
        }
        "Knowledge" => {
            "You are an expert technical instructor and consultant. \
             The user is asking for guidance or instruction on a complex topic. \
             Provide a clear, structured, and practical explanation. \
             Use formatting (bullet points, bold text) to highlight key concepts. \
             Focus on actionable advice and 'why' it matters. \
             Maintain an encouraging, senior-to-junior mentoring tone."
                .into()
        }
        "Professional" => {
            "Rewrite this text with a professional, confident tone. \
             Fix grammar, improve word choice, ensure clarity and conciseness. \
             Maintain the original intent and factual content."
                .into()
        }
        "Strategist" => {
            "You are an elite Executive Brand Strategist. \
             Analyze the following communication and provide: \
             (1) A high-level strategic critique of the tone and positioning. \
             (2) 3 punchy, high-impact alternatives (Standard, Bold, and Diplomatic). \
             (3) A 'Moat Check': how this reinforces or weakens the sender's authority. \
             Output in a structured, concise format."
                .into()
        }
        _ => {
            if let Some(instruction) = sub_mode {
                format!(
                    "The user has provided this transformation requirement: '{}'. \
                     Apply it to the text at an expert level. \
                     Output only the transformed text, no preamble.",
                    instruction
                )
            } else {
                "Rewrite the following text to be clear, professional, and concise.".into()
            }
        }
    }
}

// ── Constraint block ───────────────────────────────────────────────────────

fn build_constraint_block(ctx: &TextContext) -> String {
    let mut constraints: Vec<&str> = vec![
        "Output ONLY the transformed text.",
        "Do not include meta-commentary, explanations, or labels.",
        "Do not start with phrases like 'Here is' or 'Certainly'.",
        "Preserve all proper nouns, names, and technical terms exactly.",
    ];
    if ctx.has_emails {
        constraints.push("Preserve all email addresses exactly.");
    }
    if ctx.has_urls {
        constraints.push("Preserve all URLs exactly.");
    }
    constraints.join(" ")
}

// ── Team Voice block (Doc 2 / Feature 5) ───────────────────────────────────

fn build_team_voice_block(handle: &tauri::AppHandle) -> String {
    let path = handle.path().app_data_dir().unwrap().join("team_voice.json");
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            let mut block = String::from("### TEAM SHARED VOICE (CORPORATE GUIDELINES)\n");
            if let Some(guide) = json.get("guidelines").and_then(|g| g.as_str()) {
                block.push_str(&format!("- Guidelines: {}\n", guide));
            }
            if let Some(tone) = json.get("required_tone").and_then(|t| t.as_str()) {
                block.push_str(&format!("- Required Tone: {}\n", tone));
            }
            block.push_str("PRIORITY: Corporate guidelines take precedence over personal style where they conflict.\n\n");
            return block;
        }
    }
    String::new()
}

// ── Voice block (Doc 2) ────────────────────────────────────────────────────

fn build_voice_block(profile_data: &[(String, String, String)]) -> Option<String> {
    if profile_data.is_empty() { return None; }

    let mut openers = Vec::new();
    let mut closers = Vec::new();
    let mut vocab   = Vec::new();
    let mut formality = "neutral";

    for (t, k, v) in profile_data {
        match t.as_str() {
            "opener" => openers.push(k.as_str()),
            "closer" => closers.push(k.as_str()),
            "vocab"  => vocab.push(k.as_str()),
            "stat" if k == "formality" => {
                let f: f32 = v.parse().unwrap_or(5.0);
                formality = if f >= 7.0 { "formal" } else if f <= 3.0 { "casual" } else { "neutral" };
            }
            _ => {}
        }
    }

    if openers.is_empty() && closers.is_empty() && vocab.is_empty() { return None; }

    let mut block = String::from("User's writing style profile (learned from their actual messages):\n");
    if !openers.is_empty() { block.push_str(&format!("- Common openers: {}\n", openers.join(", "))); }
    if !closers.is_empty() { block.push_str(&format!("- Common closers: {}\n", closers.join(", "))); }
    if !vocab.is_empty()   { block.push_str(&format!("- Vocabulary fingerprint: {}\n", vocab.join(", "))); }
    block.push_str(&format!("- Typical register: {}\n", formality));
    block.push_str("Match this voice in the output. The result should sound like THIS specific person, not a generic AI.");

    Some(block)
}

// ── Memory block (Doc 2) ───────────────────────────────────────────────────

fn build_memory_block(memory: &[(String, String, String, String)]) -> String {
    if memory.is_empty() { return String::new(); }
    
    let mut block = String::from("### CONTEXT MEMORY (RELATIONSHIP CONTEXT)\n");
    for (name, etype, attr, val) in memory {
        match attr.as_str() {
            "typical_tone" => {
                let v: f32 = val.parse().unwrap_or(0.0);
                let t = if v > 1.0 { "friendly" } else if v < -1.0 { "professional/stern" } else { "neutral" };
                block.push_str(&format!("- {} ({}): Usually interacts with a {} tone.\n", name, etype, t));
            },
            "formality" => {
                let v: f32 = val.parse().unwrap_or(5.0);
                let f = if v > 7.0 { "formal" } else if v < 3.0 { "casual" } else { "standard" };
                block.push_str(&format!("- {} ({}): Relationship is {}.\n", name, etype, f));
            },
            _ => {}
        }
    }
    block.push_str("\n");
    block
}

// ── Public API ─────────────────────────────────────────────────────────────

pub fn build_prompt(
    app: &tauri::AppHandle,
    ctx: &TextContext,
    mode: &str,
    sub_mode: Option<&str>,
    profile_data: &[(String, String, String)],
    memory_data: &[(String, String, String, String)],
) -> String {
    let language_block  = build_language_block(ctx);
    let context_block   = build_context_block(ctx);
    let task_block      = build_task_block(mode, sub_mode, ctx);
    let constraint_block = build_constraint_block(ctx);

    let mut prompt = format!(
        "[ROLE]\n\
         You are an elite AI writing assistant embedded in a universal keyboard tool. \
         You process text captured from any application on the user's device.\n\n\
         [LANGUAGE]\n{}\n\n\
         [CONTEXT]\n{}\n\n\
         [TASK]\n{}\n\n\
         [CONSTRAINTS]\n{}",
        language_block, context_block, task_block, constraint_block
    );

    prompt.push_str(&build_team_voice_block(app));

    if let Some(voice) = build_voice_block(profile_data) {
        prompt.push_str(&format!("\n\n[VOICE PROFILE]\n{}", voice));
    }

    prompt.push_str(&build_memory_block(memory_data));

    let clean_text = sanitize(&ctx.original);
    format!("{}\n\n[INPUT TEXT]\n{}", prompt, clean_text)
}
