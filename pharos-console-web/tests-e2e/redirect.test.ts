import { test, expect } from '@playwright/test';

test.describe('HTTP Redirect', () => {
    test('should redirect http to https on port 3000', async ({ request }) => {
        const response = await request.get('http://localhost:3000', {
            maxRedirects: 0
        });
        expect(response.status()).toBe(301);
        expect(response.headers().location).toContain('https://localhost:3000');
    });
});
