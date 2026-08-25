import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  globalSetup: './e2e/global-setup.ts',
  timeout: 30_000,
  expect: { timeout: 15_000 },
  fullyParallel: false,
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  // Locally, a run that has started failing is nearly always a broken stack
  // rather than ten broken features; stop and say so instead of spending
  // minutes proving it. CI reports everything.
  maxFailures: process.env.CI ? 0 : 5,
  reporter: 'list',
  use: {
    baseURL: process.env.E2E_BASE_URL || 'http://localhost:8080',
    testIdAttribute: 'data-qa',
    trace: 'on-first-retry',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
});
