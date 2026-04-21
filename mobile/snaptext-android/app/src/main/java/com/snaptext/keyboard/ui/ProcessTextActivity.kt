package com.snaptext.keyboard.ui

import android.app.Activity
import android.content.Intent
import android.graphics.Typeface
import android.os.Build
import android.os.Bundle
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import com.snaptext.keyboard.R
import com.snaptext.keyboard.ai.WorkerClient
import com.snaptext.keyboard.nlp.LanguageDetector
import com.snaptext.keyboard.nlp.PromptBuilder
import kotlinx.coroutines.*

/**
 * Core SnapText experience.
 *
 * User selects text anywhere → taps "SnapText" → bottom sheet appears:
 *   - Reply auto-generates immediately at the top
 *   - Below: three alternative actions (Fix English, Explain, AI Prompt)
 *   - Copy / Insert buttons
 */
class ProcessTextActivity : Activity() {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private var currentJob: Job? = null
    private var inputText = ""
    private var isReadOnly = false
    private var activeMode = "Reply"

    // Views we need to reference after build
    private lateinit var resultText: TextView
    private lateinit var actionRow: LinearLayout

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        inputText = intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)?.toString() ?: ""
        isReadOnly = intent.getBooleanExtra(Intent.EXTRA_PROCESS_TEXT_READONLY, false)

        if (inputText.isBlank()) { finish(); return }

        setupWindow()
        setContentView(buildUI())

        // Start generating reply immediately
        startTransform("Reply")
    }

    private fun setupWindow() {
        window.setLayout(
            WindowManager.LayoutParams.MATCH_PARENT,
            WindowManager.LayoutParams.WRAP_CONTENT
        )
        window.setGravity(Gravity.BOTTOM)
        window.addFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND)
        window.setDimAmount(0.4f)
    }

    // ── UI ───────────────────────────────────────────────────────────────────

    private fun buildUI(): View {
        val dp = resources.displayMetrics.density

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(resources.getColor(R.color.bg_secondary, null))
            setPadding((16 * dp).toInt(), (14 * dp).toInt(), (16 * dp).toInt(), (14 * dp).toInt())
        }

        // ── Header: SnapText + close ────────────────────────────────────────

        val header = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = lp(matchW(), wrapH()).apply { bottomMargin = (6 * dp).toInt() }
        }

        header.addView(TextView(this).apply {
            text = "\u2728 SnapText"
            textSize = 15f
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            setTextColor(resources.getColor(R.color.text_primary, null))
            layoutParams = LinearLayout.LayoutParams(0, wrapH(), 1f)
        })

        header.addView(TextView(this).apply {
            text = "\u2715"
            textSize = 18f
            setTextColor(resources.getColor(R.color.text_muted, null))
            setPadding((8 * dp).toInt(), (4 * dp).toInt(), (8 * dp).toInt(), (4 * dp).toInt())
            setOnClickListener { currentJob?.cancel(); finish() }
        })

        root.addView(header)

        // ── Result area (reply streams here) ────────────────────────────────

        val resultScroll = ScrollView(this).apply {
            layoutParams = lp(matchW(), (140 * dp).toInt())
        }

        resultText = TextView(this).apply {
            textSize = 15f
            setTextColor(resources.getColor(R.color.text_primary, null))
            text = "..."
            setLineSpacing(5 * dp, 1f)
        }

        resultScroll.addView(resultText)
        root.addView(resultScroll)

        // ── Other actions row ───────────────────────────────────────────────

        actionRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = lp(matchW(), wrapH()).apply {
                topMargin = (10 * dp).toInt()
                bottomMargin = (6 * dp).toInt()
            }
        }

        actionRow.addView(createActionChip("\u26A1 Do", "Do"))
        actionRow.addView(createActionChip("\u2705 Fix English", "Correct"))
        actionRow.addView(createActionChip("\u2728 AI Prompt", "Prompt"))

        root.addView(actionRow)

        // ── Bottom buttons: Copy / Insert ───────────────────────────────────

        val buttonBar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.END or Gravity.CENTER_VERTICAL
            layoutParams = lp(matchW(), (44 * dp).toInt()).apply { topMargin = (2 * dp).toInt() }
        }

        // Cancel
        buttonBar.addView(createButton("Cancel", R.color.text_muted, false) {
            currentJob?.cancel(); finish()
        })

        // Copy
        buttonBar.addView(createButton("Copy", R.color.accent_blue, false) {
            hapticTick()
            val result = resultText.text.toString()
            if (result.isNotBlank() && result != "...") {
                val cb = getSystemService(CLIPBOARD_SERVICE) as android.content.ClipboardManager
                cb.setPrimaryClip(android.content.ClipData.newPlainText("SnapText", result))
            }
            finish()
        })

        // Insert (only if editable)
        if (!isReadOnly) {
            buttonBar.addView(createButton("Insert", R.color.bg_primary, true) {
                hapticTick()
                val result = resultText.text.toString()
                if (result.isNotBlank() && result != "...") {
                    setResult(RESULT_OK, Intent().apply {
                        putExtra(Intent.EXTRA_PROCESS_TEXT, result)
                    })
                }
                finish()
            })
        }

        root.addView(buttonBar)
        return root
    }

    // ── Action chips (Fix English, Explain, AI Prompt) ──────────────────────

    private fun createActionChip(label: String, mode: String): TextView {
        val dp = resources.displayMetrics.density
        return TextView(this).apply {
            text = label
            textSize = 12f
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            setTextColor(resources.getColor(R.color.text_secondary, null))
            background = resources.getDrawable(R.drawable.bg_mode_chip, null)
            setPadding((12 * dp).toInt(), (7 * dp).toInt(), (12 * dp).toInt(), (7 * dp).toInt())
            layoutParams = LinearLayout.LayoutParams(0, wrapH(), 1f).apply {
                marginEnd = (6 * dp).toInt()
            }
            gravity = Gravity.CENTER
            isClickable = true
            isFocusable = true
            setOnClickListener {
                hapticTick()
                // Highlight active chip
                highlightActiveChip(mode)
                startTransform(mode)
            }
        }
    }

    private fun highlightActiveChip(activeMode: String) {
        val modes = listOf("Correct", "Knowledge", "Prompt")
        for (i in 0 until actionRow.childCount) {
            val chip = actionRow.getChildAt(i) as? TextView ?: continue
            val chipMode = modes.getOrNull(i) ?: continue
            val isActive = chipMode == activeMode
            chip.setTextColor(resources.getColor(
                if (isActive) R.color.text_primary else R.color.text_secondary, null
            ))
            chip.background = resources.getDrawable(
                if (isActive) R.drawable.bg_mode_chip_active else R.drawable.bg_mode_chip, null
            )
        }
        activeMode.let { this.activeMode = it }
    }

    // ── Bottom buttons ──────────────────────────────────────────────────────

    private fun createButton(label: String, colorRes: Int, filled: Boolean, action: () -> Unit): TextView {
        val dp = resources.displayMetrics.density
        return TextView(this).apply {
            text = label
            textSize = 13f
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            gravity = Gravity.CENTER
            setPadding((18 * dp).toInt(), (7 * dp).toInt(), (18 * dp).toInt(), (7 * dp).toInt())
            layoutParams = LinearLayout.LayoutParams(wrapH(), (34 * dp).toInt()).apply {
                marginStart = (6 * dp).toInt()
            }
            if (filled) {
                setTextColor(resources.getColor(colorRes, null))
                background = resources.getDrawable(R.drawable.bg_insert_button, null)
            } else {
                setTextColor(resources.getColor(colorRes, null))
            }
            setOnClickListener { action() }
        }
    }

    // ── AI Transform ────────────────────────────────────────────────────────

    private fun startTransform(mode: String) {
        currentJob?.cancel()
        activeMode = mode
        resultText.text = "..."

        val language = LanguageDetector.detect(inputText)
        val systemPrompt = PromptBuilder.buildSystemPrompt(mode, inputText, language)

        currentJob = scope.launch {
            val buf = StringBuilder()
            try {
                WorkerClient.generateStream(
                    context = this@ProcessTextActivity,
                    systemPrompt = systemPrompt,
                    userText = inputText,
                    onToken = { token ->
                        buf.append(token)
                        withContext(Dispatchers.Main) {
                            resultText.text = buf.toString()
                            val sv = resultText.parent as? ScrollView
                            sv?.post { sv.fullScroll(View.FOCUS_DOWN) }
                        }
                    },
                    onError = { error ->
                        withContext(Dispatchers.Main) { resultText.text = "Error: $error" }
                    },
                    onComplete = { _ ->
                        withContext(Dispatchers.Main) { hapticTick() }
                    }
                )
            } catch (_: CancellationException) {
            } catch (e: Exception) {
                withContext(Dispatchers.Main) { resultText.text = "Error: ${e.message}" }
            }
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    private fun matchW() = LinearLayout.LayoutParams.MATCH_PARENT
    private fun wrapH() = LinearLayout.LayoutParams.WRAP_CONTENT
    private fun lp(w: Int, h: Int) = LinearLayout.LayoutParams(w, h)

    private fun hapticTick() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val vm = getSystemService(VIBRATOR_MANAGER_SERVICE) as VibratorManager
            vm.defaultVibrator.vibrate(VibrationEffect.createPredefined(VibrationEffect.EFFECT_TICK))
        } else {
            @Suppress("DEPRECATION")
            val v = getSystemService(VIBRATOR_SERVICE) as Vibrator
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                v.vibrate(VibrationEffect.createOneShot(10, VibrationEffect.DEFAULT_AMPLITUDE))
            }
        }
    }

    override fun onDestroy() {
        currentJob?.cancel()
        scope.cancel()
        super.onDestroy()
    }
}
