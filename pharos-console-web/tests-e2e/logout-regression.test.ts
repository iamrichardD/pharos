/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: pharos-console-web/tests-e2e/logout-regression.test.ts
 * Author: Gemini CLI (Senior Systems Developer)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Regression test for Bug #115: Logout button non-functional after
 * client-side navigation following a mandatory password change.
 * * Traceability:
 * Related to Issue #114.
 * ======================================================================== */
import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';

test.describe('Logout Regression', () => {
  const storePath = path.join(process.cwd(), 'data/auth_store.json');
  let storeBackup: string | null = null;

  test.beforeAll(async () => {
    if (fs.existsSync(storePath)) {
      storeBackup = fs.readFileSync(storePath, 'utf-8');
    }
  });

  test.afterAll(async () => {
    if (storeBackup) {
      fs.writeFileSync(storePath, storeBackup);
    } else if (fs.existsSync(storePath)) {
      fs.unlinkSync(storePath);
    }
  });

  test.beforeEach(async () => {
    if (fs.existsSync(storePath)) {
      fs.unlinkSync(storePath);
    }
  });

  test('should logout successfully after a mandatory password change', async ({ page }) => {
    // 1. Go to login
    await page.goto('/login');

    // 2. Login with default credentials (admin/admin)
    await page.fill('input[name="username"]', 'admin');
    await page.fill('input[name="password"]', 'admin');
    await page.click('button[type="submit"]');

    // 3. Should be redirected to /change-password
    await expect(page).toHaveURL(/\/change-password$/);
    await expect(page.locator('text=Secure Your Lab')).toBeVisible();

    // 4. Change password
    await page.fill('input[name="password"]', 'new-secure-password');
    await page.fill('input[name="confirmPassword"]', 'new-secure-password');
    await page.click('button[type="submit"]');

    // 5. Should be redirected to home (/)
    await expect(page).toHaveURL(/\/$/);
    await expect(page.locator('#logoutBtn')).toBeVisible();

    // 6. Click Logout
    await page.click('#logoutBtn');

    // 7. Should be redirected back to /login
    // This will FAIL if the bug exists because the event listener won't be attached
    await expect(page).toHaveURL(/\/login$/);
  });
});
