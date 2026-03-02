/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: pharos-console-web/playwright.config.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * E2E test configuration for Playwright. Enables automated browser
 * testing of the Web Console inside the Podman test environment.
 * * Traceability:
 * Related to Phase 17 Sandbox verification.
 * ======================================================================== */
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests-e2e',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1,
  reporter: 'list',
  use: {
    baseURL: 'http://localhost:3000',
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'PHAROS_SANDBOX=true HOST=0.0.0.0 PORT=3000 node dist/server/entry.mjs',
    url: 'http://localhost:3000',
    reuseExistingServer: !process.env.CI,
    timeout: 60 * 1000,
  },
});
