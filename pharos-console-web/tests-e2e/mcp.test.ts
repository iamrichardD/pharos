/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: tests-e2e/mcp.test.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * E2E test suite for the WebMCP JSON-RPC 2.0 Gateway. Ensures that 
 * AI agents can interact with the Pharos protocol via the Web Console.
 * * Traceability:
 * Related to Phase 22 (Task 22.1, Task 22.2).
 * ======================================================================== */
import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';

test.describe('WebMCP Gateway', () => {
    const storePath = path.join(process.cwd(), 'data/auth_store.json');

    test.beforeEach(async () => {
        if (fs.existsSync(storePath)) {
            fs.unlinkSync(storePath);
        }
    });

    test('should return 401 if unauthenticated', async ({ request }) => {
        const response = await request.post('/mcp', {
            data: {
                jsonrpc: '2.0',
                method: 'query_mdb',
                params: { query: 'status=online' },
                id: 1
            }
        });
        expect(response.status()).toBe(401);
        const body = await response.json();
        expect(body.error.message).toBe('Unauthorized');
    });

    test('should execute query_mdb when authenticated', async ({ page, request }) => {
        // First login
        await page.goto('/login');
        await page.fill('input[name="username"]', 'admin');
        await page.fill('input[name="password"]', 'admin');
        await page.click('button[type="submit"]');

        // Handle mandatory password change
        await expect(page).toHaveURL(/\/change-password/);
        await page.fill('input[name="password"]', 'NewSecurePassword123!');
        await page.fill('input[name="confirmPassword"]', 'NewSecurePassword123!');
        await page.click('button[type="submit"]');
        
        // Wait for redirect to home
        await page.waitForURL('/');

        // Now perform the MCP request using the page's context (which has the session cookie)
        const mcpResponse = await page.evaluate(async () => {
            const res = await fetch('/mcp', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    jsonrpc: '2.0',
                    method: 'query_mdb',
                    params: { query: 'status=online' },
                    id: 123
                })
            });
            return await res.json();
        });

        expect(mcpResponse.jsonrpc).toBe('2.0');
        expect(mcpResponse.id).toBe(123);
        expect(mcpResponse.result).toBeDefined();
        expect(mcpResponse.result.type).toBe('matches');
        // Check if records are flattened as expected in mcp.ts
        if (mcpResponse.result.records.length > 0) {
            expect(mcpResponse.result.records[0].hostname).toBeDefined();
            expect(mcpResponse.result.records[0].id).toBeDefined();
        }
    });

    test('should return error for invalid jsonrpc version', async ({ page }) => {
        // Authenticate
        await page.goto('/login');
        await page.fill('input[name="username"]', 'admin');
        await page.fill('input[name="password"]', 'admin');
        await page.click('button[type="submit"]');

        await expect(page).toHaveURL(/\/change-password/);
        await page.fill('input[name="password"]', 'NewSecurePassword123!');
        await page.fill('input[name="confirmPassword"]', 'NewSecurePassword123!');
        await page.click('button[type="submit"]');
        await page.waitForURL('/');

        const mcpResponse = await page.evaluate(async () => {
            const res = await fetch('/mcp', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    jsonrpc: '1.0',
                    method: 'query_mdb',
                    params: { query: 'status=online' },
                    id: 456
                })
            });
            return await res.json();
        });

        expect(mcpResponse.error.code).toBe(-32600);
        expect(mcpResponse.error.message).toContain('jsonrpc must be 2.0');
    });

    test('should return error for non-existent method', async ({ page }) => {
        // Authenticate
        await page.goto('/login');
        await page.fill('input[name="username"]', 'admin');
        await page.fill('input[name="password"]', 'admin');
        await page.click('button[type="submit"]');

        await expect(page).toHaveURL(/\/change-password/);
        await page.fill('input[name="password"]', 'NewSecurePassword123!');
        await page.fill('input[name="confirmPassword"]', 'NewSecurePassword123!');
        await page.click('button[type="submit"]');
        await page.waitForURL('/');

        const mcpResponse = await page.evaluate(async () => {
            const res = await fetch('/mcp', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    jsonrpc: '2.0',
                    method: 'invalid_method',
                    params: {},
                    id: 789
                })
            });
            return await res.json();
        });

        expect(mcpResponse.error.code).toBe(-32601);
        expect(mcpResponse.error.message).toContain('Method not found');
    });
});
