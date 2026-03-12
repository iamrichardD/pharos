/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/features/mdb/search/search-logic.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Contains the business logic for MDB search operations, including
 * interaction with the Pharos protocol library and result processing.
 * * Traceability:
 * Related to Task 16.11 (Issue #98).
 * ======================================================================== */
import { executePharosQuery, type PharosResponse } from '../../../lib/pharos';

/**
 * Searches machine records using the Pharos protocol.
 * Supports client-side pagination (slicing) of the result set.
 */
export async function searchMdb(query: string, page = 1, pageSize = 25, host?: string, port?: number): Promise<PharosResponse> {
    const trimmedQuery = query?.trim();
    if (!trimmedQuery) {
        return { type: 'ok', records: [] };
    }
    
    // Security Review: Sanitize query or restrict commands?
    // For now, we allow the full query but enforce the client ID.
    const response = await executePharosQuery('web-mdb-search', trimmedQuery, host, port);
    
    // Normalize 501 No matches to an empty ok result
    if (response.type === 'error' && response.code === 501) {
        return { type: 'ok', records: [], count: 0 };
    }

    if (response.type === 'matches' && response.records) {
        const total = response.count ?? response.records.length;
        const start = (page - 1) * pageSize;
        const end = start + pageSize;
        const slicedRecords = response.records.slice(start, end);
        
        return {
            ...response,
            records: slicedRecords,
            count: total // Ensure count is the total, not the slice size
        };
    }
    
    return response;
}
