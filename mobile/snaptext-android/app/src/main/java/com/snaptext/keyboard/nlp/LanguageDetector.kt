package com.snaptext.keyboard.nlp

/**
 * Lightweight language/script detection for mobile.
 * Detects Hinglish, Devanagari, Latin, Arabic, and mixed scripts.
 */
object LanguageDetector {

    enum class ScriptType {
        LATIN, DEVANAGARI, ARABIC, CJK, CYRILLIC, MIXED, UNKNOWN
    }

    data class LanguageResult(
        val primaryScript: ScriptType,
        val isHinglish: Boolean,
        val isMixed: Boolean,
        val description: String
    )

    // Common Hinglish markers (Hindi words written in Latin script)
    private val HINGLISH_MARKERS = setOf(
        "kya", "hai", "nahi", "mujhe", "tujhe", "kaise", "kahan", "kab",
        "acha", "theek", "bhai", "yaar", "abhi", "bahut", "kuch", "aur",
        "lekin", "kyunki", "wala", "wali", "karke", "bolna", "batao",
        "samajh", "pata", "suno", "dekho", "chalo", "arre", "haan",
        "matlab", "isliye", "waise", "sahi", "galat", "accha", "thik",
        "karo", "karna", "raha", "rahi", "rahe", "hoga", "hogi",
        "chahiye", "zaruri", "pehle", "baad", "saath", "liye", "uske",
        "iske", "unka", "mera", "tera", "humara", "tumhara", "dost",
        "baat", "kaam", "paisa", "ghar", "log", "sab", "bohot"
    )

    fun detect(text: String): LanguageResult {
        if (text.isBlank()) return LanguageResult(ScriptType.UNKNOWN, false, false, "empty")

        var latinCount = 0
        var devanagariCount = 0
        var arabicCount = 0
        var cjkCount = 0
        var cyrillicCount = 0
        var total = 0

        for (char in text) {
            if (char.isWhitespace() || char.isDigit()) continue
            total++
            when {
                char in '\u0041'..'\u007A' -> latinCount++
                char in '\u0900'..'\u097F' -> devanagariCount++
                char in '\u0600'..'\u06FF' -> arabicCount++
                char in '\u4E00'..'\u9FFF' || char in '\u3040'..'\u30FF' -> cjkCount++
                char in '\u0400'..'\u04FF' -> cyrillicCount++
            }
        }

        if (total == 0) return LanguageResult(ScriptType.UNKNOWN, false, false, "no script chars")

        val latinPct = latinCount.toFloat() / total
        val devanagariPct = devanagariCount.toFloat() / total

        // Check for Hinglish (Latin script with Hindi words)
        val isHinglish = latinPct > 0.7f && hasHinglishMarkers(text)

        // Check for mixed Devanagari + Latin
        val isMixed = devanagariPct > 0.1f && latinPct > 0.1f

        val primaryScript = when {
            devanagariPct > 0.5f -> ScriptType.DEVANAGARI
            arabicCount > total / 2 -> ScriptType.ARABIC
            cjkCount > total / 2 -> ScriptType.CJK
            cyrillicCount > total / 2 -> ScriptType.CYRILLIC
            isMixed -> ScriptType.MIXED
            else -> ScriptType.LATIN
        }

        val description = when {
            isHinglish -> "Hinglish (Hindi-English mix in Latin script)"
            isMixed -> "Mixed Devanagari and Latin script"
            primaryScript == ScriptType.DEVANAGARI -> "Hindi (Devanagari script)"
            primaryScript == ScriptType.LATIN -> "English or Latin-script language"
            else -> primaryScript.name.lowercase()
        }

        return LanguageResult(primaryScript, isHinglish, isMixed, description)
    }

    private fun hasHinglishMarkers(text: String): Boolean {
        val words = text.lowercase().split(Regex("[\\s,;.!?]+"))
        val matchCount = words.count { it in HINGLISH_MARKERS }
        // If 2+ Hinglish words or >15% of words are Hinglish markers
        return matchCount >= 2 || (words.size > 3 && matchCount.toFloat() / words.size > 0.15f)
    }
}
