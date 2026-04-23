import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './tests',
  timeout: 30_000,
  expect: { timeout: 6_000 },
  fullyParallel: false,   // avoid multiple dev-server restarts
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  reporter: [['list'], ['html', { outputFolder: 'playwright-report', open: 'never' }]],

  projects: [
    {
      name: 'ui',
      testMatch: 'tests/ui/**/*.spec.ts',
      use: {
        ...devices['Desktop Chrome'],
        baseURL: 'http://localhost:5173',
        viewport: { width: 480, height: 620 },
        permissions: ['clipboard-read', 'clipboard-write'],
      },
    },
    {
      name: 'quality',
      testMatch: 'tests/quality/**/*.spec.ts',
      timeout: 60_000,     // real AI calls can be slow
      use: {
        ...devices['Desktop Chrome'],
        baseURL: 'http://localhost:5173',
        viewport: { width: 480, height: 620 },
      },
    },
  ],

  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:5173',
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
    stdout: 'ignore',
    stderr: 'pipe',
  },
})
