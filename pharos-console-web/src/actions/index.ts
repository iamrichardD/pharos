/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/actions/index.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Defines Astro Actions for server-side operations, particularly the HitL
 * (Human-in-the-Loop) additions for MDB records.
 * * Traceability:
 * Related to Task 16.3 (Issue #65).
 * ======================================================================== */
import { defineAction } from 'astro:actions';
import { z } from 'astro:schema';
import { executePharosQuery } from '../lib/pharos';

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
                let queryStr = `add type="machine" hostname="${input.hostname}" ip="${input.ip}"`;
                if (input.mac) queryStr += ` mac="${input.mac}"`;
                if (input.os) queryStr += ` os="${input.os}"`;
                if (input.alias) queryStr += ` alias="${input.alias}"`;

                try {
                    const res = await executePharosQuery('web-console', queryStr);
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
