package com.snaptext.keyboard.nlp

/**
 * Lightweight intent detection for mobile.
 * Simplified from the desktop's 35-signal engine to ~10 core signals.
 */
object IntentDetector {

    data class IntentResult(
        val suggestedMode: String,
        val confidence: Float,
        val reason: String
    )

    private val GREETING_PATTERNS = Regex(
        "^(hi|hello|hey|dear|respected|good morning|good evening|good afternoon)",
        RegexOption.IGNORE_CASE
    )
    private val QUESTION_STARTERS = Regex(
        "^(how|what|why|when|where|who|which|can|could|should|would|is|are|do|does|will)",
        RegexOption.IGNORE_CASE
    )
    private val EMAIL_SIGNALS = Regex(
        "(subject:|dear |regards|sincerely|attached|please find|forwarding|cc:|bcc:)",
        RegexOption.IGNORE_CASE
    )
    private val CODE_PATTERNS = Regex(
        "(function |def |class |const |let |var |import |#include|pub fn|interface |=>|\\{\\s*\\})"
    )
    private val ERROR_PATTERNS = Regex(
        "(Error:|error:|Exception|Traceback|FAILED|panic!|errno|stack trace|Caused by)",
        RegexOption.IGNORE_CASE
    )

    fun detect(text: String, language: LanguageDetector.LanguageResult): IntentResult {
        val trimmed = text.trim()
        val wordCount = trimmed.split(Regex("\\s+")).size
        val hasQuestion = trimmed.contains("?")

        // Priority 1: Hinglish → always suggest Correct (rewrite to English)
        if (language.isHinglish) {
            return IntentResult("Correct", 0.9f, "Hinglish detected — rewrite to English")
        }

        // Priority 2: Devanagari/mixed → Correct
        if (language.primaryScript == LanguageDetector.ScriptType.DEVANAGARI || language.isMixed) {
            return IntentResult("Correct", 0.85f, "Non-English script — rewrite to English")
        }

        // Priority 3: Code or errors → Prompt
        if (ERROR_PATTERNS.containsMatchIn(trimmed)) {
            return IntentResult("Prompt", 0.9f, "Error/stack trace detected")
        }
        if (CODE_PATTERNS.containsMatchIn(trimmed)) {
            return IntentResult("Prompt", 0.85f, "Code snippet detected")
        }

        // Priority 4: Email signals
        if (EMAIL_SIGNALS.containsMatchIn(trimmed)) {
            return IntentResult("Email", 0.85f, "Email structure detected")
        }

        // Priority 5: Greeting + instruction → Reply (compose a message)
        if (GREETING_PATTERNS.containsMatchIn(trimmed) || isInstruction(trimmed)) {
            return IntentResult("Reply", 0.8f, "Looks like a message instruction")
        }

        // Priority 6: Question → Knowledge
        if (hasQuestion || QUESTION_STARTERS.containsMatchIn(trimmed)) {
            return IntentResult("Knowledge", 0.75f, "Question detected")
        }

        // Priority 7: Long text → Summarize
        if (wordCount > 100) {
            return IntentResult("Summarize", 0.7f, "Long text — suggest summarize")
        }

        // Default: Professional polish
        return IntentResult("Professional", 0.6f, "General text — suggest professional rewrite")
    }

    private fun isInstruction(text: String): Boolean {
        val lower = text.lowercase()
        // Patterns that suggest user wants to compose something
        return lower.startsWith("tell ") ||
                lower.startsWith("write ") ||
                lower.startsWith("reply ") ||
                lower.startsWith("say ") ||
                lower.startsWith("ask ") ||
                lower.startsWith("inform ") ||
                lower.startsWith("send ") ||
                lower.contains("usko bol") ||
                lower.contains("bol de") ||
                lower.contains("likh de") ||
                lower.contains("reply kar") ||
                lower.contains("mail kar")
    }
}
