package com.snaptext.keyboard.data

import android.content.Context
import android.content.SharedPreferences

class Preferences(context: Context) {

    private val prefs: SharedPreferences =
        context.getSharedPreferences("snaptext_prefs", Context.MODE_PRIVATE)

    var aiMode: String
        get() = prefs.getString("ai_mode", "worker") ?: "worker"
        set(value) = prefs.edit().putString("ai_mode", value).apply()

    var byokKey: String?
        get() = prefs.getString("byok_key", null)
        set(value) = prefs.edit().putString("byok_key", value).apply()

    var onboardingDone: Boolean
        get() = prefs.getBoolean("onboarding_done", false)
        set(value) = prefs.edit().putBoolean("onboarding_done", value).apply()

    var lastSelectedMode: String
        get() = prefs.getString("last_mode", "Correct") ?: "Correct"
        set(value) = prefs.edit().putString("last_mode", value).apply()

    // Tone Guard
    var toneGuardEnabled: Boolean
        get() = prefs.getBoolean("tone_guard_enabled", true)
        set(value) = prefs.edit().putBoolean("tone_guard_enabled", value).apply()

    // Translate mode
    var translateModeEnabled: Boolean
        get() = prefs.getBoolean("translate_mode_enabled", false)
        set(value) = prefs.edit().putBoolean("translate_mode_enabled", value).apply()

    var translateTargetLang: String
        get() = prefs.getString("translate_target_lang", "English") ?: "English"
        set(value) = prefs.edit().putString("translate_target_lang", value).apply()

    // Analytics
    var analyticsEnabled: Boolean
        get() = prefs.getBoolean("analytics_enabled", true)
        set(value) = prefs.edit().putBoolean("analytics_enabled", value).apply()

    // Smart Reply
    var smartReplyEnabled: Boolean
        get() = prefs.getBoolean("smart_reply_enabled", true)
        set(value) = prefs.edit().putBoolean("smart_reply_enabled", value).apply()
}
