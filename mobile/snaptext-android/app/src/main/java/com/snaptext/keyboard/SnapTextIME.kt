package com.snaptext.keyboard

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Typeface
import android.inputmethodservice.InputMethodService
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import android.text.TextUtils
import android.view.Gravity
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import com.snaptext.keyboard.ai.WorkerClient
import com.snaptext.keyboard.clipboard.ClipboardHub
import com.snaptext.keyboard.data.AnalyticsTracker
import com.snaptext.keyboard.data.Preferences
import com.snaptext.keyboard.data.TemplateManager
import com.snaptext.keyboard.nlp.IntentDetector
import com.snaptext.keyboard.nlp.LanguageDetector
import com.snaptext.keyboard.nlp.PromptBuilder
import com.snaptext.keyboard.nlp.ToneAnalyzer
import kotlinx.coroutines.*

class SnapTextIME : InputMethodService() {

    private lateinit var keyboardView: View
    private lateinit var prefs: Preferences
    private lateinit var clipboardHub: ClipboardHub
    private lateinit var templateManager: TemplateManager
    private lateinit var analytics: AnalyticsTracker
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val handler = Handler(Looper.getMainLooper())

    // Keyboard state
    private var isShifted = false
    private var isCapsLock = false
    private var isSymbols = false
    private var isAiPanelVisible = false
    private var isClipboardPanelVisible = false
    private var isTemplatePanelVisible = false
    private var isListening = false
    private var selectedMode = "Correct"
    private var currentAiJob: Job? = null
    private var speechRecognizer: SpeechRecognizer? = null
    private var selectedTemplate: TemplateManager.Template? = null

    // Backspace repeat
    private var backspaceRepeating = false
    private val backspaceRepeatDelay = 50L   // ms between deletes while held
    private val backspaceInitialDelay = 400L // ms before repeat starts

    // Tone Guard
    private var toneCheckRunnable: Runnable? = null

    // ── Key layouts ─────────────────────────────────────────────────────────

    private val ROW1_KEYS = listOf("q", "w", "e", "r", "t", "y", "u", "i", "o", "p")
    private val ROW2_KEYS = listOf("a", "s", "d", "f", "g", "h", "j", "k", "l")
    private val ROW3_KEYS = listOf("z", "x", "c", "v", "b", "n", "m")
    private val SYMBOL_ROW1 = listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", "0")
    private val SYMBOL_ROW2 = listOf("@", "#", "$", "%", "&", "-", "+", "(", ")")
    private val SYMBOL_ROW3 = listOf("*", "\"", "'", ":", ";", "!", "?")

    // Only the essential modes users actually need. Others available via long-press.
    private val AI_MODES = listOf(
        AiMode("Reply", "\uD83D\uDCAC", R.color.mode_reply),
        AiMode("Correct", "\u2705", R.color.mode_correct),
        AiMode("Professional", "\uD83D\uDC54", R.color.mode_professional),
        AiMode("Casual", "\uD83D\uDE0A", R.color.mode_casual),
        AiMode("Email", "\u2709\uFE0F", R.color.mode_email),
        AiMode("Translate", "\uD83C\uDF10", R.color.mode_translate),
        AiMode("Expand", "\uD83D\uDCDD", R.color.mode_ghostwriter),
        AiMode("Summarize", "\uD83D\uDCCB", R.color.mode_summarize),
    )

    data class AiMode(val name: String, val icon: String, val colorRes: Int)

    // ── Lifecycle ────────────────────────────────────────────────────────────

    override fun onCreate() {
        super.onCreate()
        prefs = Preferences(this)
        clipboardHub = ClipboardHub(this)
        templateManager = TemplateManager(this)
        analytics = AnalyticsTracker(this)

        // Auto-collect clipboard entries
        val cm = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        cm.addPrimaryClipChangedListener {
            val clip = cm.primaryClip
            if (clip != null && clip.itemCount > 0) {
                val text = clip.getItemAt(0).text?.toString()
                if (!text.isNullOrBlank()) clipboardHub.addClip(text)
            }
        }
    }

    override fun onCreateInputView(): View {
        keyboardView = LayoutInflater.from(this).inflate(R.layout.keyboard_view, null)
        setupKeyboard()
        setupAiBar()
        setupAiResultPanel()
        setupClipboardPanel()
        setupTemplatePanel()
        setupToolsRow()
        return keyboardView
    }

    override fun onStartInputView(info: EditorInfo?, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        hideAllPanels()
        selectedMode = prefs.lastSelectedMode
    }

    override fun onDestroy() {
        stopVoiceInput()
        scope.cancel()
        handler.removeCallbacksAndMessages(null)
        super.onDestroy()
    }

    // ── Keyboard setup ───────────────────────────────────────────────────────

