package com.snaptext.keyboard.ai

import android.content.Context
import android.provider.Settings
import com.snaptext.keyboard.SnapTextApp
import com.snaptext.keyboard.data.Preferences
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import okhttp3.*
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.io.BufferedReader
import java.io.IOException
import java.io.InputStreamReader
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

object WorkerClient {

    private const val WORKER_URL = "https://snaptext-worker.snaptext-ai.workers.dev"
    private const val APP_SECRET = "snptxt_v1_8f3a2c7e9d1b4506"
    private const val GEMINI_STREAM_URL =
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:streamGenerateContent"
    private val JSON_MEDIA = "application/json; charset=utf-8".toMediaType()

    fun deviceId(context: Context): String {
        val androidId = Settings.Secure.getString(context.contentResolver, Settings.Secure.ANDROID_ID) ?: "unknown"
        // FNV-1a hash (same algorithm as desktop Rust version)
        var hash = 0xcbf29ce484222325UL
        for (byte in androidId.toByteArray()) {
            hash = hash xor byte.toULong()
            hash = hash * 0x100000000001b3UL
        }
        return hash.toString(16)
    }

    // ── Usage check ──────────────────────────────────────────────────────────

    suspend fun getUsage(context: Context): Pair<Int, Int>? = withContext(Dispatchers.IO) {
        try {
            val request = Request.Builder()
                .url("$WORKER_URL/usage")
                .header("X-Device-ID", deviceId(context))
                .header("X-App-Secret", APP_SECRET)
                .get()
                .build()

            val response = SnapTextApp.httpClient.newCall(request).await()
            if (!response.isSuccessful) return@withContext null

            val body = response.body?.string() ?: return@withContext null
            val json = JSONObject(body)
            Pair(json.optInt("used", 0), json.optInt("cap", 20))
        } catch (e: Exception) {
            null
        }
    }

    // ── Streaming generation (Worker mode) ───────────────────────────────────

    suspend fun generateStream(
        context: Context,
        systemPrompt: String,
        userText: String,
        onToken: suspend (String) -> Unit,
        onError: suspend (String) -> Unit,
        onComplete: suspend (String) -> Unit
    ) {
        val prefs = Preferences(context)
        val mode = prefs.aiMode
        val byokKey = prefs.byokKey

        when (mode) {
            "byok" -> {
                if (byokKey.isNullOrBlank()) {
                    onError("No Gemini API key configured")
                    return
                }
                generateDirectGemini(byokKey, systemPrompt, userText, onToken, onError, onComplete)
            }
            else -> {
                generateWorkerStream(context, systemPrompt, userText, onToken, onError, onComplete)
            }
        }
    }

    private suspend fun generateWorkerStream(
        context: Context,
        systemPrompt: String,
        userText: String,
        onToken: suspend (String) -> Unit,
        onError: suspend (String) -> Unit,
        onComplete: suspend (String) -> Unit
    ) = withContext(Dispatchers.IO) {
        try {
            val body = JSONObject().apply {
                put("system_prompt", systemPrompt)
                put("user_text", userText)
                put("stream", true)
                put("max_tokens", 2048)
                put("temperature", 0.7)
            }

            val request = Request.Builder()
                .url("$WORKER_URL/generate")
                .header("X-Device-ID", deviceId(context))
                .header("X-App-Secret", APP_SECRET)
                .post(body.toString().toRequestBody(JSON_MEDIA))
                .build()

            val response = SnapTextApp.httpClient.newCall(request).await()

            if (!response.isSuccessful) {
                val errBody = response.body?.string() ?: "Unknown error"
                onError("Error ${response.code}: $errBody")
                return@withContext
            }

            parseSSEStream(response, onToken, onError, onComplete)
        } catch (e: Exception) {
            onError("Network error: ${e.message}")
        }
    }

    private suspend fun generateDirectGemini(
        apiKey: String,
        systemPrompt: String,
        userText: String,
        onToken: suspend (String) -> Unit,
        onError: suspend (String) -> Unit,
        onComplete: suspend (String) -> Unit
    ) = withContext(Dispatchers.IO) {
        try {
            val geminiBody = JSONObject().apply {
                put("contents", org.json.JSONArray().put(
                    JSONObject().put("parts", org.json.JSONArray().put(
                        JSONObject().put("text", "$systemPrompt\n\n$userText")
                    ))
                ))
                put("generationConfig", JSONObject().apply {
                    put("temperature", 0.7)
                    put("maxOutputTokens", 2048)
                    put("thinkingConfig", JSONObject().put("thinkingBudget", 0))
                })
            }

            val url = "$GEMINI_STREAM_URL?key=$apiKey&alt=sse"
            val request = Request.Builder()
                .url(url)
                .post(geminiBody.toString().toRequestBody(JSON_MEDIA))
                .build()

            val response = SnapTextApp.httpClient.newCall(request).await()

            if (!response.isSuccessful) {
                onError("Gemini API error: ${response.code}")
                return@withContext
            }

            parseSSEStream(response, onToken, onError, onComplete)
        } catch (e: Exception) {
            onError("API error: ${e.message}")
        }
    }

    // ── SSE stream parser (shared by Worker and direct Gemini) ───────────────

    private suspend fun parseSSEStream(
        response: Response,
        onToken: suspend (String) -> Unit,
        onError: suspend (String) -> Unit,
        onComplete: suspend (String) -> Unit
    ) {
        val fullOutput = StringBuilder()

        response.use { resp ->
            resp.body?.byteStream()?.let { stream ->
                BufferedReader(InputStreamReader(stream)).use { reader ->
                    var line: String?
                    while (reader.readLine().also { line = it } != null) {
                        // Check for cancellation between lines
                        kotlinx.coroutines.currentCoroutineContext().ensureActive()

                        val l = line ?: continue
                        if (!l.startsWith("data: ")) continue
                        val jsonStr = l.removePrefix("data: ").trim()
                        if (jsonStr == "[DONE]") continue

                        try {
                            val json = JSONObject(jsonStr)
                            val candidates = json.optJSONArray("candidates") ?: continue
                            for (i in 0 until candidates.length()) {
                                val content = candidates.getJSONObject(i)
                                    .optJSONObject("content") ?: continue
                                val parts = content.optJSONArray("parts") ?: continue
                                for (j in 0 until parts.length()) {
                                    val text = parts.getJSONObject(j).optString("text", "")
                                    if (text.isNotEmpty()) {
                                        fullOutput.append(text)
                                        onToken(text)
                                    }
                                }
                            }
                        } catch (_: org.json.JSONException) {
                            // Skip malformed JSON chunks
                        }
                    }
                }
            }
        }

        if (fullOutput.isEmpty()) {
            onError("No response received")
        } else {
            onComplete(fullOutput.toString())
        }
    }

    // ── OkHttp coroutine extension ───────────────────────────────────────────

    private suspend fun Call.await(): Response = suspendCancellableCoroutine { cont ->
        cont.invokeOnCancellation { cancel() }
        enqueue(object : Callback {
            override fun onFailure(call: Call, e: IOException) {
                if (cont.isActive) cont.resumeWithException(e)
            }
            override fun onResponse(call: Call, response: Response) {
                if (cont.isActive) cont.resume(response)
            }
        })
    }
}
