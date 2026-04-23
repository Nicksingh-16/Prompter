import { test, expect, Page } from '@playwright/test'
import { buildTauriInitScript } from '../helpers/tauri-mock'
import { SAMPLES } from '../helpers/sample-texts'

async function loadAndCapture(page: Page, text: string, mode = 'Correct', hinglish = false) {
  await page.addInitScript({ content: buildTauriInitScript() })
  await page.goto('/')
  await page.waitForLoadState('networkidle')
  if (hinglish) {
    await page.evaluate((t) => (window as any).__TAURI_MOCK__.captureHinglish(t), text)
  } else {
    await page.evaluate(([t, m]) => (window as any).__TAURI_MOCK__.captureText(t, m), [text, mode])
  }
  await page.waitForTimeout(200)
}

test.describe('Edge cases', () => {
  test('very long input (>10k chars) shows warning and disables Transform', async ({ page }) => {
    await loadAndCapture(page, SAMPLES.tooLong.input)
    await expect(page.getByText(/too long.*chars|select fewer/i)).toBeVisible({ timeout: 3000 })
    await expect(page.getByRole('button', { name: /^Transform$/ })).toBeDisabled()
  })

  test('single character input is accepted and Transform is enabled', async ({ page }) => {
    await loadAndCapture(page, SAMPLES.singleChar.input)
    await expect(page.getByRole('button', { name: /^Transform$/ })).toBeEnabled()
  })

  test('emoji-heavy text does not crash the UI', async ({ page }) => {
    await loadAndCapture(page, SAMPLES.emojiHeavy.input)
    await expect(page.locator('.glass-card')).toBeVisible()
    await expect(page.getByRole('button', { name: /^Transform$/ })).toBeEnabled()
  })

  test('all-caps input is accepted', async ({ page }) => {
    await loadAndCapture(page, SAMPLES.allCaps.input)
    await expect(page.getByRole('button', { name: /^Transform$/ })).toBeEnabled()
  })

  test('Arabic RTL text does not crash the UI', async ({ page }) => {
    await loadAndCapture(page, SAMPLES.rtl.input)
    await expect(page.locator('.glass-card')).toBeVisible()
    await expect(page.getByRole('button', { name: /^Transform$/ })).toBeEnabled()
  })

  test('multiline input shows first part in preview', async ({ page }) => {
    await loadAndCapture(page, SAMPLES.withNewlines.input)
    await expect(page.getByText(/First point/)).toBeVisible()
  })

  test('Hinglish text shows MIXED badge in header', async ({ page }) => {
    await loadAndCapture(page, SAMPLES.hinglish.input, 'Translate', true)
    await expect(page.getByText('MIXED', { exact: true })).toBeVisible({ timeout: 3000 })
  })

  test('Hinglish text auto-selects Translate mode', async ({ page }) => {
    await loadAndCapture(page, SAMPLES.hinglish.input, 'Translate', true)
    // Translate is a hidden mode — must expand the tray to see the active pill
    await page.getByRole('button', { name: '···' }).click()
    await expect(page.locator('.mode-pill.active')).toContainText('Translate')
  })

  test('Hinglish ToneMirror shows mixed-language note', async ({ page }) => {
    await loadAndCapture(page, SAMPLES.hinglish.input, 'Translate', true)
    await expect(page.getByText(/Hinglish.*mixed language|mixed language detected/i)).toBeVisible({ timeout: 3000 })
  })

  test('Hinglish ToneMirror suggests Translate mode', async ({ page }) => {
    await loadAndCapture(page, SAMPLES.hinglish.input, 'Translate', true)
    await expect(page.getByText(/Translate mode suggested/i)).toBeVisible({ timeout: 3000 })
  })

  test('code snippet input is accepted', async ({ page }) => {
    await loadAndCapture(page, SAMPLES.codeSnippet.input)
    await expect(page.getByRole('button', { name: /^Transform$/ })).toBeEnabled()
  })

  test('sensitive_data_detected shows history-not-saved notice', async ({ page }) => {
    await loadAndCapture(page, 'some text')
    await page.getByRole('button', { name: /^Transform$/ }).click()
    await page.evaluate(() => (window as any).__TAURI_MOCK__.__emit('sensitive_data_detected', 'email address'))
    await expect(page.getByText(/History not saved.*email address/i)).toBeVisible({ timeout: 3000 })
  })

  test('sensitive data notice auto-dismisses after ~4s', async ({ page }) => {
    await loadAndCapture(page, 'some text')
    await page.getByRole('button', { name: /^Transform$/ }).click()
    await page.evaluate(() => (window as any).__TAURI_MOCK__.__emit('sensitive_data_detected', 'password'))
    await expect(page.getByText(/History not saved/i)).toBeVisible({ timeout: 3000 })
    await expect(page.getByText(/History not saved/i)).not.toBeVisible({ timeout: 6000 })
  })

  test('update_available banner appears when event fires', async ({ page }) => {
    await page.addInitScript({ content: buildTauriInitScript() })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await page.evaluate(() =>
      (window as any).__TAURI_MOCK__.__emit('update_available', { version: '2.5.0', body: 'New features' })
    )
    await expect(page.getByText(/SnapText 2.5.0 is available/i)).toBeVisible({ timeout: 3000 })
  })

  test('update banner "Update" button calls plugin:updater|install', async ({ page }) => {
    await page.addInitScript({ content: buildTauriInitScript() })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await page.evaluate(() =>
      (window as any).__TAURI_MOCK__.__emit('update_available', { version: '2.5.0', body: '' })
    )
    await expect(page.getByRole('button', { name: /^Update$/ })).toBeVisible({ timeout: 3000 })
    // Click update — should not throw
    await page.getByRole('button', { name: /^Update$/ }).click()
    await expect(page.getByText(/SnapText 2.5.0/i)).not.toBeVisible({ timeout: 2000 })
  })

  test('update banner × dismiss button hides it', async ({ page }) => {
    await page.addInitScript({ content: buildTauriInitScript() })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await page.evaluate(() =>
      (window as any).__TAURI_MOCK__.__emit('update_available', { version: '2.5.0', body: '' })
    )
    await page.getByRole('button', { name: /^×$/ }).click()
    await expect(page.getByText(/SnapText 2.5.0/i)).not.toBeVisible({ timeout: 2000 })
  })

  test('usage counter shows "0 left" and red banner at daily cap', async ({ page }) => {
    await page.addInitScript({ content: buildTauriInitScript({ get_worker_usage: { used: 20, cap: 20 } }) })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await expect(page.getByText('0 left')).toBeVisible()
    await expect(page.getByText(/daily limit reached/i)).toBeVisible()
  })

  test('running low banner shows correct transform count', async ({ page }) => {
    // remaining = 3 → "3 transforms left today"
    await page.addInitScript({ content: buildTauriInitScript({ get_worker_usage: { used: 17, cap: 20 } }) })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await expect(page.getByText(/3 transforms left today/i)).toBeVisible()
  })

  test('running low banner with 1 remaining uses singular "transform"', async ({ page }) => {
    await page.addInitScript({ content: buildTauriInitScript({ get_worker_usage: { used: 19, cap: 20 } }) })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await expect(page.getByText(/1 transform left today/i)).toBeVisible()
  })
})
