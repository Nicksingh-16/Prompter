/**
 * Direct HTTP client for the Cloudflare Worker.
 * Used by quality tests to make real AI calls without the Tauri shell.
 *
 * Requires env vars:
 *   SNAPTEXT_APP_SECRET   — X-App-Secret header value
 *   SNAPTEXT_DEVICE_ID    — UUID for the test device (optional, defaults below)
 */

const WORKER_URL = 'https://snaptext-worker.snaptext-ai.workers.dev'
const TEST_DEVICE_ID = process.env.SNAPTEXT_DEVICE_ID ?? 'test-playwright-00000000-0000'

export function getSecret(): string {
  const s = process.env.SNAPTEXT_APP_SECRET
  if (!s) throw new Error('Set SNAPTEXT_APP_SECRET env var to run quality tests')
  return s
}

export interface WorkerResponse {
  text: string
  durationMs: number
  rawStatus: number
}

/** Call /generate (non-streaming) and return the assembled response text. */
export async function callWorker(
  mode: string,
  userText: string,
  systemPrompt: string,
  options: { maxTokens?: number; temperature?: number } = {}
): Promise<WorkerResponse> {
  const secret = getSecret()
  const t0 = Date.now()

  const body = {
    system_prompt: systemPrompt,
    user_text: userText,
    stream: false,
    max_tokens: options.maxTokens ?? 600,
    temperature: options.temperature ?? 0.3,
    thinking_budget: 0,
  }

  const res = await fetch(`${WORKER_URL}/generate`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-App-Secret': secret,
      'X-Device-ID': TEST_DEVICE_ID,
    },
    body: JSON.stringify(body),
  })

  const durationMs = Date.now() - t0
  const rawText = await res.text()

  if (!res.ok) {
    throw new Error(`Worker error ${res.status}: ${rawText}`)
  }

  // Parse non-streaming Gemini response
  let text = ''
  try {
    const json = JSON.parse(rawText)
    // Gemini non-streaming response structure
    text = json?.candidates?.[0]?.content?.parts?.[0]?.text ?? ''
    if (!text && json?.error) throw new Error(json.error)
  } catch {
    text = rawText
  }

  return { text: text.trim(), durationMs, rawStatus: res.status }
}

/** System prompts matching what the Rust backend sends for each mode. */
export const SYSTEM_PROMPTS: Record<string, string> = {
  Correct: 'Correct grammar, spelling, and punctuation in the following text. Return only the corrected text, no explanations.',
  Translate: 'Detect the language of the following text and translate it to English. Return only the translation.',
  Summarize: 'Summarize the following text in 2-3 concise bullet points. Be brief and factual.',
  Reply: 'Draft a professional and friendly reply to the following message. Be concise.',
  Do: 'Convert the following request into a clear, actionable task description.',
  Email: 'Rewrite the following as a professional email. Use proper greeting, body, and sign-off.',
  Casual: 'Rewrite the following text in a casual, conversational tone. Keep it friendly and natural.',
  Professional: 'Rewrite the following text in a professional, polished tone suitable for business communication.',
  Prompt: 'Complete the following request or task as instructed.',
}

/** Basic quality heuristics */
export const qualityChecks = {
  isNonEmpty: (text: string) => text.trim().length > 0,
  isEnglish: (text: string) => /[a-zA-Z]/.test(text) && !/[ऀ-ॿ؀-ۿ]/.test(text),
  isShorterThan: (text: string, ref: string) => text.length < ref.length,
  hasMinLength: (text: string, min: number) => text.length >= min,
  isPolite: (text: string) => /\b(thank|please|appreciate|hello|hi|dear)\b/i.test(text),
  noObviousGrammarErrors: (text: string) => !/ i /i.test(text),  // uncapitalized standalone "i"
}