    private fun setupKeyboard() {
        val row1 = keyboardView.findViewById<LinearLayout>(R.id.row1)
        val row2 = keyboardView.findViewById<LinearLayout>(R.id.row2)
        val row3 = keyboardView.findViewById<LinearLayout>(R.id.row3)
        val row4 = keyboardView.findViewById<LinearLayout>(R.id.row4)

        populateLetterRow(row1, ROW1_KEYS, 1f)
        populateLetterRow(row2, ROW2_KEYS, 1f)
        populateRow3(row3)
        populateRow4(row4)
    }

    private fun populateLetterRow(row: LinearLayout, keys: List<String>, weight: Float) {
        row.removeAllViews()
        for (key in keys) row.addView(createKeyView(key, weight))
    }

    private fun populateRow3(row: LinearLayout) {
        row.removeAllViews()
        row.addView(createSpecialKeyView("\u21E7", 1.5f) { toggleShift() })
        val keys = if (isSymbols) SYMBOL_ROW3 else ROW3_KEYS
        for (key in keys) row.addView(createKeyView(key, 1f))
        // Backspace with long-press repeat
        row.addView(createBackspaceKey())
    }

    private fun populateRow4(row: LinearLayout) {
        row.removeAllViews()
        row.addView(createSpecialKeyView(if (isSymbols) "ABC" else "?123", 1.3f) { toggleSymbols() })
        row.addView(createKeyView(",", 0.7f))
        row.addView(createSpecialKeyView("space", 3.5f) { commitText(" ") })
        row.addView(createKeyView(".", 0.7f))
        // Mic button with visual feedback
        row.addView(createMicKey())
        // Enter — long press = "Say It Better"
        val enter = createSpecialKeyView("\u21B5", 1.3f) { handleEnter() }
        enter.setOnLongClickListener {
            hapticTick()
            triggerSayItBetter()
            true
        }
        row.addView(enter)
    }

    // ── Tools Row (below keyboard, minimal) ─────────────────────────────────

    private fun setupToolsRow() {
        val row = keyboardView.findViewById<LinearLayout>(R.id.row_tools) ?: return
        row.removeAllViews()

        val dp = resources.displayMetrics.density

        // Clipboard
        row.addView(createToolButton("\uD83D\uDCCB", "Clipboard") {
            if (isClipboardPanelVisible) hideClipboardPanel() else showClipboardPanel()
        })
        // Templates
        row.addView(createToolButton("\u26A1", "Templates") {
            if (isTemplatePanelVisible) hideTemplatePanel() else showTemplatePanel()
        })
    }

    private fun createToolButton(icon: String, label: String, action: () -> Unit): TextView {
        val dp = resources.displayMetrics.density
        return TextView(this).apply {
            text = "$icon $label"
            textSize = 11f
            setTextColor(resources.getColor(R.color.text_muted, null))
            gravity = Gravity.CENTER
            setPadding((16 * dp).toInt(), 0, (16 * dp).toInt(), 0)
            layoutParams = LinearLayout.LayoutParams(
                0, LinearLayout.LayoutParams.MATCH_PARENT, 1f
            )
            isClickable = true
            isFocusable = true
            setOnClickListener {
                hapticTick()
                action()
            }
        }
    }

    // ── Backspace with long-press repeat ────────────────────────────────────

