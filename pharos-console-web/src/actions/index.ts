/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/actions/index.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Defines Astro Actions for server-side operations. Uses feature-specific
 * logic from the Vertical Slices.
 * * Traceability:
 * Related to Task 16.6 (Issue #69) and Bug #115 (Issue #114).
 * ======================================================================== */
import { defineAction } from 'astro:actions';
import { z } from 'astro:schema';
import { commitMdbRecord } from '../features/mdb/add/add-logic';
import { createSessionToken } from '../features/auth/session-logic';
import { AUTH_COOKIE_NAME } from '../features/auth/auth-config';
import { verifyStoredPassword, updateStoredPassword } from '../features/auth/password-store';
import { executePharosQuery, executeAuthCheck } from '../lib/pharos';

export const server = {
    sandboxQuery: defineAction({
        accept: 'form',
        input: z.object({
            query: z.string().min(1, 'Query is required')
        }),
        handler: async (input) => {
            if (process.env.PHAROS_SANDBOX !== 'true') {
                throw new Error('Sandbox mode is not enabled');
            }
            try {
                const host = process.env.PHAROS_HOST;
                const port = process.env.PHAROS_PORT ? parseInt(process.env.PHAROS_PORT, 10) : undefined;
                const res = await executePharosQuery('sandbox-term', input.query, host, port);
                return { success: true, result: res };
            } catch (e: any) {
                throw new Error(e.message || 'Sandbox query failed');
            }
        }
    }),
    login: defineAction({
        accept: 'form',
        input: z.object({
            username: z.string().min(1, 'Username is required'),
            password: z.string().min(1, 'Password is required'),
        }),
        handler: async (input, context) => {
            // Home Lab Mode (MVP): Simple credential check
            // Use the password store if it exists, otherwise fall back to environment variables.
            const storedVerification = await verifyStoredPassword(input.password);
            const ADMIN_PASSWORD = process.env.ADMIN_PASSWORD || 'admin';
            
            let authenticated = false;
            let mustChangePassword = false;

            if (storedVerification === null) {
                // First run: Use default password and enforce change
                if (input.username === 'admin' && input.password === ADMIN_PASSWORD) {
                    authenticated = true;
                    mustChangePassword = true;
                }
            } else if (storedVerification === true && input.username === 'admin') {
                authenticated = true;
                mustChangePassword = false;
            }
            
            if (authenticated) {
                const token = await createSessionToken(input.username, ['admin'], mustChangePassword);
                
                context.cookies.set(AUTH_COOKIE_NAME, token, {
                    httpOnly: true,
                    // Use secure cookies if the request is over HTTPS.
                    // This is essential for the Sandbox and Production TLS environments.
                    secure: context.url.protocol === 'https:',
                    sameSite: 'lax',
                    path: '/',
                    maxAge: 60 * 60 * 24 // 24 hours
                });
                return { success: true, mustChangePassword };
            }
            throw new Error('Invalid credentials');
        }
    }),
    updatePassword: defineAction({
        accept: 'form',
        input: z.object({
            password: z.string().min(8, 'Password must be at least 8 characters'),
            confirmPassword: z.string().min(8)
        }),
        handler: async (input, context) => {
            if (input.password !== input.confirmPassword) {
                throw new Error('Passwords do not match');
            }

            const session = context.locals.session;
            if (!session || session.userId !== 'admin') {
                throw new Error('Unauthorized');
            }

            const success = await updateStoredPassword(input.password);
            if (success) {
                // Issue a fresh token without the mustChangePassword flag
                const token = await createSessionToken(session.userId, session.roles, false);
                context.cookies.set(AUTH_COOKIE_NAME, token, {
                    httpOnly: true,
                    // Use secure cookies if the request is over HTTPS.
                    secure: context.url.protocol === 'https:',
                    sameSite: 'lax',
                    path: '/',
                    maxAge: 60 * 60 * 24 // 24 hours
                });
                return { success: true };
            }
            throw new Error('Failed to update password store');
        }
    }),
    handshakeLogin: defineAction({
        accept: 'form',
        input: z.object({
            publicKey: z.string().min(1, 'Public key is required'),
            signature: z.string().min(1, 'Signature is required'),
            challenge: z.string().min(1, 'Challenge is required'),
        }),
        handler: async (input, context) => {
            try {
                const host = process.env.PHAROS_HOST;
                const port = process.env.PHAROS_PORT ? parseInt(process.env.PHAROS_PORT, 10) : undefined;
                const isValid = await executeAuthCheck(input.publicKey, input.signature, input.challenge, host, port);
                
                if (isValid) {
                    // Extract user ID from public key (simplified: last part of comment if exists)
                    const userId = input.publicKey.split(' ').pop() || 'cli-user';
                    const token = await createSessionToken(userId, ['admin']);
                    
                    context.cookies.set(AUTH_COOKIE_NAME, token, {
                        httpOnly: true,
                        // Use secure cookies if the request is over HTTPS.
                        secure: context.url.protocol === 'https:',
                        sameSite: 'lax',
                        path: '/',
                        maxAge: 60 * 60 * 24 // 24 hours
                    });
                    return { success: true };
                }
            } catch (e: any) {
                console.error('Handshake verification error:', e);
            }
            throw new Error('Handshake verification failed');
        }
    }),
    logout: defineAction({
        handler: async (_, context) => {
            context.cookies.delete(AUTH_COOKIE_NAME, { path: '/' });
            return { success: true };
        }
    }),
    addMachine: defineAction({
        accept: 'form',
        input: z.object({
            hostname: z.string().min(1, 'Hostname is required'),
            ip: z.string().min(1, 'IP is required'),
            mac: z.string().optional(),
            os: z.string().optional(),
            alias: z.string().optional(),
            actionType: z.enum(['stage', 'commit']).default('stage')
        }),
        handler: async (input) => {
            if (input.actionType === 'stage') {
                return { staged: true, data: input };
            } else if (input.actionType === 'commit') {
                try {
                    const host = process.env.PHAROS_HOST;
                    const port = process.env.PHAROS_PORT ? parseInt(process.env.PHAROS_PORT, 10) : undefined;
                    const res = await commitMdbRecord(input, host, port);
                    if (res.type === 'error') {
                        throw new Error(res.message || 'Failed to add record');
                    }
                    return { staged: false, success: true, message: 'Record added successfully' };
                } catch (e: any) {
                    throw new Error(e.message || 'Failed to connect to Pharos server');
                }
            }
            return { staged: false, success: false, message: 'Invalid action type' };
        }
    })
};
