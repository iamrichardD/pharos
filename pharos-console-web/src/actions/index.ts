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

export const server = {
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
