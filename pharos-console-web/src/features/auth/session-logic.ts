/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/features/auth/session-logic.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * High-level session management using JWT. Handles token generation,
 * validation, and extraction of user claims for SSR-based authorization.
 * * Traceability:
 * Related to Task 16.4 (Issue #66).
 * ======================================================================== */
import { signToken, verifyToken, type UserSession } from './jwt-logic';
import { JWT_SECRET, SESSION_EXPIRATION } from './auth-config';

/**
 * Creates a new session token for the given user.
 */
export async function createSessionToken(userId: string, roles: string[]): Promise<string> {
    const payload: UserSession = {
        userId,
        roles,
        sub: userId, // JWT standard subject
    };
    return await signToken(payload, JWT_SECRET, SESSION_EXPIRATION);
}

/**
 * Validates a session token and returns the user session if valid.
 */
export async function getSession(token: string | undefined): Promise<UserSession | null> {
    if (!token) return null;
    try {
        return await verifyToken(token, JWT_SECRET);
    } catch (err) {
        return null;
    }
}
