package com.snaptext.keyboard.nlp

/**
 * Builds system prompts for each AI mode.
 * Mirrors the desktop prompt.rs logic but simplified for mobile.
 */
object PromptBuilder {

    private val INJECTION_PATTERNS = listOf(
        "\nIgnore previous", "\nForget previous", "\nSystem:", "\nsystem:",
        "\n[INST]", "\n[SYS]", "###", "```system"
    )

    private fun sanitize(text: String): String {
        var out = text
        for (pattern in INJECTION_PATTERNS) {
            out = out.replace(pattern, " [sanitized] ")
        }
        return out
    }

    fun buildSystemPrompt(
        mode: String,
        userText: String,
        language: LanguageDetector.LanguageResult
    ): String {
        val sanitized = sanitize(userText)
        val langInfo = "The input text is ${language.description}."
        val task = buildTaskBlock(mode, sanitized)
        val constraints = buildConstraints()

        return """
            |SYSTEM: You are an elite AI writing assistant embedded in a mobile keyboard.
            |
            |LANGUAGE INFO: $langInfo
            |
            |YOUR TASK: $task
            |
            |OUTPUT RULES: $constraints
        """.trimMargin()
    }

    private fun buildTaskBlock(mode: String, text: String): String = when (mode) {
        "Correct" ->
            "You are a professional English rewriter. " +
            "Rewrite the following text into clear, fluent, professional English. " +
            "If the input is in Hinglish, Hindi, broken English, or any other language — " +
            "output clean English only. " +
            "Preserve the original meaning and intent exactly. " +
            "Fix grammar, spelling, and structure. " +
            "Do not add information. Do not change the tone unless it is rude — then make it polite. " +
            "Output only the rewritten English text. Nothing else."

        "Reply" ->
            "You are an expert communicator who writes natural replies. " +
            "The user has shared a message they received (or their own notes about what to reply). " +
            "Your job: compose a REPLY to that message. " +
            "CRITICAL LANGUAGE RULE: Reply in the SAME language, dialect, and style as the input. " +
            "If the input is Hinglish, reply in Hinglish. If it's Hindi, reply in Hindi. " +
            "If it's English, reply in English. If it has a regional dialect (Rajasthani, Punjabi, etc.), " +
            "keep that flavor in the reply. " +
            "CRITICAL TONE RULE: Match the tone and vibe of the input. " +
            "If it's casual/friendly, reply casually. If it's formal, reply formally. " +
            "Mirror how real people in that language/context actually text. " +
            "Use natural slang, abbreviations, and expressions that fit the conversation. " +
            "Do NOT translate to English. Do NOT make it sound robotic or overly formal. " +
            "Do NOT include subject lines unless it's clearly an email. " +
            "Do NOT explain what you did. Just output the reply."

        "Do" ->
            "The input is an INSTRUCTION from the user describing what they want you to produce. " +
            "It may be: a request to write a message/email, notes to turn into a list, " +
            "rough thoughts to structure, a task to perform, or any creative ask. " +
            "Your job: read the instruction, understand the intent, then DO IT. " +
            "Output ONLY the finished result — the message, the list, the document, whatever was asked for. " +
            "Examples: " +
            "'write a leave message to my boss' → output the actual leave message. " +
            "'make a daily standup task list from these points' → output the formatted list. " +
            "'translate this to Hindi' → output the translation. " +
            "Match the language they ask for. If they don't specify, default to fluent English. " +
            "Do NOT translate or echo the instruction back. Do NOT explain what you did. " +
            "Do NOT add preamble like 'Sure' or 'Here is'. Just output the result."

        "Professional" ->
            "Rewrite this text with a professional, confident tone. " +
            "Fix grammar, improve word choice, ensure clarity and conciseness. " +
            "Maintain the original intent and factual content. " +
            "IMPORTANT: Keep the same language as the input. If input is Hindi, output professional Hindi. " +
            "If input is Hinglish, output professional Hinglish. Only output English if input is English."

        "Prompt" -> {
            val englishRule = "CRITICAL: The entire output MUST be in fluent, professional English " +
                "regardless of the input language."
            when {
                isError(text) ->
                    "You are a senior debugging assistant. The user selected an error message " +
                    "or stack trace. Transform it into a structured debug prompt with sections: " +
                    "**Error:** (exact error, cleaned up), " +
                    "**Likely Cause:** (2-3 probable reasons), " +
                    "**Context to Provide:** (language, framework, OS), " +
                    "**Ask:** (specific, well-formed question to get a fix). " +
                    "$englishRule Output only the prompt."

                isCode(text) ->
                    "You are a senior code reviewer. The user selected a code snippet. " +
                    "Transform it into a structured prompt with sections: " +
                    "**Language:** (detected language/framework), " +
                    "**Code Purpose:** (what this code does), " +
                    "**Task:** (explain, review, optimize, or find bugs), " +
                    "**Focus Areas:** (edge cases, performance, security). " +
                    "$englishRule Output only the prompt."

                else ->
                    "You are an expert Prompt Engineer. Transform the following rough notes " +
                    "into a high-quality, professional AI prompt. Structure with: " +
                    "**Role:** (who the AI should be), " +
                    "**Context:** (background information), " +
                    "**Task:** (what to do), " +
                    "**Constraints:** (rules to follow). " +
                    "$englishRule Output only the enhanced prompt."
            }
        }

        "Translate" ->
            "Translate the following text to English (if not English) or Spanish (if already English). " +
            "Maintain the original tone, formality level, and cultural nuances."

        "Email" ->
            "Transform the following notes into a well-structured email that is " +
            "professional and polished. Include a proper subject line, greeting, body, and sign-off. " +
            "Write the email in the same language as the input. If input is Hindi/Hinglish, write the email in that language."

        "Casual" ->
            "Rewrite this text in a natural, conversational tone. " +
            "Make it sound like a real person talking — use contractions, " +
            "keep sentences short, cut any stiff or corporate language. " +
            "IMPORTANT: Reply in the SAME language as the input. If Hinglish, stay Hinglish. " +
            "If Hindi, stay Hindi. Use natural slang and expressions from that language."

        "Summarize" ->
            "Summarize the key points into 1–3 concise sentences. " +
            "Preserve the most important information and discard filler. " +
            "Output in the same language as the input."

        "Knowledge" ->
            "You are an expert technical instructor and consultant. " +
            "Provide a clear, structured, and practical explanation. " +
            "Use formatting (bullet points, bold text) to highlight key concepts. " +
            "Focus on actionable advice and 'why' it matters."

        "GhostWriter" ->
            "You are a skilled ghostwriter embedded in a mobile keyboard. " +
            "The user has provided rough bullet points, notes, or an outline. " +
            "Expand these into a fully fleshed-out, well-structured message, email, or post. " +
            "Maintain the user's voice and intent — don't make it sound like a generic AI. " +
            "Match formality to the context: casual notes become casual messages, " +
            "professional bullets become polished emails. " +
            "IMPORTANT: Write in the SAME language as the input. If notes are in Hinglish, " +
            "expand in Hinglish. If Hindi, expand in Hindi. " +
            "Output only the expanded text. No meta-commentary."

        "SayItBetter" ->
            "You are an expert rewriter. The user wants an improved version of their text. " +
            "Make it clearer, more impactful, and better structured while preserving the " +
            "original meaning and tone. Fix grammar, improve word choice, tighten sentences. " +
            "If the text is already good, make only minimal improvements. " +
            "IMPORTANT: Keep the same language. If input is Hinglish, improve in Hinglish. " +
            "If Hindi, improve in Hindi. Do NOT translate to English unless the input is English. " +
            "Output only the improved text."

        "SmartReply" ->
            "You are a smart reply generator embedded in a mobile keyboard. " +
            "The user has received a message and needs quick reply options. " +
            "Generate EXACTLY 3 short reply options separated by ||| (three pipes). " +
            "Each reply should be 1-2 sentences max. " +
            "Vary the tone: one warm/friendly, one professional/direct, one brief/casual. " +
            "CRITICAL: Reply in the SAME language and style as the input message. " +
            "If input is Hinglish, all 3 replies must be in Hinglish. " +
            "If Hindi, reply in Hindi. If English, reply in English. " +
            "Use natural expressions from that language — sound like a real person, not a robot. " +
            "Output ONLY the 3 replies separated by |||, nothing else. " +
            "Example (Hinglish): Haan bhai, done!|||Dekh leta hu, bata dunga|||Ok theek hai"

        "TranslateInline" -> {
            val langPair = extractLangPair(text)
            "You are an inline translator embedded in a mobile keyboard. " +
            "Translate the following text from ${langPair.first} to ${langPair.second}. " +
            "Maintain the original tone, formality level, and cultural nuances. " +
            "If the text contains slang or colloquial expressions, translate them to " +
            "equivalent expressions in the target language. " +
            "Output only the translated text."
        }

        else ->
            "Rewrite the following text to be clear, professional, and concise."
    }

