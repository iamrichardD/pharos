/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/features/mdb/search/search-logic.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Decouples the MDB search logic from the UI. Handles input validation
 * and routes queries to the Pharos protocol client.
 * * Traceability:
 * Related to Task 16.6 (Issue #69).
 * ======================================================================== */

import { executePharosQuery, type PharosResponse } from '../../../lib/pharos';

export async function searchMdb(query: string): Promise<PharosResponse> {
    const trimmedQuery = query?.trim();
    if (!trimmedQuery) {
        return { type: 'ok', records: [] };
    }
    
    // Security Review: Sanitize query or restrict commands?
    // For now, we allow the full query but enforce the client ID.
    return executePharosQuery('web-mdb-search', trimmedQuery);
}
