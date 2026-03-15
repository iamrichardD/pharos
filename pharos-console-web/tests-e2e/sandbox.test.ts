/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: pharos-console-web/tests-e2e/sandbox.test.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * E2E verification of the Sandbox mode and its "Resource-First" preview UI.
 * * Traceability:
 * Related to Task 22.2 (Resource-First Realignment).
 * ======================================================================== */
import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';

const storePath = path.join(process.cwd(), 'data/auth_store.json');

test.beforeEach(async () => {
  if (fs.existsSync(storePath)) {
    fs.unlinkSync(storePath);
  }
});

test.describe('Sandbox Mode Resource Preview', () => {
  test.beforeEach(async ({ page }) => {
    // 1. Login first
    await page.goto('/login');
    await page.fill('input[name="username"]', 'admin');
    await page.fill('input[name="password"]', 'admin');
    await page.click('button[type="submit"]');
    
    // 2. Handle mandatory password change
    await expect(page).toHaveURL(/\/change-password/);
    await page.fill('input[name="password"]', 'NewSecurePassword123!');
    await page.fill('input[name="confirmPassword"]', 'NewSecurePassword123!');
    await page.click('button[type="submit"]');
    
    // 3. Should be on home page
    await expect(page).toHaveURL(/\/$/);
  });

  test('should display the Live Resource Preview and at least one active node', async ({ page }) => {
    await page.goto('/');
    
    // Verify Resource Preview container is visible
    const preview = page.locator('.resource-preview');
    await expect(preview).toBeVisible();
    await expect(preview).toContainText('Live Resource Preview');
    await expect(preview).toContainText('WebMCP Active');

    // Wait for the pulse agent to register and show up in the preview
    // In our playwright config, pharos-pulse is started as 'e2e-pharos-main'
    const resource = preview.getByText('e2e-pharos-main');
    await expect(resource).toBeVisible({ timeout: 15000 });
    
    // Verify it shows as online
    const statusIndicator = preview.locator('.bg-emerald-500');
    await expect(statusIndicator).toBeVisible();
  });

  test('should navigate to resource details from preview', async ({ page }) => {
    await page.goto('/');
    
    // Wait for resource to appear
    const preview = page.locator('.resource-preview');
    const resource = preview.getByText('e2e-pharos-main');
    await expect(resource).toBeVisible({ timeout: 15000 });
    
    // Click "Inspect"
    await page.click('text=Inspect →');
    
    // Verify navigation to detail page
    await expect(page).toHaveURL(/\/mdb\/e2e-pharos-main/);
    await expect(page.locator('h1')).toContainText('e2e-pharos-main');
    
    // Verify machine-optimized metadata is present
    const metadata = await page.locator('script[type="application/pharos+json"]').innerHTML();
    const record = JSON.parse(metadata);
    expect(record.fields.find((f: any) => f.key === 'hostname').value).toBe('e2e-pharos-main');
  });
});
