import { defineConfig, devices } from '@playwright/test';

// Specs that cannot share the stack with anything else: two of them stop the
// realtime container, one floods the write limiter on purpose, one suspends a
// seeded user mid-run, and one posts 120 messages spread over three authors to
// stay under that same limiter. They run as their own batch, one worker.
const SOLO = /(qa-flow|qa-flow2|qa-flow4|durable-delivery|message-pagination)\.spec\.ts$/;

export default defineConfig({
  testDir: './e2e',
  globalSetup: './e2e/global-setup.ts',
  timeout: 30_000,
  expect: { timeout: 15_000 },
  // Files run in parallel, tests inside a file stay in order: the qa-flow specs
  // are narratives that depend on it. One worker unless asked otherwise, so a
  // bare `playwright test` is always the safe thing to run.
  fullyParallel: false,
  workers: Number(process.env.E2E_WORKERS) || 1,
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
  projects: [
    {
      name: 'chromium',
      // The mobile spec asserts phone geometry; running it at desktop width
      // fails for the right reason and the wrong project.
      testIgnore: [/mobile\.spec\.ts/, SOLO],
      use: { ...devices['Desktop Chrome'] },
    },
    {
      name: 'serial',
      testMatch: SOLO,
      use: { ...devices['Desktop Chrome'] },
    },
    // Layout regressions only. Everything emulation cannot reach — the virtual
    // keyboard, safe areas, the install flow — is in docs/manual-qa.md.
    {
      name: 'mobile',
      testMatch: /mobile\.spec\.ts/,
      use: { ...devices['Pixel 7'] },
    },
  ],
});