    private fun createBackspaceKey(): TextView {
        return TextView(this).apply {
            text = "\u232B"
            textSize = 16f
            setTextColor(resources.getColor(R.color.text_secondary, null))
            gravity = Gravity.CENTER
            background = resources.getDrawable(R.drawable.bg_key_special, null)
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.MATCH_PARENT, 1.5f).apply {
                setMargins(3, 4, 3, 4)
            }
            isClickable = true
            isFocusable = true

            val repeatRunnable = object : Runnable {
                override fun run() {
                    if (backspaceRepeating) {
                        handleBackspace()
                        handler.postDelayed(this, backspaceRepeatDelay)
                    }
                }
            }

            setOnTouchListener { _, event ->
                when (event.action) {
                    MotionEvent.ACTION_DOWN -> {
                        hapticTick()
                        handleBackspace()
                        backspaceRepeating = true
                        handler.postDelayed(repeatRunnable, backspaceInitialDelay)
                        true
                    }
                    MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                        backspaceRepeating = false
                        handler.removeCallbacks(repeatRunnable)
                        true
                    }
                    else -> false
                }
            }
        }
    }

    // ── Mic button with visual state ────────────────────────────────────────

    private fun createMicKey(): TextView {
        return TextView(this).apply {
            text = "\uD83C\uDFA4"
            textSize = 16f
            tag = "mic_key"
            setTextColor(resources.getColor(R.color.text_secondary, null))
            gravity = Gravity.CENTER
            background = resources.getDrawable(R.drawable.bg_key_special, null)
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.MATCH_PARENT, 1f).apply {
                setMargins(3, 4, 3, 4)
            }
            isClickable = true
            isFocusable = true
            setOnClickListener {
                hapticTick()
                toggleVoiceInput()
            }
        }
    }

    private fun updateMicButtonState() {
        val micBtn = keyboardView.findViewWithTag<TextView>("mic_key") ?: return
        if (isListening) {
            micBtn.text = "\uD83D\uDD34"  // Red circle = recording
            micBtn.setBackgroundColor(resources.getColor(R.color.accent_red, null))
        } else {
            micBtn.text = "\uD83C\uDFA4"
            micBtn.background = resources.getDrawable(R.drawable.bg_key_special, null)
        }
    }

    // ── Key views ────────────────────────────────────────────────────────────

    private fun createKeyView(label: String, weight: Float): TextView {
        return TextView(this).apply {
            text = if (isShifted || isCapsLock) label.uppercase() else label
            textSize = 20f
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            setTextColor(resources.getColor(R.color.text_key, null))
            gravity = Gravity.CENTER
            background = resources.getDrawable(R.drawable.bg_key, null)
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.MATCH_PARENT, weight).apply {
                setMargins(3, 4, 3, 4)
            }
            isClickable = true
            isFocusable = true
            setOnClickListener {
                hapticTick()
                val char = if (isShifted || isCapsLock) label.uppercase() else label
                commitText(char)
                if (isShifted && !isCapsLock) {
                    isShifted = false
                    refreshKeyboard()
                }
                scheduleToneCheck()
            }
        }
    }

    private fun createSpecialKeyView(label: String, weight: Float, action: () -> Unit): TextView {
        val isSpace = label == "space"
        return TextView(this).apply {
            text = if (isSpace) "" else label
            textSize = when {
                isSpace -> 12f
                label.length > 2 -> 13f
                else -> 16f
            }
            setTextColor(resources.getColor(
                if (isSpace) R.color.text_muted else R.color.text_secondary, null
            ))
            gravity = Gravity.CENTER
            background = resources.getDrawable(
                if (isSpace) R.drawable.bg_space_bar else R.drawable.bg_key_special, null
            )
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.MATCH_PARENT, weight).apply {
                setMargins(3, 4, 3, 4)
            }
            isClickable = true
            isFocusable = true
            setOnClickListener {
                hapticTick()
                action()
            }
        }
    }

    private fun refreshKeyboard() {
        val row1 = keyboardView.findViewById<LinearLayout>(R.id.row1)
        val row2 = keyboardView.findViewById<LinearLayout>(R.id.row2)
        val row3 = keyboardView.findViewById<LinearLayout>(R.id.row3)
        val row4 = keyboardView.findViewById<LinearLayout>(R.id.row4)

        if (isSymbols) {
            populateLetterRow(row1, SYMBOL_ROW1, 1f)
            populateLetterRow(row2, SYMBOL_ROW2, 1f)
        } else {
            populateLetterRow(row1, ROW1_KEYS, 1f)
            populateLetterRow(row2, ROW2_KEYS, 1f)
        }
        populateRow3(row3)
        populateRow4(row4)
    }

    // ── Key actions ──────────────────────────────────────────────────────────

    private fun commitText(text: String) {
        currentInputConnection?.commitText(text, 1)
    }

    private fun handleBackspace() {
        val ic = currentInputConnection ?: return
        val selected = ic.getSelectedText(0)
        if (selected != null && selected.isNotEmpty()) {
            ic.commitText("", 1)
        } else {
            ic.deleteSurroundingText(1, 0)
        }
    }

    private fun handleEnter() {
        val ic = currentInputConnection ?: return
        ic.performEditorAction(currentInputEditorInfo?.imeOptions ?: EditorInfo.IME_ACTION_DONE)
    }

    private fun toggleShift() {
        if (isShifted) {
            isCapsLock = true
        } else {
            isShifted = !isShifted
            isCapsLock = false
        }
        refreshKeyboard()
    }

    private fun toggleSymbols() {
        isSymbols = !isSymbols
        isShifted = false
        isCapsLock = false
        refreshKeyboard()
    }

    // ── Voice Input ─────────────────────────────────────────────────────────

    private fun toggleVoiceInput() {
        if (isListening) {
            stopVoiceInput()
            return
        }

        if (!SpeechRecognizer.isRecognitionAvailable(this)) return

        isListening = true
        updateMicButtonState()

        // Show feedback in suggestion bar
        val suggestion = keyboardView.findViewById<TextView>(R.id.suggestion_text)
        suggestion?.text = "\uD83C\uDFA4 Listening..."
        suggestion?.setTextColor(resources.getColor(R.color.accent_red, null))

        speechRecognizer = SpeechRecognizer.createSpeechRecognizer(this)
        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
            putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
            putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1)
        }

        speechRecognizer?.setRecognitionListener(object : RecognitionListener {
            override fun onReadyForSpeech(params: Bundle?) {}
            override fun onBeginningOfSpeech() {}
            override fun onRmsChanged(rmsdB: Float) {}
            override fun onBufferReceived(buffer: ByteArray?) {}
            override fun onEndOfSpeech() {
                isListening = false
                updateMicButtonState()
                resetSuggestionText()
            }
            override fun onError(error: Int) {
                isListening = false
                updateMicButtonState()
                val suggestion = keyboardView.findViewById<TextView>(R.id.suggestion_text)
                when (error) {
                    SpeechRecognizer.ERROR_NO_MATCH -> suggestion?.text = "Didn't catch that. Try again."
                    SpeechRecognizer.ERROR_AUDIO -> suggestion?.text = "Mic unavailable"
                    SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> suggestion?.text = "Mic permission needed"
                    else -> suggestion?.text = "Voice error. Try again."
                }
                suggestion?.setTextColor(resources.getColor(R.color.text_muted, null))
                handler.postDelayed({ resetSuggestionText() }, 2000)
            }
            override fun onResults(results: Bundle?) {
                isListening = false
                updateMicButtonState()
                resetSuggestionText()
                val matches = results?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                if (!matches.isNullOrEmpty()) {
                    commitText(matches[0])
                }
            }
            override fun onPartialResults(partialResults: Bundle?) {
                val partial = partialResults?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                if (!partial.isNullOrEmpty()) {
                    val suggestion = keyboardView.findViewById<TextView>(R.id.suggestion_text)
                    suggestion?.text = "\uD83C\uDFA4 ${partial[0]}"
                }
            }
            override fun onEvent(eventType: Int, params: Bundle?) {}
        })

        speechRecognizer?.startListening(intent)
    }

    private fun stopVoiceInput() {
        isListening = false
        speechRecognizer?.stopListening()
        speechRecognizer?.destroy()
        speechRecognizer = null
        updateMicButtonState()
        resetSuggestionText()
    }

    private fun resetSuggestionText() {
        val suggestion = keyboardView.findViewById<TextView>(R.id.suggestion_text) ?: return
        suggestion.text = "Tap \u2728 to transform with AI"
        suggestion.setTextColor(resources.getColor(R.color.text_muted, null))
    }

    // ── AI Bar ───────────────────────────────────────────────────────────────

    private fun setupAiBar() {
        val aiTrigger = keyboardView.findViewById<TextView>(R.id.btn_ai_trigger)
        val modeScroll = keyboardView.findViewById<HorizontalScrollView>(R.id.mode_scroll)
        val modeContainer = keyboardView.findViewById<LinearLayout>(R.id.mode_chips_container)
        val suggestionText = keyboardView.findViewById<TextView>(R.id.suggestion_text)

        aiTrigger.setOnClickListener {
            hapticTick()

            // Toggle mode strip
            if (modeScroll.visibility == View.VISIBLE) {
                modeScroll.visibility = View.GONE
                suggestionText.visibility = View.VISIBLE
                return@setOnClickListener
            }

            suggestionText.visibility = View.GONE
            modeScroll.visibility = View.VISIBLE

            // Auto-detect best mode from input
            val inputText = getInputFieldText()
            if (inputText.isNotBlank()) {
                val lang = LanguageDetector.detect(inputText)
                val intent = IntentDetector.detect(inputText, lang)
                selectedMode = intent.suggestedMode
            }

            // Populate mode chips
            modeContainer.removeAllViews()
            for (mode in AI_MODES) {
                modeContainer.addView(createModeChip(mode))
            }
        }
    }

    private fun createModeChip(mode: AiMode): TextView {
        val isActive = mode.name == selectedMode
        return TextView(this).apply {
            text = "${mode.icon} ${mode.name}"
            textSize = 13f
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            setTextColor(
                if (isActive) resources.getColor(R.color.text_primary, null)
                else resources.getColor(R.color.text_secondary, null)
            )
            background = resources.getDrawable(
                if (isActive) R.drawable.bg_mode_chip_active else R.drawable.bg_mode_chip, null
            )
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins(4, 4, 4, 4) }
            isClickable = true
            isFocusable = true
            setOnClickListener {
                hapticTick()
                selectedMode = mode.name
                prefs.lastSelectedMode = mode.name
                val promptMode = when (mode.name) {
                    "Expand" -> "GhostWriter"
                    else -> mode.name
                }
                triggerAiTransform(promptMode)
            }
        }
    }

    // ── AI Transform ─────────────────────────────────────────────────────────

    private fun triggerAiTransform(mode: String = selectedMode) {
        val inputText = getInputFieldText()
        if (inputText.isBlank()) return

        showAiPanel()

        val resultText = keyboardView.findViewById<TextView>(R.id.ai_result_text)
        resultText.text = "Thinking..."

        val language = LanguageDetector.detect(inputText)
        val systemPrompt = PromptBuilder.buildSystemPrompt(mode, inputText, language)
        val toneResult = ToneAnalyzer.analyze(inputText)

        currentAiJob?.cancel()
        currentAiJob = scope.launch {
            val fullResult = StringBuilder()
            try {
                WorkerClient.generateStream(
                    context = this@SnapTextIME,
                    systemPrompt = systemPrompt,
                    userText = inputText,
                    onToken = { token ->
                        fullResult.append(token)
                        withContext(Dispatchers.Main) {
                            resultText.text = fullResult.toString()
                            val scrollView = resultText.parent as? ScrollView
                            scrollView?.post { scrollView.fullScroll(View.FOCUS_DOWN) }
                        }
                    },
                    onError = { error ->
                        withContext(Dispatchers.Main) {
                            resultText.text = "Error: $error"
                        }
                    },
                    onComplete = { _ ->
                        withContext(Dispatchers.Main) {
                            hapticTick()
                            if (prefs.analyticsEnabled) {
                                analytics.recordTransform(mode, toneResult.score)
                            }
                        }
                    }
                )
            } catch (_: CancellationException) {
            } catch (e: Exception) {
                withContext(Dispatchers.Main) {
                    resultText.text = "Error: ${e.message}"
                }
            }
        }
    }

    // ── Say It Better (long-press enter) ────────────────────────────────────

    private fun triggerSayItBetter() {
        val inputText = getInputFieldText()
        if (inputText.isBlank()) return
        selectedMode = "SayItBetter"
        triggerAiTransform("SayItBetter")
    }

    // ── Tone Guard ──────────────────────────────────────────────────────────

    private fun scheduleToneCheck() {
        if (!prefs.toneGuardEnabled) return
        toneCheckRunnable?.let { handler.removeCallbacks(it) }
        toneCheckRunnable = Runnable { performToneCheck() }
        handler.postDelayed(toneCheckRunnable!!, 600L)
    }

    private fun performToneCheck() {
        val inputText = getInputFieldText()
        if (inputText.length < 15) {
            hideToneGuard()
            return
        }

        val result = ToneAnalyzer.analyze(inputText)
        val strip = keyboardView.findViewById<View>(R.id.tone_guard_strip)
        val emoji = keyboardView.findViewById<TextView>(R.id.tone_emoji)
        val suggestion = keyboardView.findViewById<TextView>(R.id.suggestion_text)
        val modeScroll = keyboardView.findViewById<HorizontalScrollView>(R.id.mode_scroll)

        when (result.level) {
            ToneAnalyzer.ToneLevel.POSITIVE -> {
                strip?.setBackgroundColor(resources.getColor(R.color.tone_positive, null))
                strip?.visibility = View.VISIBLE
                emoji?.text = "\uD83D\uDE0A"
                emoji?.visibility = View.VISIBLE
            }
            ToneAnalyzer.ToneLevel.CAUTION -> {
                strip?.setBackgroundColor(resources.getColor(R.color.tone_caution, null))
                strip?.visibility = View.VISIBLE
                emoji?.text = "\u26A0\uFE0F"
                emoji?.visibility = View.VISIBLE
                if (result.suggestion != null && modeScroll?.visibility != View.VISIBLE) {
                    suggestion?.text = "\u26A0\uFE0F ${result.suggestion}"
                    suggestion?.setTextColor(resources.getColor(R.color.tone_caution, null))
                }
            }
            ToneAnalyzer.ToneLevel.HARSH -> {
                strip?.setBackgroundColor(resources.getColor(R.color.tone_harsh, null))
                strip?.visibility = View.VISIBLE
                emoji?.text = "\uD83D\uDED1"
                emoji?.visibility = View.VISIBLE
                if (result.suggestion != null && modeScroll?.visibility != View.VISIBLE) {
                    suggestion?.text = "\uD83D\uDED1 ${result.suggestion}"
                    suggestion?.setTextColor(resources.getColor(R.color.tone_harsh, null))
                }
            }
            ToneAnalyzer.ToneLevel.NEUTRAL -> hideToneGuard()
        }
    }

    private fun hideToneGuard() {
        keyboardView.findViewById<View>(R.id.tone_guard_strip)?.visibility = View.GONE
        keyboardView.findViewById<TextView>(R.id.tone_emoji)?.visibility = View.GONE
    }

    // ── Clipboard Hub ───────────────────────────────────────────────────────

    private fun setupClipboardPanel() {
        keyboardView.findViewById<TextView>(R.id.btn_clip_close)?.setOnClickListener {
            hapticTick(); hideClipboardPanel()
        }
        keyboardView.findViewById<TextView>(R.id.btn_clip_clear)?.setOnClickListener {
            hapticTick(); clipboardHub.clearNonPinned(); refreshClipboardItems()
        }
    }

    private fun showClipboardPanel() {
        hideAllPanels()
        isClipboardPanelVisible = true
        keyboardView.findViewById<LinearLayout>(R.id.clipboard_panel)?.visibility = View.VISIBLE
        keyboardView.findViewById<LinearLayout>(R.id.keyboard_rows)?.visibility = View.GONE
        refreshClipboardCategories()
        refreshClipboardItems()
    }

    private fun hideClipboardPanel() {
        isClipboardPanelVisible = false
        keyboardView.findViewById<LinearLayout>(R.id.clipboard_panel)?.visibility = View.GONE
        keyboardView.findViewById<LinearLayout>(R.id.keyboard_rows)?.visibility = View.VISIBLE
    }

    private fun refreshClipboardCategories(filter: ClipboardHub.ClipCategory? = null) {
        val container = keyboardView.findViewById<LinearLayout>(R.id.clip_category_chips) ?: return
        container.removeAllViews()
        container.addView(createFilterChip("All", filter == null) {
            refreshClipboardCategories(null); refreshClipboardItems(null)
        })
        for (cat in clipboardHub.getItems().map { it.category }.distinct()) {
            val label = "${ClipboardHub.categoryIcon(cat)} ${ClipboardHub.categoryLabel(cat)}"
            container.addView(createFilterChip(label, cat == filter) {
                refreshClipboardCategories(cat); refreshClipboardItems(cat)
            })
        }
    }

    private fun refreshClipboardItems(filter: ClipboardHub.ClipCategory? = null) {
        val container = keyboardView.findViewById<LinearLayout>(R.id.clip_items_container) ?: return
        container.removeAllViews()
        val items = if (filter != null) clipboardHub.getByCategory(filter) else clipboardHub.getItems()
        if (items.isEmpty()) {
            container.addView(TextView(this).apply {
                text = "No clipboard items yet"; textSize = 13f
                setTextColor(resources.getColor(R.color.text_muted, null))
                gravity = Gravity.CENTER; setPadding(0, 40, 0, 40)
            })
            return
        }
        for (item in items) container.addView(createClipItemView(item))
    }

    private fun createClipItemView(item: ClipboardHub.ClipItem): LinearLayout {
        val dp = resources.displayMetrics.density
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding((10 * dp).toInt(), (8 * dp).toInt(), (10 * dp).toInt(), (8 * dp).toInt())
            setBackgroundColor(resources.getColor(R.color.clip_item_bg, null))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { bottomMargin = (2 * dp).toInt() }

            // Icon
            addView(TextView(this@SnapTextIME).apply {
                text = ClipboardHub.categoryIcon(item.category); textSize = 14f
                setPadding(0, 0, (8 * dp).toInt(), 0)
            })
            // Text — tap to paste
            addView(TextView(this@SnapTextIME).apply {
                text = item.text.take(50).replace("\n", " "); textSize = 13f
                setTextColor(resources.getColor(R.color.text_primary, null))
                maxLines = 1; ellipsize = TextUtils.TruncateAt.END
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                isClickable = true
                setOnClickListener {
                    hapticTick()
                    currentInputConnection?.commitText(item.text, 1)
                    hideClipboardPanel()
                }
            })
            // Pin
            addView(TextView(this@SnapTextIME).apply {
                text = if (item.isPinned) "\uD83D\uDCCC" else "\u2022"; textSize = 14f
                setPadding((8 * dp).toInt(), 0, (4 * dp).toInt(), 0)
                isClickable = true
                setOnClickListener {
                    hapticTick(); clipboardHub.togglePin(item.text); refreshClipboardItems()
                }
            })
        }
    }

    // ── Template Panel ──────────────────────────────────────────────────────

    private fun setupTemplatePanel() {
        keyboardView.findViewById<TextView>(R.id.btn_tmpl_close)?.setOnClickListener {
            hapticTick(); hideTemplatePanel()
        }
        keyboardView.findViewById<TextView>(R.id.btn_tmpl_insert)?.setOnClickListener {
            hapticTick(); insertFilledTemplate()
        }
    }

    private fun showTemplatePanel() {
        hideAllPanels()
        isTemplatePanelVisible = true
        selectedTemplate = null
        keyboardView.findViewById<LinearLayout>(R.id.template_panel)?.visibility = View.VISIBLE
        keyboardView.findViewById<LinearLayout>(R.id.keyboard_rows)?.visibility = View.GONE
        keyboardView.findViewById<LinearLayout>(R.id.tmpl_fill_area)?.visibility = View.GONE
        keyboardView.findViewById<TextView>(R.id.btn_tmpl_insert)?.visibility = View.GONE
        refreshTemplateCategories()
        refreshTemplateItems()
    }

    private fun hideTemplatePanel() {
        isTemplatePanelVisible = false
        selectedTemplate = null
        keyboardView.findViewById<LinearLayout>(R.id.template_panel)?.visibility = View.GONE
        keyboardView.findViewById<LinearLayout>(R.id.keyboard_rows)?.visibility = View.VISIBLE
    }

    private var selectedTemplateCategory: String? = null

    private fun refreshTemplateCategories() {
        val container = keyboardView.findViewById<LinearLayout>(R.id.tmpl_category_chips) ?: return
        container.removeAllViews()
        container.addView(createFilterChip("All", selectedTemplateCategory == null) {
            selectedTemplateCategory = null; refreshTemplateCategories(); refreshTemplateItems()
        })
        for (cat in templateManager.getCategories()) {
            container.addView(createFilterChip(cat, cat == selectedTemplateCategory) {
                selectedTemplateCategory = cat; refreshTemplateCategories(); refreshTemplateItems()
            })
        }
    }

    private fun refreshTemplateItems() {
        val container = keyboardView.findViewById<LinearLayout>(R.id.tmpl_items_container) ?: return
        container.removeAllViews()
        val templates = if (selectedTemplateCategory != null)
            templateManager.getByCategory(selectedTemplateCategory!!)
        else templateManager.getAll()
        for (tmpl in templates) container.addView(createTemplateItemView(tmpl))
    }

    private fun createTemplateItemView(tmpl: TemplateManager.Template): LinearLayout {
        val dp = resources.displayMetrics.density
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding((10 * dp).toInt(), (8 * dp).toInt(), (10 * dp).toInt(), (8 * dp).toInt())
            setBackgroundColor(resources.getColor(R.color.clip_item_bg, null))
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { bottomMargin = (2 * dp).toInt() }

            addView(TextView(this@SnapTextIME).apply {
                text = tmpl.name; textSize = 14f
                typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
                setTextColor(resources.getColor(R.color.text_primary, null))
            })
            addView(TextView(this@SnapTextIME).apply {
                text = tmpl.body.take(70).replace("\n", " "); textSize = 12f
                setTextColor(resources.getColor(R.color.text_muted, null))
                maxLines = 1; ellipsize = TextUtils.TruncateAt.END
            })
            isClickable = true; isFocusable = true
            setOnClickListener { hapticTick(); selectTemplate(tmpl) }
        }
    }

    private fun selectTemplate(tmpl: TemplateManager.Template) {
        selectedTemplate = tmpl
        val placeholders = tmpl.getPlaceholders()
        if (placeholders.isEmpty()) {
            currentInputConnection?.commitText(tmpl.body, 1)
            templateManager.incrementUsage(tmpl.id)
            hideTemplatePanel()
            return
        }

        val fillArea = keyboardView.findViewById<LinearLayout>(R.id.tmpl_fill_area) ?: return
        fillArea.visibility = View.VISIBLE
        keyboardView.findViewById<TextView>(R.id.btn_tmpl_insert)?.visibility = View.VISIBLE
        keyboardView.findViewById<TextView>(R.id.tmpl_preview)?.text = tmpl.body

        val fields = keyboardView.findViewById<LinearLayout>(R.id.tmpl_fields_container) ?: return
        fields.removeAllViews()
        val dp = resources.displayMetrics.density
        for (ph in placeholders) {
            fields.addView(EditText(this).apply {
                hint = ph.replaceFirstChar { it.uppercase() }.replace("_", " ")
                textSize = 14f
                setTextColor(resources.getColor(R.color.text_primary, null))
                setHintTextColor(resources.getColor(R.color.text_muted, null))
                setBackgroundColor(resources.getColor(R.color.tmpl_field_bg, null))
                setPadding((12 * dp).toInt(), (8 * dp).toInt(), (12 * dp).toInt(), (8 * dp).toInt())
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply { bottomMargin = (4 * dp).toInt() }
                isSingleLine = true; tag = ph
            })
        }
    }

    private fun insertFilledTemplate() {
        val tmpl = selectedTemplate ?: return
        val fields = keyboardView.findViewById<LinearLayout>(R.id.tmpl_fields_container) ?: return
        val values = mutableMapOf<String, String>()
        for (i in 0 until fields.childCount) {
            val f = fields.getChildAt(i) as? EditText ?: continue
            val k = f.tag as? String ?: continue
            values[k] = f.text.toString().ifBlank { "{$k}" }
        }
        currentInputConnection?.commitText(tmpl.fill(values), 1)
        templateManager.incrementUsage(tmpl.id)
        hideTemplatePanel()
    }

    // ── Filter chip (shared by clipboard & templates) ───────────────────────

    private fun createFilterChip(label: String, active: Boolean, onClick: () -> Unit): TextView {
        val dp = resources.displayMetrics.density
        return TextView(this).apply {
            text = label; textSize = 11f
            setTextColor(resources.getColor(
                if (active) R.color.text_primary else R.color.text_muted, null
            ))
            background = resources.getDrawable(
                if (active) R.drawable.bg_mode_chip_active else R.drawable.bg_mode_chip, null
            )
            setPadding((10 * dp).toInt(), (4 * dp).toInt(), (10 * dp).toInt(), (4 * dp).toInt())
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply { setMargins((3 * dp).toInt(), 0, (3 * dp).toInt(), 0) }
            isClickable = true; isFocusable = true
            setOnClickListener { hapticTick(); onClick() }
        }
    }

    // ── AI Result Panel ──────────────────────────────────────────────────────

    private fun setupAiResultPanel() {
        val btnInsert = keyboardView.findViewById<TextView>(R.id.btn_insert)
        val btnCopy = keyboardView.findViewById<TextView>(R.id.btn_copy)
        val btnCancel = keyboardView.findViewById<TextView>(R.id.btn_cancel)
        val resultText = keyboardView.findViewById<TextView>(R.id.ai_result_text)

        btnInsert.setOnClickListener {
            hapticTick()
            val result = resultText.text.toString()
            if (result.isNotBlank() && result != "Thinking...") {
                val ic = currentInputConnection
                if (ic != null) {
                    ic.beginBatchEdit()
                    val before = ic.getTextBeforeCursor(10000, 0)?.length ?: 0
                    val after = ic.getTextAfterCursor(10000, 0)?.length ?: 0
                    ic.setSelection(0, before + after)
                    ic.commitText(result, 1)
                    ic.endBatchEdit()
                }
            }
            hideAiPanel()
        }

        btnCopy.setOnClickListener {
            hapticTick()
            val result = resultText.text.toString()
            if (result.isNotBlank() && result != "Thinking...") {
                val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                clipboard.setPrimaryClip(ClipData.newPlainText("SnapText", result))
            }
            hideAiPanel()
        }

        btnCancel.setOnClickListener {
            hapticTick()
            currentAiJob?.cancel()
            hideAiPanel()
        }
    }

    private fun showAiPanel() {
        hideAllPanels()
        isAiPanelVisible = true
        keyboardView.findViewById<LinearLayout>(R.id.ai_result_panel).visibility = View.VISIBLE
        keyboardView.findViewById<LinearLayout>(R.id.keyboard_rows).visibility = View.GONE
    }

    private fun hideAiPanel() {
        isAiPanelVisible = false
        currentAiJob?.cancel()
        keyboardView.findViewById<LinearLayout>(R.id.ai_result_panel).visibility = View.GONE
        keyboardView.findViewById<LinearLayout>(R.id.keyboard_rows).visibility = View.VISIBLE
        keyboardView.findViewById<HorizontalScrollView>(R.id.mode_scroll).visibility = View.GONE
        keyboardView.findViewById<TextView>(R.id.suggestion_text).visibility = View.VISIBLE
        resetSuggestionText()
    }

    private fun hideAllPanels() {
        isAiPanelVisible = false
        isClipboardPanelVisible = false
        isTemplatePanelVisible = false
        currentAiJob?.cancel()
        keyboardView.findViewById<LinearLayout>(R.id.ai_result_panel)?.visibility = View.GONE
        keyboardView.findViewById<LinearLayout>(R.id.clipboard_panel)?.visibility = View.GONE
        keyboardView.findViewById<LinearLayout>(R.id.template_panel)?.visibility = View.GONE
        keyboardView.findViewById<LinearLayout>(R.id.keyboard_rows)?.visibility = View.VISIBLE
        keyboardView.findViewById<HorizontalScrollView>(R.id.mode_scroll)?.visibility = View.GONE
        keyboardView.findViewById<TextView>(R.id.suggestion_text)?.visibility = View.VISIBLE
    }

    private fun getInputFieldText(): String {
        val ic = currentInputConnection ?: return ""
        val before = ic.getTextBeforeCursor(5000, 0) ?: ""
        val after = ic.getTextAfterCursor(5000, 0) ?: ""
        val selected = ic.getSelectedText(0) ?: ""
        if (selected.isNotEmpty()) return selected.toString()
        return "$before$after".trim()
    }

    // ── Haptics ──────────────────────────────────────────────────────────────

    private fun hapticTick() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val vm = getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as VibratorManager
            vm.defaultVibrator.vibrate(VibrationEffect.createPredefined(VibrationEffect.EFFECT_TICK))
        } else {
            @Suppress("DEPRECATION")
            val vibrator = getSystemService(Context.VIBRATOR_SERVICE) as Vibrator
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                vibrator.vibrate(VibrationEffect.createOneShot(10, VibrationEffect.DEFAULT_AMPLITUDE))
            }
        }
    }
}
