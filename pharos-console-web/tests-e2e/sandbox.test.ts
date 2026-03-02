/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: pharos-console-web/tests-e2e/sandbox.test.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * E2E verification of the Sandbox mode and its interaction with the 
 * pharos-server backend.
 * * Traceability:
 * Related to Bug #82 (Issue #82).
 * ======================================================================== */
import { test, expect } from '@playwright/test';

test.describe('Sandbox Mode Backend Interaction', () => {
  test.beforeEach(async ({ page }) => {
    // Login first
    await page.goto('/login');
    await page.fill('input[name="username"]', 'admin');
    await page.fill('input[name="password"]', 'admin');
    await page.click('button[type="submit"]');
    await expect(page).toHaveURL(/\/$/);
  });

  test('should execute a query and show 200 OK from backend', async ({ page }) => {
    // Navigate to a page with the Sandbox Terminal (usually the home page in sandbox mode)
    await page.goto('/');
    
    const queryInput = page.locator('#query-input');
    await expect(queryInput).toBeVisible();

    // Execute a simple status query
    await queryInput.fill('status');
    await page.click('button:has-text("Execute")');

    // Wait for the response in the terminal output
    const terminalOutput = page.locator('#terminal-output');
    // If the backend is running, it should return some records or OK.
    // If it's NOT running, it should show a "Failed to connect" error.
    await expect(terminalOutput).toContainText('200 OK', { timeout: 10000 });
  });
});
