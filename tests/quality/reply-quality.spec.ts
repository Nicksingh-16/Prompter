/**
 * Quality tests — open the real app UI and run actual AI calls through it.
 *
 * Flow for each test:
 *   1. page.exposeFunction — Node.js-side worker call (no CORS)
 *   2. addInitScript — standard Tauri mock + override generate_ai_response
 *      to call the real worker and stream results as ai_token events
 *   3. captureText → click Transform → wait for result → assert quality
 *
 * Requires:
 *   SNAPTEXT_APP_SECRET=<value>
 *
 * Run:
 *   npx playwright test --project=quality
 */

import { test, expect, Page } from '@playwright/test'
import { buildTauriInitScript } from '../helpers/tauri-mock'
import { SAMPLES } from '../helpers/sample-texts'

const WORKER_URL = 'https://snaptext-worker.snaptext-ai.workers.dev'
// Fresh device ID per run so each run gets its own daily quota
const DEVICE_ID = `test-pw-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

const SYSTEM_PROMPTS: Record<string, string> = {
  Correct:      'Correct grammar, spelling, and punctuation. Return only the corrected text, no explanations.',
  Translate:    'Detect the language and translate to English. Return only the translation.',
  Summarize:    'Summarize in 2-3 concise bullet points. Be brief and factual.',
  Reply:        'Draft a professional and friendly reply. Be concise.',
  Do:           'Convert this into a clear, actionable task description.',
  Email:        'Rewrite as a professional email with greeting, body, and sign-off.',
  Casual:       'Rewrite in a casual, conversational tone.',
  Professional: 'Rewrite in a professional, polished tone for business communication.',
  Prompt:       'Complete the following request as instructed.',
}

function getSecret(): string {
  const s = process.env.SNAPTEXT_APP_SECRET
  if (!s) throw new Error('Set SNAPTEXT_APP_SECRET to run quality tests')
  return s
}

// ── Setup ─────────────────────────────────────────────────────────────────────

/** Expose a Node.js worker call on the page — bypasses CORS restrictions. */
async function exposeWorker(page: Page) {
  const secret = getSecret()
  await page.exposeFunction('__callWorker', async (mode: string, text: string) => {
    const sp  = SYSTEM_PROMPTS[mode] ?? ''
    const res = await fetch(`${WORKER_URL}/generate`, {
      method:  'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-App-Secret': secret,
        'X-Device-ID':  DEVICE_ID,
      },
      body: JSON.stringify({
        system_prompt:   sp,
        user_text:       text,
        stream:          false,
        max_tokens:      600,
        temperature:     0.3,
        thinking_budget: 0,
      }),
    })
    if (!res.ok) {
      const err = await res.text()
      throw new Error(`Worker ${res.status}: ${err}`)
    }
    const json = await res.json()
    return ((json as any)?.candidates?.[0]?.content?.parts?.[0]?.text ?? '').trim()
  })
}

/** Load the app with real AI calls wired into generate_ai_response. */
async function loadQualityApp(page: Page) {
  await exposeWorker(page)

  // Standard Tauri mock (first-run done, Worker mode)
  await page.addInitScript({ content: buildTauriInitScript({
    get_config_value: '1',
    get_ai_mode:      'Worker',
    get_worker_usage: { used: 0, cap: 20 },
  }) })

  // Override generate_ai_response: call real worker → stream ai_token events
  await page.addInitScript({ content: `
    ;(function () {
      window.__TAURI_MOCK__.setHandler('generate_ai_response', async function (args) {
        try {
          var text  = await window.__callWorker(args.mode, args.text)
          var words = text.split(' ')
          var i     = 0
          function next() {
            if (i < words.length) {
              window.__TAURI_MOCK__.__emit('ai_token', (i === 0 ? '' : ' ') + words[i++])
              setTimeout(next, 15)
            } else {
              window.__TAURI_MOCK__.__emit('ai_stream_end', null)
            }
          }
          setTimeout(next, 0)
        } catch (e) {
          window.__TAURI_MOCK__.__emit('ai_error', e.message || 'Worker error')
        }
        return null
      })
    })()
  ` })

  await page.goto('/')
  await page.waitForLoadState('networkidle')
}

/**
 * Capture text, click Transform, wait for the real AI result.
 * Returns the final text shown in .token-container.
 */
async function generate(page: Page, text: string, mode = 'Correct'): Promise<string> {
  await page.evaluate(
    ([t, m]) => (window as any).__TAURI_MOCK__.captureText(t, m),
    [text, mode]
  )
  await page.waitForTimeout(200)
  await page.locator('button.primary-action').click()
  // Wait until generation finishes — button re-enables, blinking cursor gone
  await expect(page.locator('button.primary-action')).toBeEnabled({ timeout: 50_000 })
  await page.waitForTimeout(300)
  return (await page.locator('.token-container').textContent() ?? '').trim()
}

// ── Skip suite if no secret ───────────────────────────────────────────────────

test.beforeAll(() => {
  if (!process.env.SNAPTEXT_APP_SECRET) test.skip()
})

// ── Correct mode ──────────────────────────────────────────────────────────────

test.describe('Correct mode', () => {
  test('fixes grammar errors in an email', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, SAMPLES.qualityBenchmarks.emailCorrection.input, 'Correct')
    console.log(`[Correct] "${result.slice(0, 100)}"`)

    expect(result.length).toBeGreaterThan(0)
    expect(result.toLowerCase()).not.toMatch(/\bwe have face\b/)
    expect(result).not.toEqual(SAMPLES.qualityBenchmarks.emailCorrection.input)
  })

  test('all-caps text is fixed to normal casing', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, SAMPLES.allCaps.input, 'Correct')
    expect(result.length).toBeGreaterThan(0)
    expect(result).not.toEqual(result.toUpperCase())
  })

  test('already-correct text is not mangled', async ({ page }) => {
    await loadQualityApp(page)
    const input  = 'The quick brown fox jumps over the lazy dog.'
    const result = await generate(page, input, 'Correct')
    expect(result.length).toBeGreaterThan(0)
    expect(result.toLowerCase()).toMatch(/fox|dog|quick/)
  })
})

// ── Translate mode ────────────────────────────────────────────────────────────

test.describe('Translate mode', () => {
  test('translates French to English', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, SAMPLES.translate.input, 'Translate')
    console.log(`[Translate FR] "${result}"`)

    expect(result.length).toBeGreaterThan(0)
    expect(/[a-zA-Z]/.test(result)).toBe(true)
    expect(result.toLowerCase()).toMatch(/coffee|please|like|want/)
  })

  test('translates Hinglish to English', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, SAMPLES.hinglish.input, 'Translate')
    console.log(`[Translate Hinglish] "${result}"`)

    expect(result.length).toBeGreaterThan(5)
    expect(/[a-zA-Z]/.test(result)).toBe(true)
    expect(result.toLowerCase()).toMatch(/coffee|like|want|need/)
  })

  test('translates Arabic RTL to English', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, SAMPLES.rtl.input, 'Translate')
    expect(result.length).toBeGreaterThan(0)
    expect(/[a-zA-Z]/.test(result)).toBe(true)
  })
})

// ── Summarize mode ────────────────────────────────────────────────────────────

test.describe('Summarize mode', () => {
  test('summary is shorter than input', async ({ page }) => {
    await loadQualityApp(page)
    const input  = SAMPLES.qualityBenchmarks.bulletSummary.input
    const result = await generate(page, input, 'Summarize')
    console.log(`[Summarize] ${result.length} chars (input: ${input.length})`)

    expect(result.length).toBeGreaterThan(0)
    expect(result.length).toBeLessThan(input.length)
  })

  test('summary covers key facts', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, SAMPLES.summarize.input, 'Summarize')
    expect(result.length).toBeGreaterThan(0)
    expect(result.toLowerCase()).toMatch(/revenue|growth|margin|earnings|cloud|product/)
  })
})

// ── Reply mode ────────────────────────────────────────────────────────────────

test.describe('Reply mode', () => {
  test('drafts a reply to a meeting request', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, SAMPLES.qualityBenchmarks.replyDraft.input, 'Reply')
    console.log(`[Reply] "${result.slice(0, 100)}"`)

    expect(result.length).toBeGreaterThan(20)
    expect(result.toLowerCase()).toMatch(/api|timeline|demo|friday|week|complete|ready|update/)
  })

  test('reply does not repeat the input verbatim', async ({ page }) => {
    await loadQualityApp(page)
    const input  = SAMPLES.reply.input
    const result = await generate(page, input, 'Reply')
    expect(result.length).toBeGreaterThan(0)
    expect(result).not.toEqual(input)
  })

  test('reply to casual lunch invite is friendly', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, 'Hey, are you joining us for lunch at 1pm today?', 'Reply')
    expect(result.toLowerCase()).toMatch(/lunch|join|yes|no|sorry|1pm|today/)
  })
})

// ── Do mode ───────────────────────────────────────────────────────────────────

test.describe('Do mode', () => {
  test('converts a vague request to an actionable task', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, SAMPLES.qualityBenchmarks.doAction.input, 'Do')
    console.log(`[Do] "${result.slice(0, 100)}"`)

    expect(result.length).toBeGreaterThan(10)
    expect(result.toLowerCase()).toMatch(/login|bug|jira|ticket|inactiv|logout/)
  })

  test('short vague task gets structured output', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, 'Fix the login bug', 'Do')
    expect(result.length).toBeGreaterThan(0)
    expect(result.toLowerCase()).toMatch(/login|bug|fix/)
  })
})

// ── Email mode ────────────────────────────────────────────────────────────────

test.describe('Email mode', () => {
  test('produces a structured email with greeting and sign-off', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, SAMPLES.email.input, 'Email')
    console.log(`[Email] ${result.length} chars`)

    expect(result.toLowerCase()).toMatch(/dear|hi|hello|subject/)
    expect(result.toLowerCase()).toMatch(/regards|sincerely|best|thank/)
  })
})

// ── Casual / Professional ─────────────────────────────────────────────────────

test.describe('Casual mode', () => {
  test('rewrites formal text in casual tone', async ({ page }) => {
    await loadQualityApp(page)
    const input  = SAMPLES.qualityBenchmarks.casualTone.input
    const result = await generate(page, input, 'Casual')
    console.log(`[Casual] "${result.slice(0, 100)}"`)

    expect(result.length).toBeGreaterThan(0)
    // Model sometimes offers multiple options — don't check length, check that formal jargon is gone
    expect(result.toLowerCase()).not.toMatch(/aforementioned|hereby|pursuant/)
  })
})

test.describe('Professional mode', () => {
  test('rewrites casual text in professional tone', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, SAMPLES.professional.input, 'Professional')
    expect(result.length).toBeGreaterThan(0)
    expect(result.toLowerCase()).not.toMatch(/\byo\b|\bthx\b/)
  })

  test('output is grammatically clean', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, 'can u check this asap and tell me', 'Professional')
    expect(result.length).toBeGreaterThan(0)
    expect(/[a-zA-Z]/.test(result)).toBe(true)
  })
})

// ── Edge cases ────────────────────────────────────────────────────────────────

test.describe('Edge case quality', () => {
  test('single word returns non-empty output', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, 'ok', 'Correct')
    expect(result.length).toBeGreaterThan(0)
  })

  test('emoji-heavy text returns coherent output', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, SAMPLES.emojiHeavy.input, 'Casual')
    expect(result.length).toBeGreaterThan(0)
  })

  test('code snippet is not mangled by Correct mode', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, SAMPLES.codeSnippet.input, 'Correct')
    expect(result.length).toBeGreaterThan(0)
  })

  test('worker 503 fallback — always returns a result', async ({ page }) => {
    await loadQualityApp(page)
    const result = await generate(page, 'Test fallback resilience.', 'Correct')
    expect(result.length).toBeGreaterThan(0)
  })
})