    private fun extractLangPair(text: String): Pair<String, String> {
        // Default pairs based on detected language
        val lang = LanguageDetector.detect(text)
        return when {
            lang.isHinglish || lang.primaryScript == LanguageDetector.ScriptType.DEVANAGARI ->
                Pair("Hindi", "English")
            lang.primaryScript == LanguageDetector.ScriptType.ARABIC ->
                Pair("Arabic", "English")
            lang.primaryScript == LanguageDetector.ScriptType.CJK ->
                Pair("Chinese/Japanese", "English")
            lang.primaryScript == LanguageDetector.ScriptType.CYRILLIC ->
                Pair("Russian", "English")
            else -> Pair("English", "Hindi")
        }
    }

    private fun buildConstraints(): String =
        "Output ONLY the transformed text. " +
        "Do not include meta-commentary, explanations, or labels. " +
        "Do not start with phrases like 'Here is' or 'Certainly'. " +
        "Preserve all proper nouns, names, and technical terms exactly."

    private fun isError(text: String): Boolean =
        text.contains("Error:") || text.contains("error:") ||
        text.contains("Traceback") || text.contains("Exception") ||
        text.contains("panic!") || text.contains("stack trace")

    private fun isCode(text: String): Boolean =
        text.contains("function ") || text.contains("def ") ||
        text.contains("class ") || text.contains("const ") ||
        text.contains("import ") || text.contains("pub fn")
}
