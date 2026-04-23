import { test, expect, Page } from '@playwright/test'
import { buildTauriInitScript } from '../helpers/tauri-mock'

const SAMPLE_HISTORY = [
  { id: 1, timestamp: '2026-04-22 10:30:00', mode: 'Correct', input_preview: 'Fix my spelling mistakes', output: 'Fix my spelling mistakes. (corrected)' },
  { id: 2, timestamp: '2026-04-22 09:15:00', mode: 'Reply',   input_preview: 'Are you coming to the meeting', output: 'Yes, I will be there at 3pm.' },
  { id: 3, timestamp: '2026-04-21 16:00:00', mode: 'Summarize', input_preview: 'Long article about AI trends...', output: '• AI adoption growing\n• Cost reduction key\n• Ethics concerns remain' },
]

async function loadWithHistory(page: Page, history = SAMPLE_HISTORY) {
  await page.addInitScript({ content: buildTauriInitScript({ get_history: history }) })
  await page.goto('/')
  await page.waitForLoadState('networkidle')
  await page.evaluate(() => (window as any).__TAURI_MOCK__.captureText('test text', 'Correct'))
  await page.waitForTimeout(200)
}

const openHistory = (page: Page) =>
  page.getByRole('button', { name: /recent transforms/i, exact: false })
    .or(page.locator('button[title="Recent transforms"]'))
    .click()

test.describe('History panel', () => {
  test('history button opens the panel', async ({ page }) => {
    await loadWithHistory(page)
    await openHistory(page)
    await expect(page.getByText('Recent transforms')).toBeVisible({ timeout: 3000 })
  })

  test('history panel shows entries with mode badges', async ({ page }) => {
    await loadWithHistory(page)
    await openHistory(page)
    await expect(page.getByText('Correct').first()).toBeVisible({ timeout: 3000 })
    await expect(page.getByText('Reply').first()).toBeVisible({ timeout: 3000 })
  })

  test('history panel shows input preview in italics', async ({ page }) => {
    await loadWithHistory(page)
    await openHistory(page)
    await expect(page.getByText('"Fix my spelling mistakes"')).toBeVisible({ timeout: 3000 })
  })

  test('history panel shows output snippet', async ({ page }) => {
    await loadWithHistory(page)
    await openHistory(page)
    await expect(page.getByText(/Yes, I will be there/)).toBeVisible({ timeout: 3000 })
  })

  test('empty history shows "No history yet" message', async ({ page }) => {
    await page.addInitScript({ content: buildTauriInitScript({ get_history: [] }) })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
    await page.evaluate(() => (window as any).__TAURI_MOCK__.captureText('test', 'Correct'))
    await page.waitForTimeout(200)
    await openHistory(page)
    await expect(page.getByText(/no history yet|transform some text first/i)).toBeVisible({ timeout: 3000 })
  })

  test('search field filters entries by input_preview', async ({ page }) => {
    await loadWithHistory(page)
    await openHistory(page)
    const searchInput = page.getByPlaceholder('Search history…')
    await expect(searchInput).toBeVisible({ timeout: 3000 })
    await searchInput.fill('meeting')
    await expect(page.getByText(/Are you coming to the meeting/)).toBeVisible()
    await expect(page.getByText(/Fix my spelling mistakes/)).not.toBeVisible()
  })

  test('search "no matches" message when query finds nothing', async ({ page }) => {
    await loadWithHistory(page)
    await openHistory(page)
    const searchInput = page.getByPlaceholder('Search history…')
    await searchInput.fill('xyznotfound123')
    await expect(page.getByText(/no matches/i)).toBeVisible({ timeout: 3000 })
  })

  test('Escape closes the history panel', async ({ page }) => {
    await loadWithHistory(page)
    await openHistory(page)
    await expect(page.getByText('Recent transforms')).toBeVisible({ timeout: 3000 })
    await page.keyboard.press('Escape')
    await expect(page.getByText('Recent transforms')).not.toBeVisible({ timeout: 3000 })
  })

  test('clicking X button closes the history panel', async ({ page }) => {
    await loadWithHistory(page)
    await openHistory(page)
    await expect(page.getByText('Recent transforms')).toBeVisible({ timeout: 3000 })
    // X button is inside the history panel header
    await page.locator('.glass-card [style*="position: absolute"] button').first().click()
    await expect(page.getByText('Recent transforms')).not.toBeVisible({ timeout: 3000 })
  })

  test('history entries show date from timestamp', async ({ page }) => {
    await loadWithHistory(page)
    await openHistory(page)
    // App shows entry.timestamp.split(' ')[0] → '2026-04-22'
    await expect(page.getByText('2026-04-22').first()).toBeVisible({ timeout: 3000 })
  })

  test('restoring a history entry populates the output', async ({ page }) => {
    await loadWithHistory(page)
    await openHistory(page)
    // Click the first entry
    await page.getByText('"Fix my spelling mistakes"').click()
    // History panel should close and result should be shown
    await expect(page.getByText('Recent transforms')).not.toBeVisible({ timeout: 3000 })
    await expect(page.locator('.token-container')).toContainText('Fix my spelling mistakes. (corrected)', { timeout: 3000 })
  })

  test('new text capture closes history panel automatically', async ({ page }) => {
    await loadWithHistory(page)
    await openHistory(page)
    await expect(page.getByText('Recent transforms')).toBeVisible({ timeout: 3000 })
    // New text_captured event should auto-close the panel
    await page.evaluate(() => (window as any).__TAURI_MOCK__.captureText('new text', 'Correct'))
    await page.waitForTimeout(300)
    await expect(page.getByText('Recent transforms')).not.toBeVisible()
  })
})
