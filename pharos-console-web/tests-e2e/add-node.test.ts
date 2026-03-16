/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: pharos-console-web/tests-e2e/add-node.test.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * E2E verification of the "Add Node" UI and "Search Examples" guidance.
 * * Traceability:
 * Related to Task 21.4 (Issue #133).
 * ======================================================================== */
import { test, expect } from '@playwright/test';

test.describe('Add Node and Search Guidance', () => {
    test('should navigate to Add Node page and show installation commands', async ({ page }) => {
        await page.goto('/');
        
        // Check for Quick Action card (specifically the one on the home page, not the nav link)
        const addNodeCard = page.locator('main a[href="/add-node"]').first();
        await expect(addNodeCard).toBeVisible();
        await addNodeCard.click();

        await expect(page).toHaveURL(/\/add-node/);
        await expect(page.locator('h1')).toContainText('Add a New Node');
        
        // Verify installation commands are visible
        await expect(page.locator('code').first()).toContainText('scripts/install.sh');
        await expect(page.locator('code').last()).toContainText('pharos-pulse');
    });

    test('should populate search bar from examples and trigger search', async ({ page }) => {
        await page.goto('/mdb');
        
        const searchInput = page.locator('#search');
        await expect(searchInput).toBeVisible();

        // Click an example button
        const exampleBtn = page.locator('.example-btn').filter({ hasText: 'Online Only' });
        await expect(exampleBtn).toBeVisible();
        await exampleBtn.click();

        // Verify input is populated
        await expect(searchInput).toHaveValue('status=online');

        // Submit form
        await page.keyboard.press('Enter');

        // Verify URL contains the query
        await expect(page).toHaveURL(/\/mdb\?q=status%3Donline/);
    });

    test('should show CLI power user hint with current query', async ({ page }) => {
        await page.goto('/mdb?q=hostname%3Dpharos-*');
        
        const hint = page.locator('#cli-hint');
        await expect(hint).toBeVisible();
        await expect(hint).toContainText('mdb "hostname=pharos-*"');
    });
});
