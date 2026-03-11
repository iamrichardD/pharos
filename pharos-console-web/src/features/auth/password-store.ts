/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/features/auth/password-store.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Provides a secure, file-based persistence layer for the Home Labber 
 * admin password. Uses scrypt for hashing to ensure cryptographic safety 
 * without the overhead of a database.
 * * Traceability:
 * Related to Task 16.15 (Issue #113).
 * ======================================================================== */
import * as crypto from 'node:crypto';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { promisify } from 'node:util';

const scrypt = promisify(crypto.scrypt);

/**
 * Returns the path to the password store.
 */
function getStorePath(): string {
    return process.env.AUTH_STORE_PATH || path.join(process.cwd(), 'data/auth_store.json');
}

interface AuthData {
    hash: string;
    salt: string;
    updatedAt: string;
}

/**
 * Verifies a password against the stored hash.
 * If no store exists, it returns null to indicate that the system
 * is still in its default "first-run" state.
 */
export async function verifyStoredPassword(password: string): Promise<boolean | null> {
    const storePath = getStorePath();
    if (!fs.existsSync(storePath)) {
        return null;
    }

    try {
        const data: AuthData = JSON.parse(fs.readFileSync(storePath, 'utf-8'));
        const derivedKey = await scrypt(password, data.salt, 64) as Buffer;
        return crypto.timingSafeEqual(Buffer.from(data.hash, 'hex'), derivedKey);
    } catch (err) {
        console.error('Failed to read or verify password store:', err);
        return false;
    }
}

/**
 * Updates the stored password.
 */
export async function updateStoredPassword(password: string): Promise<boolean> {
    const salt = crypto.randomBytes(16).toString('hex');
    const derivedKey = await scrypt(password, salt, 64) as Buffer;
    
    const data: AuthData = {
        hash: derivedKey.toString('hex'),
        salt: salt,
        updatedAt: new Date().toISOString()
    };

    try {
        const storePath = getStorePath();
        const dir = path.dirname(storePath);
        if (!fs.existsSync(dir)) {
            fs.mkdirSync(dir, { recursive: true });
        }
        fs.writeFileSync(storePath, JSON.stringify(data), 'utf-8');
        return true;
    } catch (err) {
        console.error('Failed to update password store:', err);
        return false;
    }
}
