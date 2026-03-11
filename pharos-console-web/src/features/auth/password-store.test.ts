/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/features/auth/password-store.test.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Verifies that the password store correctly persists and validates 
 * changed passwords, ensuring that users are not locked out after 
 * a mandatory password rotation.
 * * Traceability:
 * Reproduces the regression reported by the user (Bug #114 variant).
 * ======================================================================== */
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { verifyStoredPassword, updateStoredPassword } from './password-store';

const TEST_STORE_PATH = path.join(process.cwd(), 'data/test_auth_store.json');

describe('Password Store Persistence', () => {
    beforeEach(() => {
        process.env.AUTH_STORE_PATH = TEST_STORE_PATH;
        if (fs.existsSync(TEST_STORE_PATH)) {
            fs.unlinkSync(TEST_STORE_PATH);
        }
    });

    afterEach(() => {
        if (fs.existsSync(TEST_STORE_PATH)) {
            fs.unlinkSync(TEST_STORE_PATH);
        }
    });

    it('test_should_return_null_when_no_store_exists', async () => {
        // Rationale: Ensures we correctly detect the "first-run" state 
        // to allow login with default credentials.
        const result = await verifyStoredPassword('any-password');
        expect(result).toBeNull();
    });

    it('test_should_verify_password_after_update', async () => {
        // Rationale: Verifies that a newly set password can be correctly 
        // validated, preventing user lockout.
        const newPassword = 'NewSecurePassword123!';
        const updateSuccess = await updateStoredPassword(newPassword);
        expect(updateSuccess).toBe(true);

        const verifyResult = await verifyStoredPassword(newPassword);
        expect(verifyResult).toBe(true);
    });

    it('test_should_reject_incorrect_password', async () => {
        // Rationale: Ensures that the authentication remains secure 
        // and does not allow unauthorized access with wrong credentials.
        await updateStoredPassword('correct-password');
        const verifyResult = await verifyStoredPassword('wrong-password');
        expect(verifyResult).toBe(false);
    });
});
