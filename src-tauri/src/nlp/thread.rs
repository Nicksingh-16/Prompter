/// Conversation thread parser.
/// Detects multi-message threads copied from WhatsApp, iMessage, Slack, or any
/// app that uses "Name: message" formatting, and extracts structured messages
/// so the AI gets full conversation context instead of just the last line.

pub struct ThreadMessage {
    pub sender: String,
    pub text:   String,
}

pub struct ParsedThread {
    pub messages:     Vec<ThreadMessage>,
    pub last_sender:  String,
    /// The contact we are replying to (last sender, excluding self-references).
    pub contact_name: Option<String>,
}

/// Try to parse `raw` as a conversation thread.
/// Returns `None` if the text looks like a single standalone message.
pub fn parse_thread(raw: &str) -> Option<ParsedThread> {
    // Need at least 2 non-empty lines to even consider thread parsing.
    let non_empty: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
    if non_empty.len() < 2 { return None; }

    // Strategy 1: WhatsApp Desktop — [HH:MM AM/PM] Name: message
    if let Some(t) = try_whatsapp_desktop(raw) {
        if t.messages.len() >= 2 { return Some(t); }
    }

    // Strategy 2: Generic "Name: message" lines (most messaging apps)
    if let Some(t) = try_name_colon(raw) {
        if t.messages.len() >= 2 { return Some(t); }
    }

    None
}

// ── Strategy 1: WhatsApp Desktop ──────────────────────────────────────────

fn try_whatsapp_desktop(raw: &str) -> Option<ParsedThread> {
    let mut messages = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if !line.starts_with('[') { continue; }
        let Some(bracket_end) = line.find(']') else { continue };
        let after = line[bracket_end + 1..].trim_start();
        let Some(colon) = after.find(": ") else { continue };
        let sender = after[..colon].trim().to_string();
        let text   = after[colon + 2..].trim().to_string();
        if sender.is_empty() || text.is_empty() || sender.len() > 60 { continue; }
        // Skip system messages like "Messages and calls are end-to-end encrypted"
        if text.len() < 4 { continue; }
        messages.push(ThreadMessage { sender, text });
    }
    build_thread(messages)
}

// ── Strategy 2: Name: message ─────────────────────────────────────────────

fn try_name_colon(raw: &str) -> Option<ParsedThread> {
    let mut messages: Vec<ThreadMessage> = Vec::new();
    let mut unique_senders: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }

        let Some(colon) = line.find(": ") else { continue };
        let potential_name = line[..colon].trim();

        // Sender name heuristics:
        // - 1–3 words, no leading digit, under 50 chars
        // - Not all-caps (avoids "NOTE: ...", "WARNING: ...")
        // - No special chars except space/hyphen/apostrophe
        let word_count = potential_name.split_whitespace().count();
        let all_caps   = potential_name.chars().filter(|c| c.is_alphabetic()).all(|c| c.is_uppercase());
        let valid_chars = potential_name.chars().all(|c| c.is_alphanumeric() || " -'.".contains(c));

        if word_count < 1 || word_count > 3 { continue; }
        if potential_name.len() > 50 { continue; }
        if potential_name.starts_with(|c: char| c.is_ascii_digit()) { continue; }
        if all_caps { continue; }
        if !valid_chars { continue; }

        let sender = potential_name.to_string();
        let text   = line[colon + 2..].trim().to_string();
        if text.is_empty() || text.len() < 2 { continue; }

        unique_senders.insert(sender.clone());
        messages.push(ThreadMessage { sender, text });
    }

    // Require at least 2 distinct sender names to confirm it's a thread.
    // Single-sender with 4+ messages is also accepted (quoted conversation).
    if unique_senders.len() < 2 && messages.len() < 4 { return None; }

    build_thread(messages)
}

// ── Shared builder ─────────────────────────────────────────────────────────

fn build_thread(messages: Vec<ThreadMessage>) -> Option<ParsedThread> {
    if messages.is_empty() { return None; }

    let last_sender = messages.last().unwrap().sender.clone();

    // Heuristic: "Me", "You", "I" are self-references — skip them to find contact.
    const SELF_NAMES: &[&str] = &["me", "you", "i", "myself", "main", "mujhe"];
    let is_self = |s: &str| SELF_NAMES.contains(&s.to_lowercase().as_str());

    let contact_name = if is_self(&last_sender) {
        messages.iter().rev().skip(1)
            .find(|m| !is_self(&m.sender))
            .map(|m| m.sender.clone())
    } else {
        Some(last_sender.clone())
    };

    Some(ParsedThread { messages, last_sender, contact_name })
}

// ── Prompt formatter ───────────────────────────────────────────────────────

/// Format a parsed thread into a prompt context block.
pub fn format_for_prompt(thread: &ParsedThread) -> String {
    let mut out = String::from(
        "CONVERSATION THREAD (chronological — most recent message last):\n"
    );
    // Show at most the last 10 messages to stay within token budget.
    let start = thread.messages.len().saturating_sub(10);
    for msg in &thread.messages[start..] {
        let snippet: String = msg.text.chars().take(300).collect();
        out.push_str(&format!("  {}: {}\n", msg.sender, snippet));
    }
    if let Some(ref contact) = thread.contact_name {
        out.push_str(&format!(
            "\nYou are replying to {}'s latest message above. \
             Use the full thread for context — tone, history, relationship.",
            contact
        ));
    }
    out
}
