import { test, expect, Page } from '@playwright/test'
import { buildTauriInitScript } from '../helpers/tauri-mock'

async function loadWithText(page: Page, text = 'Hello world, lets fix this.', suggestedMode = 'Correct') {
  await page.addInitScript({ content: buildTauriInitScript() })
  await page.goto('/')
  await page.waitForLoadState('networkidle')
  await page.evaluate(
    ([t, m]) => (window as any).__TAURI_MOCK__.captureText(t, m),
    [text, suggestedMode]
  )
  await page.waitForTimeout(200)
}

test.describe('Mode selection', () => {
  test('all 4 primary mode pills are visible after text capture', async ({ page }) => {
    await loadWithText(page)
    for (const mode of ['Reply', 'Do', 'Correct', 'Prompt']) {
      await expect(page.getByRole('button', { name: new RegExp(`^${mode}$`) }).first()).toBeVisible()
    }
  })

  test('NLP-suggested mode has active class', async ({ page }) => {
    await loadWithText(page, 'fix my grammer pls', 'Correct')
    const correctPill = page.locator('.mode-pill.active')
    await expect(correctPill).toContainText('Correct')
  })

  test('clicking a different pill makes it active', async ({ page }) => {
    await loadWithText(page, 'hello world', 'Correct')
    await page.getByRole('button', { name: /^Reply$/ }).first().click()
    const activePill = page.locator('.mode-pill.active')
    await expect(activePill).toContainText('Reply')
  })

  test('··· button expands hidden modes', async ({ page }) => {
    await loadWithText(page)
    await page.getByRole('button', { name: '···' }).click()
    await expect(page.getByRole('button', { name: /^Translate$/ })).toBeVisible()
    await expect(page.getByRole('button', { name: /^Email$/ })).toBeVisible()
    await expect(page.getByRole('button', { name: /^Summarize$/ })).toBeVisible()
    await expect(page.getByRole('button', { name: /^Casual$/ })).toBeVisible()
    await expect(page.getByRole('button', { name: /^Knowledge$/ })).toBeVisible()
  })

  test('selecting a hidden mode collapses the tray', async ({ page }) => {
    await loadWithText(page)
    await page.getByRole('button', { name: '···' }).click()
    await page.getByRole('button', { name: /^Translate$/ }).click()
    // Tray collapses — hidden modes no longer in DOM
    await expect(page.getByRole('button', { name: /^Email$/ })).not.toBeVisible()
    // Re-expand to verify Translate retained its active state
    await page.getByRole('button', { name: '···' }).click()
    await expect(page.locator('.mode-pill.active')).toContainText('Translate')
  })

  test('Custom mode shows prompt input', async ({ page }) => {
    await loadWithText(page)
    await page.getByRole('button', { name: '···' }).click()
    await page.getByRole('button', { name: /^Custom$/ }).click()
    await expect(page.getByPlaceholder(/make it a tweet|translate to french/i)).toBeVisible()
  })

  test('Custom prompt input shows /300 character counter', async ({ page }) => {
    await loadWithText(page)
    await page.getByRole('button', { name: '···' }).click()
    await page.getByRole('button', { name: /^Custom$/ }).click()
    await page.getByPlaceholder(/make it a tweet|translate to french/i).fill('Write this as a haiku')
    await expect(page.getByText(/\/300/)).toBeVisible()
  })

  test('Hinglish capture: expanding tray shows Translate as active', async ({ page }) => {
    await page.addInitScript({ content: buildTauriInitScript() })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await page.evaluate(() => (window as any).__TAURI_MOCK__.captureHinglish('mne cofee pasand se tne ke chahiye'))
    await page.waitForTimeout(200)
    // Translate is a hidden mode — must expand tray to see active class
    await page.getByRole('button', { name: '···' }).click()
    await expect(page.locator('.mode-pill.active')).toContainText('Translate')
  })

  test('mixed-language input shows MIXED badge in header', async ({ page }) => {
    await page.addInitScript({ content: buildTauriInitScript() })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await page.evaluate(() => (window as any).__TAURI_MOCK__.captureHinglish('mne cofee pasand se tne ke chahiye'))
    await page.waitForTimeout(200)
    // Use exact:true to avoid partial match on "mixed language detected" text
    await expect(page.getByText('MIXED', { exact: true })).toBeVisible()
  })

  test('ToneMirror shows Hinglish/mixed language label', async ({ page }) => {
    await page.addInitScript({ content: buildTauriInitScript() })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await page.evaluate(() => (window as any).__TAURI_MOCK__.captureHinglish('mne cofee pasand se tne ke chahiye'))
    await page.waitForTimeout(200)
    await expect(page.getByText(/Hinglish.*mixed language detected/i)).toBeVisible()
  })

  test('captured text preview is shown (up to 120 chars)', async ({ page }) => {
    const text = 'I need to fix this sentence grammar error'
    await loadWithText(page, text)
    await expect(page.getByText(`"${text}"`)).toBeVisible()
  })

  test('text longer than 120 chars is truncated with ellipsis in preview', async ({ page }) => {
    const longText = 'A'.repeat(130)
    await loadWithText(page, longText)
    await expect(page.getByText(/…/)).toBeVisible()
  })

  test('app context badge shows 💬 Chat for messaging context', async ({ page }) => {
    await page.addInitScript({ content: buildTauriInitScript() })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await page.evaluate(() =>
      (window as any).__TAURI_MOCK__.captureText('hello there', 'Reply', 'messaging')
    )
    await page.waitForTimeout(200)
    await expect(page.getByText('💬 Chat')).toBeVisible()
  })

  test('app context badge shows ⌨ IDE for code_editor context', async ({ page }) => {
    await page.addInitScript({ content: buildTauriInitScript() })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await page.evaluate(() =>
      (window as any).__TAURI_MOCK__.captureText('const x = 1', 'Prompt', 'code_editor')
    )
    await page.waitForTimeout(200)
    await expect(page.getByText('⌨ IDE')).toBeVisible()
  })

  test('NLP suggested pill shows sparkle icon when not selected', async ({ page }) => {
    await loadWithText(page, 'fix this', 'Correct')
    // Correct is suggested but NOT yet active (wait for mode to settle)
    // The Sparkles icon appears when isNlpSuggested && !isActive
    // After capture, Correct BECOMES active (suggested = active)
    // Click Reply to deselect Correct, then Correct should show sparkle
    await page.getByRole('button', { name: /^Reply$/ }).first().click()
    // Now Correct is no longer active but still NLP-suggested → sparkle visible
    const correctPill = page.locator('button.mode-pill', { hasText: 'Correct' })
    // Sparkle is a lucide SVG inside the button
    await expect(correctPill.locator('svg')).toBeVisible()
  })

  test('−button collapses the hidden mode tray', async ({ page }) => {
    await loadWithText(page)
    await page.getByRole('button', { name: '···' }).click()
    await expect(page.getByRole('button', { name: /^Translate$/ })).toBeVisible()
    await page.getByRole('button', { name: '−' }).click()
    await expect(page.getByRole('button', { name: /^Translate$/ })).not.toBeVisible()
  })
})
