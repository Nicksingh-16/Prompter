package com.snaptext.keyboard.ui

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.animation.*
import androidx.compose.foundation.background
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.snaptext.keyboard.ai.WorkerClient
import com.snaptext.keyboard.data.Preferences
import kotlinx.coroutines.launch

// ── Color palette ───────────────────────────────────────────────────────────

private val BgPrimary = Color(0xFF0A0A0F)
private val BgSecondary = Color(0xFF141420)
private val BgCard = Color(0xFF1A1A2E)
private val AccentBlue = Color(0xFF3B82F6)
private val AccentGreen = Color(0xFF10B981)
private val TextPrimary = Color(0xFFFFFFFF)
private val TextSecondary = Color(0xFF9CA3AF)
private val TextMuted = Color(0xFF6B7280)
private val Border = Color(0xFF2A2A3E)

class SettingsActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            val prefs = remember { Preferences(this) }
            if (!prefs.onboardingDone) {
                OnboardingScreen(onComplete = { prefs.onboardingDone = true })
            } else {
                SettingsScreen()
            }
        }
    }
}

// ── Onboarding (PROCESS_TEXT focused) ───────────────────────────────────────

@Composable
fun OnboardingScreen(onComplete: () -> Unit) {
    var step by remember { mutableIntStateOf(0) }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(BgPrimary)
            .padding(24.dp),
        contentAlignment = Alignment.Center
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(24.dp)
        ) {
            when (step) {
                0 -> {
                    Text("SnapText", fontSize = 36.sp, fontWeight = FontWeight.Bold, color = AccentBlue)
                    Text(
                        "Skip ChatGPT. Reply to clients in one tap.",
                        fontSize = 16.sp, color = TextSecondary, textAlign = TextAlign.Center
                    )
                    Spacer(Modifier.height(24.dp))

                    OnboardingStep(
                        icon = Icons.Default.TouchApp,
                        title = "Select any text",
                        desc = "WhatsApp message, email, anything"
                    )
                    OnboardingStep(
                        icon = Icons.Default.AutoAwesome,
                        title = "Tap a SnapText action",
                        desc = "Insert Reply, Fix English, Explain, or AI Prompt"
                    )
                    OnboardingStep(
                        icon = Icons.Default.Done,
                        title = "Done. One tap.",
                        desc = "Result appears instantly. Copy or insert."
                    )

                    Spacer(Modifier.height(16.dp))

                    Button(
                        onClick = { step = 1 },
                        colors = ButtonDefaults.buttonColors(containerColor = AccentBlue),
                        modifier = Modifier.fillMaxWidth().height(52.dp),
                        shape = RoundedCornerShape(12.dp)
                    ) {
                        Text("Get Started", fontSize = 16.sp)
                    }
                }

                1 -> {
                    Text("Try it now", fontSize = 24.sp, fontWeight = FontWeight.Bold, color = TextPrimary)
                    Spacer(Modifier.height(8.dp))

                    // Demo text the user can try selecting
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(12.dp))
                            .background(BgCard)
                            .border(1.dp, Border, RoundedCornerShape(12.dp))
                            .padding(16.dp)
                    ) {
                        Text("Try selecting this text:", fontSize = 12.sp, color = TextMuted)
                        Spacer(Modifier.height(8.dp))
                        SelectionContainer {
                            Text(
                                "bhai mujhe kal tak report chahiye, client bohot irritate ho raha hai. jaldi kar de please",
                                fontSize = 14.sp, color = TextPrimary
                            )
                        }
                        Spacer(Modifier.height(12.dp))
                        Text(
                            "Select the text above, then tap \"Insert Reply\" or \"Fix English\" from the menu",
                            fontSize = 12.sp, color = TextSecondary
                        )
                    }

                    Spacer(Modifier.height(24.dp))

                    Text(
                        "Works in every app: WhatsApp, Gmail, Chrome, Instagram, LinkedIn...",
                        fontSize = 13.sp, color = TextMuted, textAlign = TextAlign.Center
                    )

                    Spacer(Modifier.height(16.dp))

                    Button(
                        onClick = { onComplete() },
                        colors = ButtonDefaults.buttonColors(containerColor = AccentBlue),
                        modifier = Modifier.fillMaxWidth().height(52.dp),
                        shape = RoundedCornerShape(12.dp)
                    ) {
                        Text("I got it", fontSize = 16.sp)
                    }

                    TextButton(onClick = { onComplete() }) {
                        Text("Skip", color = TextMuted)
                    }
                }
            }
        }
    }
}

@Composable
fun OnboardingStep(icon: ImageVector, title: String, desc: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(BgCard)
            .border(1.dp, Border, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(icon, contentDescription = null, tint = AccentBlue, modifier = Modifier.size(28.dp))
        Spacer(Modifier.width(16.dp))
        Column {
            Text(title, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = TextPrimary)
            Text(desc, fontSize = 13.sp, color = TextSecondary)
        }
    }
}

// ── Settings ────────────────────────────────────────────────────────────────

