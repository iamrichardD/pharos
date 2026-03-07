/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: pharos-console-web/tests-e2e/guest-access.test.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Automated E2E verification of Guest (unauthenticated) access to MDB.
 * Ensures that public routes are accessible and data is visible while 
 * write-actions are restricted.
 * * Traceability:
 * Related to Bug #103.
 * ======================================================================== */
import { test, expect } from '@playwright/test';

test.describe('Guest Access (Unauthenticated)', () => {
  test('should allow guest access to /mdb without redirecting to login', async ({ page }) => {
    await page.goto('/mdb');
    await expect(page).toHaveURL(/\/mdb$/);
    await expect(page.locator('h1')).toContainText('Search MDB');
  });

  test('should hide "Add Record +" button for guests on /mdb', async ({ page }) => {
    await page.goto('/mdb');
    await expect(page.locator('text=Add Record +')).toBeHidden();
  });

  test('should redirect unauthenticated users from /mdb/add to /login', async ({ page }) => {
    await page.goto('/mdb/add');
    await expect(page).toHaveURL(/\/login/);
  });

  test('should show search results and include Serial Number for guests', async ({ page }) => {
    // Search for the node added by pulse in playwright config
    await page.goto('/mdb?q=e2e-pharos-main');
    
    const resultsTable = page.locator('table');
    // Ensure table eventually appears
    await expect(resultsTable).toBeVisible({ timeout: 10000 });

    // Serial Number header should be visible (No masking per user preference)
    await expect(page.locator('th:has-text("Serial Number")')).toBeVisible();
    
    // Action button "View Details" should be visible
    await expect(page.locator('text=View Details').first()).toBeVisible();
    
    // Navigate to details
    await page.click('text=View Details');
    
    // Should be on details page
    await expect(page).toHaveURL(/\/mdb\//);
    
    // Serial Number on details page should be visible
    await expect(page.locator('text=Serial Number')).toBeVisible();
    // Full Metadata table should be visible
    await expect(page.locator('text=Full Record Metadata')).toBeVisible();
  });
});

test.describe('Authenticated MDB Access', () => {
  test.beforeEach(async ({ page }) => {
    // Login first
    await page.goto('/login');
    await page.fill('input[name="username"]', 'admin');
    await page.fill('input[name="password"]', 'admin');
    await page.click('button[type="submit"]');
    await expect(page).toHaveURL(/\/$/);
  });

  test('should show "Add Record +" button for authenticated users on /mdb', async ({ page }) => {
    await page.goto('/mdb');
    await expect(page.locator('text=Add Record +')).toBeVisible();
  });

  test('should allow authenticated users to access /mdb/add', async ({ page }) => {
    await page.goto('/mdb/add');
    await expect(page).toHaveURL(/\/mdb\/add$/);
    await expect(page.locator('h1')).toContainText('Add Machine Record');
  });
});
