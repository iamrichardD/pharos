/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/features/mdb/add/add-logic.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Logic for adding new MDB records. Handles protocol command generation.
 * * Traceability:
 * Related to Task 16.6 (Issue #69).
 * ======================================================================== */
import { executePharosQuery, type PharosResponse } from '../../../lib/pharos';

export interface MdbAddInput {
    hostname: string;
    ip: string;
    mac?: string;
    os?: string;
    alias?: string;
}

/**
 * Commits a new machine record to the Pharos server.
 * Security Review: Inputs are escaped via protocol syntax (double-quote escaping).
 */
export async function commitMdbRecord(input: MdbAddInput): Promise<PharosResponse> {
    const esc = (s?: string) => s ? s.replace(/"/g, '\\"') : '';
    
    let queryStr = `add type="machine" hostname="${esc(input.hostname)}" ip="${esc(input.ip)}"`;
    if (input.mac) queryStr += ` mac="${esc(input.mac)}"`;
    if (input.os) queryStr += ` os="${esc(input.os)}"`;
    if (input.alias) queryStr += ` alias="${esc(input.alias)}"`;

    return executePharosQuery('web-console-add', queryStr);
}
