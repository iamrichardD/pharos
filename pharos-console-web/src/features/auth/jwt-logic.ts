/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/features/auth/jwt-logic.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Provides stateless JWT session management using the 'jose' library.
 * This ensures mobile-friendly, secure authentication that survives
 * server restarts without requiring a database.
 * * Traceability:
 * Related to Task 16.4 (Issue #66).
 * ======================================================================== */
import { SignJWT, jwtVerify, JWTPayload } from 'jose';

export interface UserSession extends JWTPayload {
    userId: string;
    roles: string[];
}

/**
 * Signs a payload into a JWT token using the provided secret.
 */
export async function signToken(payload: UserSession, secret: string, expiresIn = '24h'): Promise<string> {
    const secretKey = new TextEncoder().encode(secret);
    return await new SignJWT(payload)
        .setProtectedHeader({ alg: 'HS256' })
        .setIssuedAt()
        .setExpirationTime(expiresIn)
        .sign(secretKey);
}

/**
 * Verifies a JWT token and returns the decoded payload.
 */
export async function verifyToken(token: string, secret: string): Promise<UserSession> {
    const secretKey = new TextEncoder().encode(secret);
    const { payload } = await jwtVerify(token, secretKey);
    return payload as UserSession;
}
