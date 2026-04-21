package com.snaptext.keyboard.nlp

/**
 * Real-time tone analysis for Tone Guard feature.
 * Analyzes text as user types and returns a tone level with feedback.
 */
object ToneAnalyzer {

    enum class ToneLevel {
        POSITIVE,   // Green — friendly, warm, professional
        NEUTRAL,    // No indicator — plain text
        CAUTION,    // Yellow — slightly aggressive or passive-aggressive
        HARSH       // Red — hostile, rude, or very aggressive
    }

    data class ToneResult(
        val level: ToneLevel,
        val score: Float,           // -5.0 (harsh) to +5.0 (warm)
        val frictionPhrases: List<String>,
        val suggestion: String?
    )

    // Passive-aggressive / friction phrases
    private val FRICTION_PHRASES = mapOf(
        "per my last email" to "This can sound passive-aggressive",
        "as i already mentioned" to "May feel dismissive",
        "as previously stated" to "Can sound condescending",
        "i was under the impression" to "May sound accusatory",
        "going forward" to null, // mild, no suggestion
        "with all due respect" to "Often precedes disagreement",
        "just to be clear" to "Can sound patronizing",
        "i'm not sure why" to "May sound frustrated",
        "not sure if you noticed" to "Can sound passive-aggressive",
        "as per my understanding" to "Can feel cold",
        "kindly do the needful" to "Overly formal, consider being direct",
        "please revert" to "Consider 'please reply' instead",
        "correct me if i'm wrong" to "Can sound sarcastic",
        "no offense but" to "Usually precedes something offensive",
        "i think you forgot" to "Can sound accusatory",
        "you should have" to "Sounds blaming",
        "you always" to "Generalizing can escalate conflict",
        "you never" to "Generalizing can escalate conflict",
        "whatever you think" to "Can sound dismissive",
        "fine" to null, // context-dependent
        "noted" to "Can feel cold — try 'Got it, thanks!'",
        "k" to "Very brief — might seem dismissive",
        "obviously" to "Can sound condescending",
        "clearly" to "Can sound condescending",
        "actually" to null, // mild
        "basically" to null,
    )

    // Harsh / hostile words
    private val HARSH_WORDS = setOf(
        "stupid", "idiot", "dumb", "useless", "pathetic", "incompetent",
        "ridiculous", "absurd", "trash", "garbage", "terrible", "awful",
        "worst", "hate", "disgusting", "shut up", "wtf", "stfu",
        "bullshit", "crap", "damn", "hell", "pissed", "furious"
    )

    // Positive tone markers
    private val POSITIVE_MARKERS = setOf(
        "thank you", "thanks", "appreciate", "grateful", "great job",
        "well done", "excellent", "wonderful", "happy to", "glad to",
        "looking forward", "excited", "love", "amazing", "awesome",
        "please", "kindly", "hope you're", "hope this helps",
        "take care", "best regards", "warm regards", "cheers"
    )

    // ALL-CAPS detection threshold
    private const val CAPS_WORD_THRESHOLD = 3

    fun analyze(text: String): ToneResult {
        if (text.isBlank() || text.length < 3) {
            return ToneResult(ToneLevel.NEUTRAL, 0f, emptyList(), null)
        }

        val lower = text.lowercase().trim()
        val words = lower.split(Regex("\\s+"))
        var score = 0f
        val detectedFriction = mutableListOf<String>()
        var suggestion: String? = null

        // Check friction phrases
        for ((phrase, tip) in FRICTION_PHRASES) {
            if (lower.contains(phrase)) {
                score -= if (tip != null) 1.5f else 0.5f
                if (tip != null) {
                    detectedFriction.add(phrase)
                    if (suggestion == null) suggestion = tip
                }
            }
        }

        // Check harsh words
        for (harsh in HARSH_WORDS) {
            if (lower.contains(harsh)) {
                score -= 3f
                if (suggestion == null) suggestion = "Strong language detected — consider softening"
                detectedFriction.add(harsh)
            }
        }

        // Check positive markers
        for (positive in POSITIVE_MARKERS) {
            if (lower.contains(positive)) {
                score += 1.5f
            }
        }

        // ALL-CAPS detection (shouting)
        val capsWords = words.count { it.length > 2 && it == it.uppercase() && it.any { c -> c.isLetter() } }
        if (capsWords >= CAPS_WORD_THRESHOLD) {
            score -= 2f
            if (suggestion == null) suggestion = "All-caps can feel like shouting"
        }

        // Excessive exclamation/question marks
        val excessivePunctuation = Regex("[!?]{3,}").containsMatchIn(text)
        if (excessivePunctuation) {
            score -= 1f
            if (suggestion == null) suggestion = "Multiple punctuation can seem aggressive"
        }

        // Clamp score
        score = score.coerceIn(-5f, 5f)

        val level = when {
            score <= -3f -> ToneLevel.HARSH
            score <= -1f -> ToneLevel.CAUTION
            score >= 2f -> ToneLevel.POSITIVE
            else -> ToneLevel.NEUTRAL
        }

        return ToneResult(level, score, detectedFriction, suggestion)
    }
}
