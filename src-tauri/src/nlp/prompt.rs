use tauri::Manager;
use unicode_normalization::UnicodeNormalization;
use super::mod_types::TextContext;

// ── Anti-injection sanitization ────────────────────────────────────────────

/// Patterns matched case-insensitively after NFKC normalization.
/// NFKC collapses Unicode homoglyphs (e.g. Cyrillic ѕ → s, fullwidth Ａ → A).
const INJECTION_PATTERNS_LOWER: &[&str] = &[
    "\nignore previous",
    "\nforget previous",
    "\nsystem:",
    "\n[inst]",
    "\n[sys]",
    "###",
    "```system",
    "<|system|>",
    "<|im_start|>",
    "\nassistant:",
    "\nhuman:",
    "\nai:",
    "prompt injection",
];

/// Strip known prompt-injection payloads from user text.
/// Public so `lib.rs` can sanitize the RAG query before embedding.
/// Uses NFKC normalization + case-insensitive matching to defeat homoglyph
/// and casing bypasses (e.g. "\nSYSTEM:", Cyrillic lookalikes).
pub fn sanitize(text: &str) -> String {
    sanitize_inner(text)
}

fn sanitize_inner(text: &str) -> String {
    // Step 1: NFKC normalization collapses homoglyphs
    let normalized: String = text.nfkc().collect();

    // Step 2: Case-insensitive replacement — match on lowercase, replace in original
    let mut out = normalized;
    for pattern in INJECTION_PATTERNS_LOWER {
        let lower_out = out.to_lowercase();
        if !lower_out.contains(pattern) { continue; }
        // Rebuild string replacing all case-insensitive matches
        let mut result = String::with_capacity(out.len());
        let mut remaining = lower_out.as_str();
        let mut src = out.as_str();
        let mut offset = 0;
        while let Some(pos) = remaining.find(pattern) {
            result.push_str(&src[..pos]);
            result.push_str(" [sanitized] ");
            let skip = pos + pattern.len();
            src = &src[skip..];
            remaining = &remaining[skip..];
            offset += skip;
        }
        result.push_str(src);
        out = result;
        let _ = offset; // suppress unused warning
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

// ── Natural language domain detection ─────────────────────────────────────

fn detect_natural_domain(text: &str) -> &'static str {
    let lower = text.to_lowercase();
    // Legal / compliance (high specificity — check first)
    if ["contract", "clause", "liability", "compliance", "regulation", "indemnif",
        "warranty", "arbitration", "jurisdiction", "pursuant", "herein", "whereas",
        "legal", "legally", "license", "licensing", "copyright", "intellectual property",
        " ip ", "open source", "proprietary", "trademark", "patent", "royalt"]
        .iter().any(|w| lower.contains(w)) { return "legal"; }
    // Academic / research
    if ["hypothesis", "methodology", "literature review", "citation", "findings",
        "abstract", "conclusion", "thesis", "peer-reviewed", "empirical", "dataset"]
        .iter().any(|w| lower.contains(w)) { return "academic"; }
    // Marketing / sales
    if ["campaign", "conversion", "funnel", "audience", "revenue", "cta", "engagement",
        "branding", "roi", "lead generation", "pipeline", "saas", "churn"]
        .iter().any(|w| lower.contains(w)) { return "marketing"; }
    // Creative / narrative
    if ["character", "story", "plot", "scene", "narrative", "dialogue", "protagonist",
        "setting", "chapter", "fiction", "screenplay", "genre"]
        .iter().any(|w| lower.contains(w)) { return "creative"; }
    // Business / professional (broad — check last before general)
    if ["meeting", "deadline", "stakeholder", "deliverable", "budget", "roadmap",
        "quarterly", "kpi", "objective", "strategy", "initiative", "milestone"]
        .iter().any(|w| lower.contains(w)) { return "business"; }
    "general"
}

// ── Dev input type detection ───────────────────────────────────────────────

fn detect_dev_input_type(text: &str) -> &'static str {
    let t = text.trim();
    let upper = t.to_uppercase();

    // Stack trace / error patterns
    if t.contains("Error:") || t.contains("error:")
        || t.contains("Traceback") || t.contains("Exception")
        || t.contains("FAILED") || t.contains("panic!")
        || (t.contains("at ") && (t.contains(".js:") || t.contains(".ts:") || t.contains(".rs:")))
        || t.contains("errno") || t.contains("ENOENT")
        || t.contains("stack trace") || t.contains("Caused by")
    {
        return "error";
    }

    // SQL
    if upper.starts_with("SELECT ") || upper.starts_with("INSERT ")
        || upper.starts_with("UPDATE ") || upper.starts_with("DELETE ")
        || upper.starts_with("CREATE ") || upper.starts_with("ALTER ")
        || upper.starts_with("DROP ") || upper.contains(" JOIN ")
    {
        return "sql";
    }

    // Code (keywords + structural patterns)
    if t.contains("fn ") || t.contains("function ")
        || t.contains("def ") || t.contains("class ")
        || t.contains("const ") || t.contains("let ")
        || t.contains("import ") || t.contains("#include")
        || t.contains("pub struct") || t.contains("interface ")
        || (t.contains('{') && t.contains('}') && (t.contains('(') || t.contains(';')))
    {
        return "code";
    }

    // JSON / structured data
    if (t.starts_with('{') && t.ends_with('}'))
        || (t.starts_with('[') && t.ends_with(']'))
    {
        return "data";
    }

    "natural"
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
            // Alt+Shift+L: rewrite ANY language (Hinglish, broken English, Hindi, etc.)
            // into clear, fluent, professional English. This is the core use case.
            "You are a professional English rewriter. \
             Your task: produce clear, fluent, professional English. \
             \n\
             RULE 1 — ONLY FIX WHAT IS BROKEN: \
             If the input is already clear, grammatically correct English, return it UNCHANGED. \
             Do not paraphrase, do not rephrase, do not 'improve' what is already good. \
             The test: would a native English speaker reading it find any error? \
             If no — return it as-is. \
             \n\
             RULE 2 — IF BROKEN, FIX COMPLETELY: \
             If the input has errors, non-English words, Hinglish, Hindi, or broken grammar — \
             rewrite into clean English. Preserve the original meaning and intent exactly. \
             Fix grammar, spelling, word choice, and structure. \
             Do not add information. Do not change the tone unless it is rude — then make it polite. \
             \n\
             Output only the (corrected or unchanged) English text. Nothing else."
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
            let english_rule = "CRITICAL: The entire output MUST be in fluent, professional English \
                     regardless of the input language. If the input is in Hindi, Hinglish, or any \
                     other language, first understand the intent, then write the prompt in English. \
                     Never copy or transliterate non-English text into the output.";
            match detect_dev_input_type(&ctx.original) {
                "error" =>
                    format!(
                        "You are a senior debugging assistant. The user selected an error message \
                         or stack trace. Transform it into a structured debug prompt with these sections: \
                         **Error:** (the exact error, cleaned up), \
                         **Likely Cause:** (2-3 probable reasons based on the error pattern), \
                         **Context to Provide:** (what info an AI would need — language, framework, OS), \
                         **Ask:** (the specific, well-formed question to get a fix). \
                         {} Output only the prompt. No explanation.", english_rule),
                "code" =>
                    format!(
                        "You are a senior code reviewer. The user selected a code snippet. \
                         Transform it into a structured prompt with these sections: \
                         **Language:** (detected language/framework), \
                         **Code Purpose:** (what this code appears to do), \
                         **Task:** (ask to explain, review, optimize, or find bugs — pick the most useful), \
                         **Focus Areas:** (specific concerns: edge cases, performance, readability, security). \
                         {} Output only the prompt. No explanation.", english_rule),
                "sql" =>
                    format!(
                        "You are a database expert. The user selected a SQL query. \
                         Transform it into a structured prompt with these sections: \
                         **Query Purpose:** (what this query does in plain English), \
                         **Task:** (optimize, explain, fix, or review — pick the most useful), \
                         **Constraints:** (preserve correctness, note potential N+1 or index issues). \
                         {} Output only the prompt. No explanation.", english_rule),
                "data" =>
                    format!(
                        "You are a data analyst. The user selected structured data (JSON/config/YAML). \
                         Transform it into a structured prompt with these sections: \
                         **Data Shape:** (describe the structure and key fields), \
                         **Task:** (analyze, validate, transform, or document — pick the most useful), \
                         **Expected Output:** (what format the answer should be in). \
                         {} Output only the prompt. No explanation.", english_rule),
                _ => {
                    let domain_instruction = match detect_natural_domain(&ctx.original) {
                        "legal" =>
                            "You are a senior legal-writing assistant. \
                             Transform the input into a clear, structured legal or compliance prompt. \
                             Sections: **Legal Context:** (jurisdiction/agreement type), \
                             **Issue:** (what needs to be drafted or reviewed), \
                             **Constraints:** (must-have clauses, governing law, tone). \
                             Output only the prompt.",
                        "academic" =>
                            "You are a research-writing expert. \
                             Transform the input into a rigorous academic prompt. \
                             Sections: **Field:** (discipline/domain), \
                             **Research Question:** (what to investigate), \
                             **Methodology Hint:** (quantitative, qualitative, mixed), \
                             **Output Format:** (essay, abstract, literature review, etc.). \
                             Output only the prompt.",
                        "marketing" =>
                            "You are a senior growth marketer. \
                             Transform the input into a high-converting marketing prompt. \
                             Sections: **Product/Service:** (what is being promoted), \
                             **Audience:** (who the reader is), \
                             **Goal:** (awareness, conversion, retention), \
                             **Tone:** (bold, professional, playful), \
                             **Format:** (ad copy, email, landing page, post). \
                             Output only the prompt.",
                        "creative" =>
                            "You are a creative writing coach. \
                             Transform the input into a vivid creative-writing prompt. \
                             Sections: **Genre/Tone:** (thriller, romance, literary fiction), \
                             **Setting:** (time and place), \
                             **Character/Conflict:** (who and what challenge), \
                             **Task:** (write, continue, or rewrite the scene). \
                             Output only the prompt.",
                        "business" =>
                            "You are an executive business strategist. \
                             Transform the input into a structured business prompt. \
                             Sections: **Objective:** (what outcome is needed), \
                             **Stakeholders:** (who is involved), \
                             **Constraints:** (timeline, budget, format), \
                             **Expected Output:** (report, plan, email, slide deck). \
                             Output only the prompt.",
                        _ =>
                            "You are an expert Prompt Engineer. \
                             Transform the rough notes into a high-quality AI prompt. \
                             Sections: **Role:** (who the AI should be), \
                             **Context:** (background), **Task:** (what to do), \
                             **Constraints:** (rules to follow). \
                             Output only the enhanced prompt.",
                    };
                    format!("{} {}", domain_instruction, english_rule)
                },
            }
        }
        "Reply" => {
            // The user selected a message someone sent THEM.
            // SnapText composes a REPLY to that message.
            "You are composing a reply to a message the user received. \
             The text below is what ANOTHER PERSON sent to the user — not the user's own words.\n\
             \n\
             REASON FIRST — think silently, do not output this reasoning:\n\
             \n\
             Q1 — WHO NEEDS TO ACT?\n\
             • Sender mentions 'wo / usne / woh [didn't do X]' → third party didn't act. Empathize with sender.\n\
             • Sender is addressing the USER directly ('abhi tak nhi bheji', 'tune reply nhi kiya', \
               'mujhe chahiye', 'bhej de', 'you haven't', 'please do X') → the USER must act. \
               Acknowledge + commit. Never claim you are also waiting for something.\n\
             \n\
             Q2 — WHAT DOES THE SENDER NEED?\n\
             Action from me / empathy / a direct answer / casual chat / conflict resolution.\n\
             \n\
             Q3 — WHAT LANGUAGE AND ENERGY?\n\
             Detect: Hinglish / English / Hindi Devanagari / regional / mixed. Match it exactly.\n\
             \n\
             Q4 — HOW LONG IS THEIR MESSAGE?\n\
             Short = reply short. Long = can go longer. One-liner = one-liner back.\n\
             \n\
             NOW WRITE THE REPLY — one response that fits all four answers:\n\
             • User must act → acknowledge + commit: e.g. 'sorry yaar, aaj raat tak pakka bhej dunga'\n\
             • Third-party vent → empathize + advise sender: e.g. 'chhod yaar, ek message maar de usse'\n\
             • Question → answer it directly, no padding\n\
             • Casual → match their energy, keep it real\n\
             • Conflict → acknowledge calmly, don't be defensive\n\
             • Formal request → professional, clear reply\n\
             \n\
             Output ONLY the reply. No greetings. No filler (ok/sure/hmm). Sound human."
                .into()
        }
        "Do" => {
            // The user wrote an INSTRUCTION or notes describing what they want done.
            // SnapText executes the instruction — write the message, make the list, etc.
            //
            // Domain routing: give the model a specialist frame for known content types,
            // then fall back to the meta-reasoning framing for everything else.
            let dev_type = detect_dev_input_type(&ctx.original);
            let domain   = detect_natural_domain(&ctx.original);

            let specialist_frame: &str = match (dev_type, domain) {
                // Developer content
                ("code", _) =>
                    "You are a senior software engineer producing developer documentation, \
                     PR descriptions, commit messages, or code review comments. \
                     Be precise, reference the code specifics, use correct technical terminology. \
                     Format with markdown where appropriate.",
                ("error", _) =>
                    "You are a senior developer writing an escalation, bug report, or Slack message \
                     about a technical issue. Be specific about what broke, what you tried, and what you need. \
                     Do not be vague — include the actual error context.",
                ("sql", _) =>
                    "You are a data engineer writing documentation, a query explanation, or a data \
                     request. Be precise about table names, filters, and expected output.",
                // Professional content
                (_, "legal") =>
                    "You are a senior legal writer. Legal notices and formal legal correspondence \
                     must include: (1) the precise claim or demand, (2) the legal basis, \
                     (3) a clear cure/response deadline, (4) consequence of non-compliance. \
                     Use formal register. Avoid ambiguity — every sentence must have a clear meaning.",
                (_, "academic") =>
                    "You are a research writer. Academic writing must be precise, cite logic not \
                     opinion, use passive voice where conventional, and be structured: \
                     Introduction → Argument → Evidence → Conclusion.",
                (_, "marketing") =>
                    "You are a senior copywriter. Great marketing copy: leads with the benefit not \
                     the feature, speaks directly to the reader's desire or pain, has a clear CTA, \
                     and creates urgency without being pushy. Every word earns its place.",
                (_, "creative") =>
                    "You are a professional writer. Great creative writing: shows, doesn't tell; \
                     uses specific concrete details not vague adjectives; has a distinct voice; \
                     creates emotion through situation, not description.",
                (_, "business") =>
                    "You are a senior business communicator. Business writing must be clear on: \
                     (1) the decision or action being requested, (2) the business rationale in one sentence, \
                     (3) next steps with owners. Cut filler — every sentence must add information.",
                // Default: meta-reasoning fallback
                _ => "",
            };

            let meta_reasoning = "Apply the quality bar of a professional: \
             great pitches make someone lean forward, great emails get replies, \
             great task lists are instantly actionable, great bios make someone want to meet you. \
             For task lists specifically: take the user's rough keywords and expand each into a clear, \
             actionable task — add the implied action verb, clarify scope, and where obvious add a brief \
             sub-note (e.g. 'HRMS Implementation — configure modules, map employee data, test onboarding flow'). \
             Identify what the content must achieve and write to that bar. \
             Never output your reasoning — only the finished result.";

            let output_rules = "THE ONE FAILURE MODE TO AVOID:\n\
             Summarizing or reformatting the user's input. \
             The user gave you RAW MATERIAL. Your job is to TRANSFORM it into finished content. \
             If you find yourself repeating back what they said in cleaner words — stop.\n\
             \n\
             OUTPUT RULES:\n\
             • Output ONLY the finished result. Start DIRECTLY with the content — zero preamble. \
               No 'Sure', no 'Here is your X', no 'Are bhai', no 'Great!', no 'Of course', \
               no conversational opener of any kind. The very first character must be part of the deliverable.\n\
             • Do NOT echo or explain the instruction.\n\
             • LANGUAGE: The input is the user's INSTRUCTION to you — its language is irrelevant to the output language. \
               Default is always fluent English. \
               Only switch if the user explicitly requests it ('Hindi mein likho', 'Hinglish mein', 'in French', etc.). \
               Hinglish input = English output unless told otherwise.";

            if specialist_frame.is_empty() {
                format!("You are an expert content creator and writing assistant. \
                         The input is a rough instruction describing what the user wants to PRODUCE.\n\n\
                         {}\n\n{}", meta_reasoning, output_rules)
            } else {
                format!("You are an expert content creator and writing assistant. \
                         The input is a rough instruction describing what the user wants to PRODUCE.\n\n\
                         SPECIALIST FRAME FOR THIS CONTENT TYPE:\n{}\n\n\
                         GENERAL QUALITY STANDARD:\n{}\n\n{}", specialist_frame, meta_reasoning, output_rules)
            }
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

fn build_constraint_block(mode: &str, ctx: &TextContext) -> String {
    let mut constraints: Vec<&str> = match mode {
        "Reply" => vec![
            "The input is a message the user RECEIVED. Treat it as someone else's text that needs a reply.",
            "Output ONLY the reply — ready to copy-paste and send back.",
            "Match the language and tone of the original message.",
            "Do NOT rewrite or transform the original message.",
            "Do NOT include meta-commentary, explanations, or labels.",
            "Do NOT start with a greeting (hello, hi, hey, haan, hii) — this is mid-conversation, not a new chat.",
            "Do NOT start with phrases like 'Here is' or 'Sure'.",
        ],
        "Do" => vec![
            "The input is an INSTRUCTION from the user — what they want you to produce.",
            "Output ONLY the finished result the instruction asks for.",
            "CRITICAL: If the instruction requests a specific output language (e.g., 'in English', 'English mein', 'Hindi mein'), produce the output in THAT language regardless of what language the instruction itself is written in.",
            "Do NOT translate, echo, or rewrite the instruction itself.",
            "Do NOT include meta-commentary, explanations, or labels.",
            "Do NOT start with phrases like 'Here is' or 'Sure'.",
        ],
        _ => vec![
            "Output ONLY the transformed text.",
            "Do not include meta-commentary, explanations, or labels.",
            "Do not start with phrases like 'Here is' or 'Certainly'.",
            "Preserve all proper nouns, names, and technical terms exactly.",
        ],
    };
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

    let mut openers          = Vec::new();
    let mut closers          = Vec::new();
    let mut formality        = "neutral";
    let mut contraction_rate = 0.0_f32;
    let mut avg_sentence_len = 0.0_f32;
    let mut emoji_rate       = 0.0_f32;
    let mut has_stats        = false;

    for (t, k, v) in profile_data {
        match (t.as_str(), k.as_str()) {
            ("opener", _) => openers.push(k.as_str()),
            ("closer", _) => closers.push(k.as_str()),
            ("stat", "formality") => {
                let f: f32 = v.parse().unwrap_or(5.0);
                formality = if f >= 7.0 { "formal" } else if f <= 3.0 { "casual" } else { "neutral" };
                has_stats = true;
            }
            ("stat", "contraction_rate") => { contraction_rate = v.parse().unwrap_or(0.0); has_stats = true; }
            ("stat", "avg_sentence_len") => { avg_sentence_len  = v.parse().unwrap_or(0.0); has_stats = true; }
            ("stat", "emoji_rate")       => { emoji_rate        = v.parse().unwrap_or(0.0); has_stats = true; }
            _ => {}
        }
    }

    if openers.is_empty() && closers.is_empty() && !has_stats { return None; }

    let mut block = String::from("### USER VOICE DNA (learned from their actual writing — match this exactly):\n");

    if !openers.is_empty() {
        block.push_str(&format!("- Typical openers: {}\n", openers.iter().take(5).cloned().collect::<Vec<_>>().join(", ")));
    }
    if !closers.is_empty() {
        block.push_str(&format!("- Typical closers: {}\n", closers.iter().take(5).cloned().collect::<Vec<_>>().join(", ")));
    }
    if has_stats {
        block.push_str(&format!("- Typical register: {}\n", formality));
        let contraction_desc = if contraction_rate > 0.06 { "high (very casual, uses contractions freely)" }
            else if contraction_rate > 0.02 { "moderate" }
            else { "low (tends to write out full words)" };
        block.push_str(&format!("- Contraction rate: {}\n", contraction_desc));
        if avg_sentence_len > 0.0 {
            let len_desc = if avg_sentence_len > 20.0 { "long, elaborate sentences" }
                else if avg_sentence_len > 10.0 { "medium-length sentences" }
                else { "short, punchy sentences" };
            block.push_str(&format!("- Sentence style: {} (avg {:.0} words/sentence)\n", len_desc, avg_sentence_len));
        }
        if emoji_rate > 0.3 {
            block.push_str("- Uses emoji regularly — include relevant emoji where natural.\n");
        } else if emoji_rate < 0.05 && avg_sentence_len > 0.0 {
            block.push_str("- Rarely uses emoji — keep the output emoji-free.\n");
        }
    }

    block.push_str("Match this voice precisely. The output must sound like THIS specific person, not a generic AI.");
    Some(block)
}

// ── Memory block (Doc 2) ───────────────────────────────────────────────────

fn build_memory_block(memory: &[(String, String, String, String)]) -> String {
    if memory.is_empty() { return String::new(); }

    // Group by contact name
    let mut contacts: std::collections::HashMap<&str, Vec<(&str, &str, &str)>> =
        std::collections::HashMap::new();
    for (name, etype, attr, val) in memory {
        contacts.entry(name.as_str()).or_default()
            .push((etype.as_str(), attr.as_str(), val.as_str()));
    }

    let mut block = String::from("### RELATIONSHIP CONTEXT (learned from past interactions)\n");
    for (name, attrs) in &contacts {
        let mut parts = Vec::new();
        for (_, attr, val) in attrs {
            match *attr {
                "typical_tone" => {
                    let v: f32 = val.parse().unwrap_or(0.0);
                    let t = if v > 1.0 { "friendly" } else if v < -1.0 { "tense/professional" } else { "neutral" };
                    parts.push(format!("{} tone", t));
                }
                "formality" => {
                    let v: f32 = val.parse().unwrap_or(5.0);
                    let f = if v > 7.0 { "formal relationship" } else if v < 3.0 { "casual relationship" } else { "standard relationship" };
                    parts.push(f.to_string());
                }
                "opener" => parts.push(format!("usually starts with '{}'", val)),
                "closer" => parts.push(format!("usually signs off with '{}'", val)),
                "language" => parts.push(format!("communicates in {}", val)),
                _ => {}
            }
        }
        if !parts.is_empty() {
            block.push_str(&format!("- {}: {}\n", name, parts.join(", ")));
        }
    }
    block.push('\n');
    block
}

// ── RAG block (local history examples) ────────────────────────────────────

fn build_rag_block(examples: &[(String, String)]) -> String {
    if examples.is_empty() { return String::new(); }
    let mut block = String::from(
        "\n\n### REFERENCE EXAMPLES (from this user's own past — match their style and format exactly):\n"
    );
    for (input, output) in examples.iter().take(3) {
        let input_snip:  String = input.chars().take(200).collect();
        let output_snip: String = output.chars().take(600).collect();
        block.push_str(&format!("Past Input:  {}\nPast Output: {}\n---\n", input_snip, output_snip));
    }
    block
}

// ── Public API ─────────────────────────────────────────────────────────────

// ── App context block ──────────────────────────────────────────────────────

fn build_app_context_block(app_category: Option<&str>) -> String {
    match app_category {
        Some("code_editor") =>
            "APP CONTEXT: The user is working in a code editor. \
             Prefer technical, precise language. Code snippets and structured output are welcome.\n\n".into(),
        Some("email_client") =>
            "APP CONTEXT: The user is in an email client. \
             Default to professional email conventions unless the tone signals otherwise.\n\n".into(),
        Some("messaging") =>
            "APP CONTEXT: The user is in a messaging app (Slack/Teams/Discord). \
             Keep the output concise and conversational. Markdown tables/code blocks are fine.\n\n".into(),
        Some("browser") =>
            "APP CONTEXT: The user is in a web browser. \
             Context may be a web form, chat, or document — adapt the format to fit.\n\n".into(),
        Some("office") =>
            "APP CONTEXT: The user is in a Microsoft Office application. \
             Prefer structured, professional prose suitable for documents or spreadsheets.\n\n".into(),
        Some("terminal") =>
            "APP CONTEXT: The user is in a terminal. \
             Prefer shell-friendly output — avoid markdown unless it aids readability.\n\n".into(),
        Some("notes") =>
            "APP CONTEXT: The user is in a note-taking app. \
             Markdown formatting is ideal; keep output well-structured.\n\n".into(),
        _ => String::new(),
    }
}

pub fn build_prompt(
    app: &tauri::AppHandle,
    ctx: &TextContext,
    mode: &str,
    sub_mode: Option<&str>,
    profile_data: &[(String, String, String)],
    memory_data: &[(String, String, String, String)],
    rag_examples: &[(String, String)],
    contact_language: Option<&str>,
    app_context: Option<&str>,
    thread_context: Option<&str>,
    contact_examples: &[(String, String)],
) -> String {
    // Sanitize user text before it flows into any prompt block
    let mut sanitized_ctx = ctx.clone();
    sanitized_ctx.original = sanitize(&ctx.original);

    // Language block is suppressed for modes where output language differs from input
    // (e.g. user writes in Hinglish but wants English output — the language hint
    // would confuse the model into replying in Hinglish)
    let language_block = match mode {
        "Do" | "Email" | "Correct" | "Prompt" | "Knowledge" | "Strategist" | "Professional" => String::new(),
        _ => build_language_block(&sanitized_ctx),
    };
    let context_block   = build_context_block(&sanitized_ctx);
    let task_block      = build_task_block(mode, sub_mode, &sanitized_ctx);
    let constraint_block = build_constraint_block(mode, &sanitized_ctx);

    let mut prompt = if language_block.is_empty() {
        format!(
            "SYSTEM: You are an elite AI writing assistant embedded in a universal keyboard tool. \
             You process text captured from any application on the user's device.\n\n\
             TEXT ANALYSIS: {}\n\n\
             YOUR TASK: {}\n\n\
             OUTPUT RULES: {}",
            context_block, task_block, constraint_block
        )
    } else {
        format!(
            "SYSTEM: You are an elite AI writing assistant embedded in a universal keyboard tool. \
             You process text captured from any application on the user's device.\n\n\
             LANGUAGE INFO: {}\n\n\
             TEXT ANALYSIS: {}\n\n\
             YOUR TASK: {}\n\n\
             OUTPUT RULES: {}",
            language_block, context_block, task_block, constraint_block
        )
    };

    prompt.push_str(&build_app_context_block(app_context));
    prompt.push_str(&build_team_voice_block(app));

    if let Some(voice) = build_voice_block(profile_data) {
        prompt.push_str(&format!("\n\n[VOICE PROFILE]\n{}", voice));
    }

    prompt.push_str(&build_memory_block(memory_data));

    if mode == "Reply" {
        // Thread context — full conversation gives much richer reply signal than one line.
        if let Some(thread) = thread_context {
            prompt.push_str(&format!("\n\n{}", thread));
        }
        // Contact-specific accepted replies — highest-quality few-shot signal.
        if !contact_examples.is_empty() {
            prompt.push_str(
                "\n\n### YOUR PAST ACCEPTED REPLIES TO THIS CONTACT \
                 (you reviewed and sent these — match this tone/style/length exactly):\n"
            );
            for (their_msg, your_reply) in contact_examples.iter().take(4) {
                let their: String = their_msg.chars().take(200).collect();
                let yours: String = your_reply.chars().take(400).collect();
                prompt.push_str(&format!(
                    "They said: {}\nYou replied: {}\n---\n", their, yours
                ));
            }
        }
        // Fall back to general history examples only if no contact-specific ones.
        if contact_examples.is_empty() {
            prompt.push_str(&build_rag_block(rag_examples));
        }
        // Contact language override.
        if let Some(lang) = contact_language {
            prompt.push_str(&format!(
                "\n\nCONTACT LANGUAGE: Based on past conversations, this person communicates in {}. \
                 Your reply MUST be in {}.",
                lang, lang
            ));
        }
    } else {
        // RAG for all non-Reply modes except deterministic ones.
        match mode {
            "Correct" | "Translate" => {}
            _ => { prompt.push_str(&build_rag_block(rag_examples)); }
        }
    }

    // DO NOT append text here — ai.rs make_request already appends it as "Input: {text}"
    // Appending here caused the model to receive the text twice, inflating prompt size
    // and causing output truncation on Prompt mode.
    prompt
}