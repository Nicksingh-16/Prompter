import { test, expect, Page } from '@playwright/test'
import { buildTauriInitScript } from '../helpers/tauri-mock'

async function setup(page: Page, text = 'Fix this sentance please') {
  await page.addInitScript({ content: buildTauriInitScript() })
  await page.goto('/')
  await page.waitForLoadState('networkidle')
  await page.evaluate((t) => (window as any).__TAURI_MOCK__.captureText(t, 'Correct'), text)
  await page.waitForTimeout(200)
}

// Use .primary-action class to avoid matching the "Recent transforms" history button
const clickTransform = (page: Page) => page.locator('button.primary-action').click()

const streamResponse = (page: Page, text = 'This sentence is corrected.') =>
  page.evaluate((r) => (window as any).__TAURI_MOCK__.streamResponse(r, 30), text)

test.describe('Transform flow', () => {
  test('Transform button is enabled when text is captured', async ({ page }) => {
    await setup(page)
    await expect(page.locator('button.primary-action')).toBeEnabled()
  })

  test('Transform button is disabled with no captured text', async ({ page }) => {
    await page.addInitScript({ content: buildTauriInitScript() })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await expect(page.locator('button.primary-action')).toBeDisabled()
  })

  test('output shows "Thinking…" while generating', async ({ page }) => {
    await setup(page)
    await clickTransform(page)
    await expect(page.getByText('Thinking…')).toBeVisible({ timeout: 3000 })
  })

  test('streaming tokens appear in token-container', async ({ page }) => {
    await setup(page)
    await clickTransform(page)
    await streamResponse(page, 'Fixed: This sentence is correct.')
    await expect(page.locator('.token-container')).toContainText('Fixed', { timeout: 5000 })
  })

  test('full response is shown after stream completes', async ({ page }) => {
    const response = 'This sentence is corrected properly.'
    await setup(page)
    await clickTransform(page)
    await streamResponse(page, response)
    await expect(page.locator('.token-container')).toContainText(response, { timeout: 5000 })
  })

  test('Transform button shows "Generating…" during generation', async ({ page }) => {
    await setup(page)
    await clickTransform(page)
    await expect(page.locator('button.primary-action')).toContainText('Generating', { timeout: 3000 })
  })

  test('blinking-cursor class applied during generation', async ({ page }) => {
    await setup(page)
    await clickTransform(page)
    await expect(page.locator('.token-container.blinking-cursor')).toBeVisible({ timeout: 3000 })
  })

  test('blinking-cursor class removed after stream ends', async ({ page }) => {
    await setup(page)
    await clickTransform(page)
    await streamResponse(page, 'Done.')
    await expect(page.locator('.token-container.blinking-cursor')).not.toBeVisible({ timeout: 5000 })
  })

  test('Transform button re-enables after stream completes', async ({ page }) => {
    await setup(page)
    await clickTransform(page)
    await streamResponse(page, 'Done.')
    await expect(page.locator('button.primary-action')).toBeEnabled({ timeout: 5000 })
  })

  test('Insert (CheckCircle) button is disabled before result', async ({ page }) => {
    await setup(page)
    // No generation yet — insert button disabled
    const insertBtn = page.locator('button[title*="Insert"]')
    await expect(insertBtn).toBeDisabled()
  })

  test('Insert button enabled after result arrives', async ({ page }) => {
    await setup(page)
    await clickTransform(page)
    await streamResponse(page, 'Result here.')
    const insertBtn = page.locator('button[title*="Insert"]')
    await expect(insertBtn).toBeEnabled({ timeout: 5000 })
  })

  test('Copy button enabled after result arrives', async ({ page }) => {
    await setup(page)
    await clickTransform(page)
    await streamResponse(page, 'Result here.')
    const copyBtn = page.locator('button[title*="Copy"]')
    await expect(copyBtn).toBeEnabled({ timeout: 5000 })
  })

  test('new text capture clears previous result', async ({ page }) => {
    await setup(page, 'First text')
    await clickTransform(page)
    await streamResponse(page, 'First result.')
    await expect(page.locator('.token-container')).toContainText('First result.', { timeout: 4000 })

    await page.evaluate(() => (window as any).__TAURI_MOCK__.captureText('Second text', 'Correct'))
    await page.waitForTimeout(200)
    await expect(page.locator('.token-container')).not.toContainText('First result.')
  })

  test('Custom mode: Transform disabled without prompt', async ({ page }) => {
    await setup(page)
    await page.getByRole('button', { name: '···' }).click()
    await page.getByRole('button', { name: /^Custom$/ }).click()
    await expect(page.locator('button.primary-action')).toBeDisabled()
  })

  test('Custom mode: Transform enabled after entering prompt', async ({ page }) => {
    await setup(page)
    await page.getByRole('button', { name: '···' }).click()
    await page.getByRole('button', { name: /^Custom$/ }).click()
    await page.getByPlaceholder(/make it a tweet|translate to french/i).fill('Make it a haiku')
    await expect(page.locator('button.primary-action')).toBeEnabled()
  })

  test('Ctrl+Enter triggers generation', async ({ page }) => {
    await setup(page)
    await page.keyboard.press('Control+Enter')
    await expect(page.getByText('Thinking…')).toBeVisible({ timeout: 3000 })
  })

  test('double-click Transform does not duplicate requests (debounce)', async ({ page }) => {
    await setup(page)
    let invokeCount = 0
    await page.evaluate(() => {
      ;(window as any).__TAURI_MOCK__.setHandler('generate_ai_response', () => {
        ;(window as any).__invokeCount = ((window as any).__invokeCount || 0) + 1
        return null
      })
    })
    const btn = page.locator('button.primary-action')
    await btn.click()
    await btn.click({ force: true })
    await page.waitForTimeout(600)
    const count: number = await page.evaluate(() => (window as any).__invokeCount ?? 0)
    expect(count).toBeLessThanOrEqual(1)
  })

  test('usage counter decrements after stream ends', async ({ page }) => {
    // Initial state: used=3 → 17 left.
    // Install the tracking handler BEFORE captureText so captureText's refreshUsage
    // counts as call #1 (returns used=3, keeps "17 left"), and stream_end's refreshUsage
    // counts as call #2 (returns used=4 → "16 left").
    await page.addInitScript({ content: buildTauriInitScript({ get_worker_usage: { used: 3, cap: 20 } }) })
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    await page.evaluate(() => {
      ;(window as any).__TAURI_MOCK__.setHandler('get_worker_usage', () => {
        const c = ((window as any).__usageCallCount = ((window as any).__usageCallCount || 0) + 1)
        return c <= 1 ? { used: 3, cap: 20 } : { used: 4, cap: 20 }
      })
    })
    await page.evaluate(() => (window as any).__TAURI_MOCK__.captureText('test', 'Correct'))
    await page.waitForTimeout(200)
    await clickTransform(page)
    await streamResponse(page, 'Done.')
    await expect(page.getByText('16 left')).toBeVisible({ timeout: 5000 })
  })
})
