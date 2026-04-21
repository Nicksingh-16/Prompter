package com.snaptext.keyboard.clipboard

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import org.json.JSONArray
import org.json.JSONObject

/**
 * Smart Clipboard Hub — categorizes, pins, and manages clipboard history.
 * Persists to SharedPreferences as JSON.
 */
class ClipboardHub(private val context: Context) {

    enum class ClipCategory {
        LINK, EMAIL_ADDRESS, PHONE, CODE, ADDRESS, NUMBER, TEXT
    }

    data class ClipItem(
        val text: String,
        val category: ClipCategory,
        val timestamp: Long,
        val isPinned: Boolean = false
    )

    private val prefs = context.getSharedPreferences("snaptext_clipboard", Context.MODE_PRIVATE)
    private val items = mutableListOf<ClipItem>()
    private val maxHistory = 30

    init {
        loadFromPrefs()
    }

    fun getItems(): List<ClipItem> {
        // Pinned first, then by recency
        return items.sortedWith(compareByDescending<ClipItem> { it.isPinned }.thenByDescending { it.timestamp })
    }

    fun getByCategory(category: ClipCategory): List<ClipItem> {
        return getItems().filter { it.category == category }
    }

    fun addClip(text: String) {
        if (text.isBlank()) return
        // Remove duplicate
        items.removeAll { it.text == text && !it.isPinned }
        val category = categorize(text)
        items.add(0, ClipItem(text, category, System.currentTimeMillis()))
        // Trim non-pinned items
        val nonPinned = items.filter { !it.isPinned }
        if (nonPinned.size > maxHistory) {
            val toRemove = nonPinned.takeLast(nonPinned.size - maxHistory)
            items.removeAll(toRemove.toSet())
        }
        saveToPrefs()
    }

    fun togglePin(text: String) {
        val index = items.indexOfFirst { it.text == text }
        if (index >= 0) {
            val item = items[index]
            items[index] = item.copy(isPinned = !item.isPinned)
            saveToPrefs()
        }
    }

    fun removeClip(text: String) {
        items.removeAll { it.text == text }
        saveToPrefs()
    }

    fun clearNonPinned() {
        items.removeAll { !it.isPinned }
        saveToPrefs()
    }

    fun pasteToInput(text: String) {
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboard.setPrimaryClip(ClipData.newPlainText("SnapText", text))
    }

    // ── Categorization ──────────────────────────────────────────────────────

    private fun categorize(text: String): ClipCategory {
        val trimmed = text.trim()
        return when {
            URL_PATTERN.containsMatchIn(trimmed) -> ClipCategory.LINK
            EMAIL_PATTERN.containsMatchIn(trimmed) -> ClipCategory.EMAIL_ADDRESS
            PHONE_PATTERN.containsMatchIn(trimmed) -> ClipCategory.PHONE
            CODE_PATTERN.containsMatchIn(trimmed) -> ClipCategory.CODE
            NUMBER_PATTERN.matches(trimmed) -> ClipCategory.NUMBER
            ADDRESS_INDICATORS.any { trimmed.lowercase().contains(it) } -> ClipCategory.ADDRESS
            else -> ClipCategory.TEXT
        }
    }

    companion object {
        private val URL_PATTERN = Regex("https?://[\\w\\-.]+\\.[a-z]{2,}[/\\w\\-._~:/?#@!$&'()*+,;=%]*", RegexOption.IGNORE_CASE)
        private val EMAIL_PATTERN = Regex("[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}")
        private val PHONE_PATTERN = Regex("^[+]?[\\d\\s()-]{7,15}$")
        private val CODE_PATTERN = Regex("(function |def |class |const |let |var |import |=>|\\{\\s*\\}|<[a-z]+>|</)")
        private val NUMBER_PATTERN = Regex("^[\\d,.]+$")
        private val ADDRESS_INDICATORS = listOf(
            "street", "road", "avenue", "lane", "blvd", "apt", "floor",
            "city", "state", "zip", "pin code", "sector", "block", "nagar"
        )

        fun categoryIcon(category: ClipCategory): String = when (category) {
            ClipCategory.LINK -> "\uD83D\uDD17"          // 🔗
            ClipCategory.EMAIL_ADDRESS -> "\uD83D\uDCE7" // 📧
            ClipCategory.PHONE -> "\uD83D\uDCDE"         // 📞
            ClipCategory.CODE -> "\uD83D\uDCBB"          // 💻
            ClipCategory.ADDRESS -> "\uD83D\uDCCD"       // 📍
            ClipCategory.NUMBER -> "\uD83D\uDD22"        // 🔢
            ClipCategory.TEXT -> "\uD83D\uDCDD"           // 📝
        }

        fun categoryLabel(category: ClipCategory): String = when (category) {
            ClipCategory.LINK -> "Link"
            ClipCategory.EMAIL_ADDRESS -> "Email"
            ClipCategory.PHONE -> "Phone"
            ClipCategory.CODE -> "Code"
            ClipCategory.ADDRESS -> "Address"
            ClipCategory.NUMBER -> "Number"
            ClipCategory.TEXT -> "Text"
        }
    }

    // ── Persistence ─────────────────────────────────────────────────────────

    private fun saveToPrefs() {
        val arr = JSONArray()
        for (item in items) {
            arr.put(JSONObject().apply {
                put("text", item.text)
                put("category", item.category.name)
                put("timestamp", item.timestamp)
                put("pinned", item.isPinned)
            })
        }
        prefs.edit().putString("clipboard_items", arr.toString()).apply()
    }

    private fun loadFromPrefs() {
        val json = prefs.getString("clipboard_items", null) ?: return
        try {
            val arr = JSONArray(json)
            items.clear()
            for (i in 0 until arr.length()) {
                val obj = arr.getJSONObject(i)
                items.add(ClipItem(
                    text = obj.getString("text"),
                    category = ClipCategory.valueOf(obj.getString("category")),
                    timestamp = obj.getLong("timestamp"),
                    isPinned = obj.optBoolean("pinned", false)
                ))
            }
        } catch (_: Exception) {
            // Corrupted data, start fresh
        }
    }
}
