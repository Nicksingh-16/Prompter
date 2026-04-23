import { test, expect, Page } from '@playwright/test'
import { buildTauriInitScript } from '../helpers/tauri-mock'

async function loadApp(page: Page, overrides?: Record<string, unknown>) {
  await page.addInitScript({ content: buildTauriInitScript(overrides) })
  await page.goto('/')
  await page.waitForLoadState('networkidle')
}

test.describe('App startup', () => {
  test('renders main overlay without crash', async ({ page }) => {
    await loadApp(page)
    await expect(page.locator('.glass-card')).toBeVisible({ timeout: 5000 })
  })

  test('shows SnapText branding in header', async ({ page }) => {
    await loadApp(page)
    await expect(page.getByText('SnapText').first()).toBeVisible()
  })

  test('shows welcome screen for first-time users', async ({ page }) => {
    // get_config_value throws → first_run_done not set
    await loadApp(page, { get_config_value: '__THROW__' })
    // First-run screen has "Got it — show me a demo" button
    await expect(page.getByRole('button', { name: /got it.*demo|show me a demo/i })).toBeVisible({ timeout: 5000 })
  })

  test('welcome screen shows hotkey instructions', async ({ page }) => {
    await loadApp(page, { get_config_value: '__THROW__' })
    await expect(page.getByText(/alt\+k/i).first()).toBeVisible({ timeout: 4000 })
  })

  test('dismissing first-run screen shows main UI', async ({ page }) => {
    await loadApp(page, { get_config_value: '__THROW__' })
    await page.getByRole('button', { name: /got it.*demo|show me a demo/i }).click()
    await expect(page.getByText('Reply').first()).toBeVisible({ timeout: 3000 })
  })

  test('displays usage counter as "X left" on load', async ({ page }) => {
    // used=3, cap=20 → remaining=17
    await loadApp(page, { get_worker_usage: { used: 3, cap: 20 } })
    await expect(page.getByText(/17 left/)).toBeVisible()
  })

  test('shows amber warning when remaining transforms <= 5', async ({ page }) => {
    // used=16 → remaining=4 → isRunningLow
    await loadApp(page, { get_worker_usage: { used: 16, cap: 20 } })
    await expect(page.getByText(/running low|transforms left today/i)).toBeVisible()
  })

  test('shows red limit-reached banner when 0 remaining', async ({ page }) => {
    await loadApp(page, { get_worker_usage: { used: 20, cap: 20 } })
    await expect(page.getByText(/daily limit reached/i)).toBeVisible()
  })

  test('shows "0 left" in usage counter at cap', async ({ page }) => {
    await loadApp(page, { get_worker_usage: { used: 20, cap: 20 } })
    await expect(page.getByText('0 left')).toBeVisible()
  })

  test('shows Worker mode indicator by default', async ({ page }) => {
    await loadApp(page, { get_ai_mode: 'Worker' })
    await expect(page.getByText(/🛡️ Worker|Worker/)).toBeVisible()
  })

  test('shows Direct mode indicator when BYOK configured', async ({ page }) => {
    await loadApp(page, { get_ai_mode: 'Byok', has_api_key: true })
    await expect(page.getByText(/🔒 Direct|Direct/)).toBeVisible()
  })

  test('shows Local mode indicator when Ollama mode', async ({ page }) => {
    await loadApp(page, { get_ai_mode: 'Local' })
    await expect(page.getByText(/⚡ Local|Local/)).toBeVisible()
  })

  test('idle state shows "Waiting for captured text" placeholder', async ({ page }) => {
    await loadApp(page)
    await expect(page.getByText(/waiting for captured text/i)).toBeVisible()
  })

  test('idle state shows developer-focused placeholder in preview', async ({ page }) => {
    await loadApp(page)
    await expect(page.getByText(/alt\+k.*structured prompt/i)).toBeVisible()
  })

  test('mode pills visible after boot (no text captured)', async ({ page }) => {
    await loadApp(page)
    // Without NLP context, static pills render
    for (const mode of ['Reply', 'Do', 'Correct', 'Prompt']) {
      await expect(page.getByRole('button', { name: new RegExp(`^${mode}$`) }).first()).toBeVisible()
    }
  })
})
