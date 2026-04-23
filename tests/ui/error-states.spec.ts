import { test, expect, Page } from '@playwright/test'
import { buildTauriInitScript } from '../helpers/tauri-mock'

async function setupWithText(page: Page, text = 'Correct my grammar please') {
  await page.addInitScript({ content: buildTauriInitScript() })
  await page.goto('/')
  await page.waitForLoadState('networkidle')
  await page.evaluate(
    (t) => (window as any).__TAURI_MOCK__.captureText(t, 'Correct'),
    text
  )
  await page.waitForTimeout(200)
}

async function clickTransformAndError(page: Page, errorMsg: string) {
  await page.locator('button.primary-action').click()
  await page.evaluate((msg) => (window as any).__TAURI_MOCK__.streamError(msg), errorMsg)
}

test.describe('Error states', () => {
  test('503 overload shows friendly message instead of raw JSON', async ({ page }) => {
    await setupWithText(page)
    const rawError = 'Worker error 503 Service Unavailable: {"error":"Gemini error 503: {\\n \\"error\\": {\\n \\"code\\": 503,\\n \\"message\\": \\"This model is currently experiencing high demand\\"\\n}\\n}"}'
    await clickTransformAndError(page, rawError)
    // Should NOT show the raw JSON blob
    await expect(page.getByText(/503|Service Unavailable/)).not.toBeVisible({ timeout: 3000 })
    // Should show the friendly message
    await expect(page.getByText(/overloaded|busy|try again/i)).toBeVisible({ timeout: 3000 })
  })

  test('503 error shows Retry button', async ({ page }) => {
    await setupWithText(page)
    await clickTransformAndError(page, 'Worker error 503 Service Unavailable: {"error":"Gemini error 503: high demand"}')
    await expect(page.getByRole('button', { name: /retry/i })).toBeVisible({ timeout: 3000 })
  })

  test('Retry button clears error and re-triggers generation', async ({ page }) => {
    await setupWithText(page)
    await clickTransformAndError(page, 'Worker error 503 Service Unavailable: {"error":"Gemini error 503"}')
    const retryBtn = page.getByRole('button', { name: /retry/i })
    await expect(retryBtn).toBeVisible({ timeout: 3000 })
    await retryBtn.click()
    // Error should clear and generation should restart
    await expect(page.getByText(/thinking/i)).toBeVisible({ timeout: 3000 })
  })

  test('network error shows Retry button', async ({ page }) => {
    await setupWithText(page)
    await clickTransformAndError(page, 'Network error: connection refused')
    await expect(page.getByRole('button', { name: /retry/i })).toBeVisible({ timeout: 3000 })
  })

  test('daily limit error shows meaningful message', async ({ page }) => {
    await setupWithText(page)
    await clickTransformAndError(page, 'Worker error 429: {"error":"Daily limit reached (20/20). Resets at midnight."}')
    await expect(page.getByText(/limit|daily|resets/i)).toBeVisible({ timeout: 3000 })
  })

  test('input too long shows warning and blocks Transform', async ({ page }) => {
    const longText = 'word '.repeat(2100).trim()    // > 10,000 chars
    await setupWithText(page, longText)
    await expect(page.getByText(/too long|characters/i)).toBeVisible({ timeout: 3000 })
    await expect(page.locator('button.primary-action')).toBeDisabled()
  })

  test('generating state is cleared after error', async ({ page }) => {
    await setupWithText(page)
    await page.locator('button.primary-action').click()
    await page.evaluate(() => (window as any).__TAURI_MOCK__.streamError('Network error'))
    // isGenerating should become false → button re-enabled
    await expect(page.locator('button.primary-action')).toBeEnabled({ timeout: 4000 })
  })

  test('error clears when new text is captured', async ({ page }) => {
    await setupWithText(page)
    await clickTransformAndError(page, 'Network error: timeout')
    await expect(page.getByText(/network error/i)).toBeVisible({ timeout: 3000 })

    // New text capture should clear the error
    await page.evaluate(() => (window as any).__TAURI_MOCK__.captureText('Fresh new text', 'Correct'))
    await page.waitForTimeout(200)
    await expect(page.getByText(/network error/i)).not.toBeVisible()
  })

  test('error clears when Transform is clicked again', async ({ page }) => {
    await setupWithText(page)
    await clickTransformAndError(page, 'Network error')
    await page.waitForTimeout(600)
    await page.locator('button.primary-action').click()
    await expect(page.getByText(/network error/i)).not.toBeVisible()
  })

  test('"Gemini busy — retrying" message shows during BYOK fallback', async ({ page }) => {
    await setupWithText(page)
    await page.locator('button.primary-action').click()
    // Simulate the "retrying with your API key" interim message
    await page.evaluate(() => (window as any).__TAURI_MOCK__.__emit('ai_error', 'Gemini busy — retrying with your API key…'))
    // App maps 'busy' errors → "Gemini is overloaded — please try again in a moment"
    await expect(page.getByText(/overloaded|try again/i)).toBeVisible({ timeout: 3000 })
    // Then stream a successful response (fallback succeeded)
    await page.evaluate(() => (window as any).__TAURI_MOCK__.streamResponse('Corrected text here.', 30))
    await expect(page.locator('.token-container')).toContainText('Corrected', { timeout: 5000 })
  })

  test('UNAVAILABLE in error shows overloaded message', async ({ page }) => {
    await setupWithText(page)
    await clickTransformAndError(page, 'Worker error 503: {"error":"Gemini error 503: UNAVAILABLE"}')
    await expect(page.getByText(/overloaded|busy/i)).toBeVisible({ timeout: 3000 })
  })
})
