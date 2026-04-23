import { test, expect, Page } from '@playwright/test'
import { buildTauriInitScript } from '../helpers/tauri-mock'

async function setup(page: Page, text = 'Fix my grammer mistake') {
  await page.addInitScript({ content: buildTauriInitScript() })
  await page.goto('/')
  await page.waitForLoadState('networkidle')
  await page.evaluate((t) => (window as any).__TAURI_MOCK__.captureText(t, 'Correct'), text)
  await page.waitForTimeout(200)
}

async function generateResult(page: Page, result = 'Corrected sentence here.') {
  await page.locator('button.primary-action').click()
  await page.evaluate((r) => (window as any).__TAURI_MOCK__.streamResponse(r, 20), result)
  await expect(page.locator('.token-container')).toContainText(result, { timeout: 5000 })
}

test.describe('Keyboard shortcuts', () => {
  test('Ctrl+Enter triggers generation', async ({ page }) => {
    await setup(page)
    await page.keyboard.press('Control+Enter')
    await expect(page.getByText('Thinking…')).toBeVisible({ timeout: 3000 })
  })

  test('Ctrl+Enter does nothing without captured text', async ({ page }) => {
    await page.addInitScript({ content: buildTauriInitScript() })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await page.keyboard.press('Control+Enter')
    // Should not show Thinking (no text)
    await expect(page.getByText('Thinking…')).not.toBeVisible()
  })

  test('Escape calls hide_window when history panel is closed', async ({ page }) => {
    await setup(page)
    let hideCalled = false
    await page.evaluate(() => {
      ;(window as any).__TAURI_MOCK__.setHandler('hide_window', () => {
        ;(window as any).__hideCalled = true
        return null
      })
    })
    await page.keyboard.press('Escape')
    hideCalled = await page.evaluate(() => !!(window as any).__hideCalled)
    expect(hideCalled).toBe(true)
  })

  test('Escape closes history panel instead of hiding window', async ({ page }) => {
    await setup(page)
    // Open history panel first
    await page.locator('button[title="Recent transforms"]').click()
    await expect(page.getByText('Recent transforms')).toBeVisible({ timeout: 3000 })

    let hideCalled = false
    await page.evaluate(() => {
      ;(window as any).__TAURI_MOCK__.setHandler('hide_window', () => {
        ;(window as any).__hideCalled = true
        return null
      })
    })
    await page.keyboard.press('Escape')
    await expect(page.getByText('Recent transforms')).not.toBeVisible({ timeout: 3000 })
    hideCalled = await page.evaluate(() => !!(window as any).__hideCalled)
    expect(hideCalled).toBe(false)
  })

  test('C key sets copied state (button turns green)', async ({ page }) => {
    await setup(page)
    await generateResult(page)

    // Press C key (not Ctrl+C)
    await page.keyboard.press('c')
    // App sets copied=true → button color becomes #22c55e (green)
    const copyBtn = page.locator('button[title="Copy (C)"]')
    await expect(copyBtn).toHaveCSS('color', 'rgb(34, 197, 94)', { timeout: 2000 })
  })

  test('C key does nothing without a result', async ({ page }) => {
    await setup(page)
    // No result yet — press C, nothing should break
    await page.keyboard.press('c')
    // No error should appear
    await expect(page.locator('.token-container')).not.toContainText(/error/i)
  })

  test('Tab key calls inject_result when result is available', async ({ page }) => {
    await setup(page)
    await generateResult(page, 'Corrected sentence here.')

    let injectedText = ''
    await page.evaluate(() => {
      ;(window as any).__TAURI_MOCK__.setHandler('inject_result', (args: any) => {
        ;(window as any).__injectedText = args.text
        return null
      })
    })
    await page.keyboard.press('Tab')
    injectedText = await page.evaluate(() => (window as any).__injectedText ?? '')
    expect(injectedText).toContain('Corrected sentence here.')
  })

  test('Tab key does nothing without a result', async ({ page }) => {
    await setup(page)
    let injectedText: string | null = null
    await page.evaluate(() => {
      ;(window as any).__TAURI_MOCK__.setHandler('inject_result', (args: any) => {
        ;(window as any).__injectedText = args.text
        return null
      })
    })
    await page.keyboard.press('Tab')
    injectedText = await page.evaluate(() => (window as any).__injectedText ?? null)
    expect(injectedText).toBeNull()
  })

  test('Tab key does nothing while generation is in progress', async ({ page }) => {
    await setup(page)
    await page.locator('button.primary-action').click()
    // Don't emit stream_end — still generating
    let injectedText: string | null = null
    await page.evaluate(() => {
      ;(window as any).__TAURI_MOCK__.setHandler('inject_result', (args: any) => {
        ;(window as any).__injectedText = args.text
        return null
      })
    })
    await page.keyboard.press('Tab')
    injectedText = await page.evaluate(() => (window as any).__injectedText ?? null)
    expect(injectedText).toBeNull()
  })

  test('mode pills are focusable via keyboard', async ({ page }) => {
    await setup(page)
    const firstPill = page.locator('button.mode-pill').first()
    await firstPill.focus()
    await expect(firstPill).toBeFocused()
  })
})