@Composable
fun SettingsScreen() {
    val context = LocalContext.current
    val prefs = remember { Preferences(context) }
    var aiMode by remember { mutableStateOf(prefs.aiMode) }
    var byokKey by remember { mutableStateOf(prefs.byokKey ?: "") }
    var usage by remember { mutableStateOf<Pair<Int, Int>?>(null) }
    val coroutineScope = rememberCoroutineScope()

    LaunchedEffect(Unit) {
        usage = WorkerClient.getUsage(context)
    }

    Box(
        modifier = Modifier.fillMaxSize().background(BgPrimary)
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Header
            Text("SnapText", fontSize = 28.sp, fontWeight = FontWeight.Bold, color = AccentBlue)
            Text("AI writing assistant", fontSize = 14.sp, color = TextSecondary)

            Spacer(Modifier.height(4.dp))

            // How to use
            SettingsCard {
                Text("How to use", fontSize = 16.sp, fontWeight = FontWeight.SemiBold, color = TextPrimary)
                Spacer(Modifier.height(12.dp))

                HowToStep("1", "Select text in any app")
                Spacer(Modifier.height(6.dp))
                HowToStep("2", "Tap a SnapText option from the menu")
                Spacer(Modifier.height(6.dp))
                HowToStep("3", "Copy or insert the result")

                Spacer(Modifier.height(12.dp))

                Text("Available actions:", fontSize = 13.sp, color = TextMuted)
                Spacer(Modifier.height(6.dp))
                ActionLabel("\uD83D\uDCAC", "Insert Reply", "Compose a reply in the same language & tone")
                ActionLabel("\u2705", "Fix English", "Rewrite in clean, professional English")
                ActionLabel("\uD83D\uDCA1", "Explain", "Break down what the text means")
                ActionLabel("\u2728", "AI Prompt", "Turn text into a structured AI prompt")
            }

            // Usage card
            usage?.let { (used, cap) ->
                SettingsCard {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column {
                            Text("Today's Usage", fontSize = 14.sp, color = TextSecondary)
                            Text(
                                "$used / $cap transforms",
                                fontSize = 20.sp, fontWeight = FontWeight.Bold,
                                color = if (used >= cap) Color(0xFFEF4444) else AccentGreen
                            )
                        }
                        Icon(Icons.Default.TrendingUp, contentDescription = null, tint = AccentBlue, modifier = Modifier.size(32.dp))
                    }
                }
            }

            // AI Provider
            SettingsCard {
                Text("AI Provider", fontSize = 16.sp, fontWeight = FontWeight.SemiBold, color = TextPrimary)
                Spacer(Modifier.height(12.dp))

                AiModeOption(
                    label = "Cloud (Free, 20/day)",
                    description = "No setup needed. Uses SnapText servers.",
                    selected = aiMode == "worker",
                    onClick = { aiMode = "worker"; prefs.aiMode = "worker" }
                )

                Spacer(Modifier.height(8.dp))

                AiModeOption(
                    label = "Your Gemini API Key",
                    description = "Unlimited transforms. Bring your own key.",
                    selected = aiMode == "byok",
                    onClick = { aiMode = "byok"; prefs.aiMode = "byok" }
                )

                AnimatedVisibility(visible = aiMode == "byok") {
                    Column(modifier = Modifier.padding(top = 12.dp)) {
                        OutlinedTextField(
                            value = byokKey,
                            onValueChange = { byokKey = it; prefs.byokKey = it },
                            label = { Text("Gemini API Key", color = TextMuted) },
                            modifier = Modifier.fillMaxWidth(),
                            colors = OutlinedTextFieldDefaults.colors(
                                focusedBorderColor = AccentBlue, unfocusedBorderColor = Border,
                                focusedTextColor = TextPrimary, unfocusedTextColor = TextPrimary,
                                cursorColor = AccentBlue
                            ),
                            singleLine = true,
                            shape = RoundedCornerShape(8.dp)
                        )
                        Text(
                            "Get your key at aistudio.google.com",
                            fontSize = 12.sp, color = TextMuted, modifier = Modifier.padding(top = 4.dp)
                        )
                    }
                }
            }

            // About
            SettingsCard {
                Text("About", fontSize = 16.sp, fontWeight = FontWeight.SemiBold, color = TextPrimary)
                Spacer(Modifier.height(8.dp))
                Text("SnapText v2.0", fontSize = 13.sp, color = TextSecondary)
                Text("Skip ChatGPT. Reply in one tap.", fontSize = 13.sp, color = TextMuted)
            }

            Spacer(Modifier.height(32.dp))
        }
    }
}

// ── Reusable components ─────────────────────────────────────────────────────

@Composable
fun SettingsCard(content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(BgCard)
            .border(1.dp, Border, RoundedCornerShape(12.dp))
            .padding(16.dp),
        content = content
    )
}

@Composable
fun HowToStep(number: String, text: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(
            number, fontSize = 13.sp, fontWeight = FontWeight.Bold, color = AccentBlue,
            modifier = Modifier
                .size(24.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(AccentBlue.copy(alpha = 0.15f))
                .wrapContentSize(Alignment.Center)
        )
        Spacer(Modifier.width(12.dp))
        Text(text, fontSize = 14.sp, color = TextPrimary)
    }
}

@Composable
fun ActionLabel(icon: String, title: String, desc: String) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 3.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(icon, fontSize = 16.sp)
        Spacer(Modifier.width(10.dp))
        Column {
            Text(title, fontSize = 13.sp, fontWeight = FontWeight.Medium, color = TextPrimary)
            Text(desc, fontSize = 11.sp, color = TextMuted)
        }
    }
}

@Composable
fun AiModeOption(label: String, description: String, selected: Boolean, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(if (selected) AccentBlue.copy(alpha = 0.1f) else BgSecondary)
            .border(1.dp, if (selected) AccentBlue else Border, RoundedCornerShape(8.dp))
            .clickable(onClick = onClick)
            .padding(12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        RadioButton(
            selected = selected, onClick = onClick,
            colors = RadioButtonDefaults.colors(selectedColor = AccentBlue, unselectedColor = TextMuted)
        )
        Spacer(Modifier.width(8.dp))
        Column {
            Text(label, fontSize = 14.sp, fontWeight = FontWeight.Medium, color = TextPrimary)
            Text(description, fontSize = 12.sp, color = TextSecondary)
        }
    }
}
