/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/features/auth/jwt-logic.test.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Unit tests for JWT session management to ensure tokens are correctly
 * signed, contain required claims, and are properly validated.
 * * Traceability:
 * Related to Task 16.4 (Issue #66).
 * ======================================================================== */
import { describe, it, expect } from 'vitest';
import { signToken, verifyToken } from './jwt-logic';

describe('JWT Logic', () => {
    const mockPayload = { userId: 'test-user', roles: ['admin'] };
    const secret = 'test-secret-key-at-least-32-chars-long';

    it('test_should_sign_and_verify_valid_token_when_using_correct_secret', async () => {
        const token = await signToken(mockPayload, secret);
        expect(token).toBeDefined();
        expect(typeof token).toBe('string');

        const payload = await verifyToken(token, secret);
        expect(payload.userId).toBe(mockPayload.userId);
        expect(payload.roles).toEqual(mockPayload.roles);
    });

    it('test_should_fail_verification_when_using_incorrect_secret', async () => {
        const token = await signToken(mockPayload, secret);
        await expect(verifyToken(token, 'wrong-secret-key-32-chars-long-xxx')).rejects.toThrow();
    });

    it('test_should_fail_verification_when_token_is_expired', async () => {
        // Use 1s as minimum string duration supported by jose
        const token = await signToken(mockPayload, secret, '1s');
        
        // Wait for 1.1s
        await new Promise(resolve => setTimeout(resolve, 1100));

        await expect(verifyToken(token, secret)).rejects.toThrow();
    });
});
