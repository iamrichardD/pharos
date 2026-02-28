/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/features/mdb/search/search-logic.test.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Tests the MDB search logic (Vertical Slice).
 * Ensures correct handling of empty queries and delegation to the client.
 * * Traceability:
 * Related to Task 16.6 (Issue #69).
 * ======================================================================== */

import { describe, it, expect, vi } from 'vitest';
import { searchMdb } from './search-logic';
import * as pharosClient from '../../../lib/pharos';

vi.mock('../../../lib/pharos', () => ({
    executePharosQuery: vi.fn(),
}));

describe('searchMdb', () => {
    it('test_should_return_empty_ok_when_query_is_empty', async () => {
        const result = await searchMdb('');
        expect(result.type).toBe('ok');
        expect(result.records).toEqual([]);
        expect(pharosClient.executePharosQuery).not.toHaveBeenCalled();
    });

    it('test_should_call_executePharosQuery_with_trimmed_query', async () => {
        const mockResponse: pharosClient.PharosResponse = { type: 'matches', count: 1, records: [] };
        vi.mocked(pharosClient.executePharosQuery).mockResolvedValue(mockResponse);

        const result = await searchMdb('  return all  ');
        
        expect(pharosClient.executePharosQuery).toHaveBeenCalledWith('web-mdb-search', 'return all');
        expect(result).toEqual(mockResponse);
    });
});
