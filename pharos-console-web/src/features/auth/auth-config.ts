/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/features/auth/auth-config.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Centralized configuration for authentication, defining session cookie 
 * names and security parameters to ensure consistent auth behavior.
 * * Traceability:
 * Related to Task 16.4 (Issue #66).
 * ======================================================================== */

// In production, JWT_SECRET MUST be set via environment variable.
export const JWT_SECRET = process.env.JWT_SECRET || 'dev-secret-at-least-32-chars-long-123456789';
export const AUTH_COOKIE_NAME = 'pharos_session';
export const SESSION_EXPIRATION = '24h';
