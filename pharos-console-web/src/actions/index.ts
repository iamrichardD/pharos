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
 * Related to Task 16.6 (Issue #69).
 * ======================================================================== */
import { defineAction } from 'astro:actions';
import { z } from 'astro:schema';
import { commitMdbRecord } from '../features/mdb/add/add-logic';
import { createSessionToken } from '../features/auth/session-logic';
import { AUTH_COOKIE_NAME } from '../features/auth/auth-config';
import { executePharosQuery } from '../lib/pharos';

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
                const res = await executePharosQuery('sandbox-term', input.query);
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
            // Enterprise Mode: Will extend this to LDAP/OIDC in Task 16.4 (Part B)
            const ADMIN_PASSWORD = process.env.ADMIN_PASSWORD || 'admin';
            
            if (input.username === 'admin' && input.password === ADMIN_PASSWORD) {
                const token = await createSessionToken(input.username, ['admin']);
                context.cookies.set(AUTH_COOKIE_NAME, token, {
                    httpOnly: true,
                    secure: process.env.NODE_ENV === 'production',
                    sameSite: 'strict',
                    path: '/',
                    maxAge: 60 * 60 * 24 // 24 hours
                });
                return { success: true };
            }
            throw new Error('Invalid credentials');
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
                    const res = await commitMdbRecord(input);
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
