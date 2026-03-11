/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: pharos-console-web/tests-e2e/login.test.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Automated E2E verification of the login flow and navigation.
 * Ensures redirects and cookie handling work correctly.
 * ======================================================================== */
import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';

test.describe('Authentication Flow', () => {
  const storePath = path.join(process.cwd(), 'data/auth_store.json');

  test.beforeEach(async () => {
    if (fs.existsSync(storePath)) {
      fs.unlinkSync(storePath);
    }
  });

  test('should allow unauthenticated users to see the landing page on root', async ({ page }) => {
    await page.goto('/');
    await expect(page).toHaveURL(/\/$/);
    await expect(page.locator('text=Manage your Home Lab')).toBeVisible();
    await expect(page.locator('text=Get Started')).toBeVisible();
    await expect(page.locator('text=Read Documentation')).toBeVisible();
  });

  test('should login successfully with admin/admin and handle mandatory password change', async ({ page }) => {
    await page.goto('/login');
    
    // Fill the form
    await page.fill('input[name="username"]', 'admin');
    await page.fill('input[name="password"]', 'admin');
    
    // Click Sign In
    await page.click('button[type="submit"]');

    // Should be redirected to /change-password
    await expect(page).toHaveURL(/\/change-password/);
    await expect(page.locator('text=Secure Your Lab')).toBeVisible();

    // Change password to continue
    await page.fill('input[name="password"]', 'NewSecurePassword123!');
    await page.fill('input[name="confirmPassword"]', 'NewSecurePassword123!');
    await page.click('button[type="submit"]');

    // Should redirect to home (/) and show the logout button
    await expect(page).toHaveURL(/\/$/);
    await expect(page.locator('#logoutBtn')).toBeVisible();
    await expect(page.locator('header').getByText('admin')).toBeVisible();
  });

  test('should show error message for invalid credentials', async ({ page }) => {
    await page.goto('/login');
    await page.fill('input[name="username"]', 'admin');
    await page.fill('input[name="password"]', 'wrong-pass');
    await page.click('button[type="submit"]');

    await expect(page.locator('#errorMessage')).toBeVisible();
    await expect(page.locator('#errorMessage')).toContainText('Invalid credentials');
  });

  test('should switch between authentication tabs correctly', async ({ page }) => {
    await page.goto('/login');

    // Default tab should be Home Lab (Standard)
    await expect(page.locator('#standardPanel')).toBeVisible();
    await expect(page.locator('#handshakePanel')).toBeHidden();

    // Click DevSecOps tab
    await page.click('button[data-tab="handshake"]');
    await expect(page.locator('#handshakePanel')).toBeVisible();
    await expect(page.locator('#standardPanel')).toBeHidden();

    // Click Enterprise tab
    await page.click('button[data-tab="enterprise"]');
    await expect(page.locator('#enterprisePanel')).toBeVisible();
    await expect(page.locator('#handshakePanel')).toBeHidden();
  });
});
