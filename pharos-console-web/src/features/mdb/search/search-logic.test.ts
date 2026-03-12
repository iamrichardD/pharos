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
        
        expect(pharosClient.executePharosQuery).toHaveBeenCalledWith('web-mdb-search', 'return all', undefined, undefined);
        expect(result).toEqual(mockResponse);
    });

    it('test_should_honor_explicit_host_and_port', async () => {
        const mockResponse: pharosClient.PharosResponse = { type: 'matches', count: 1, records: [] };
        vi.mocked(pharosClient.executePharosQuery).mockResolvedValue(mockResponse);

        await searchMdb('all', 1, 25, 'custom-host', 1234);
        
        expect(pharosClient.executePharosQuery).toHaveBeenCalledWith('web-mdb-search', 'all', 'custom-host', 1234);
    });

    it('test_should_slice_records_for_pagination_page_1', async () => {
        const mockRecords = Array.from({ length: 10 }, (_, i) => ({ id: i + 1, fields: [] }));
        const mockResponse: pharosClient.PharosResponse = { type: 'matches', count: 10, records: mockRecords };
        vi.mocked(pharosClient.executePharosQuery).mockResolvedValue(mockResponse);

        const result = await searchMdb('all', 1, 5);
        
        expect(result.records?.length).toBe(5);
        expect(result.records?.[0].id).toBe(1);
        expect(result.records?.[4].id).toBe(5);
        expect(result.count).toBe(10); // total remains 10
    });

    it('test_should_slice_records_for_pagination_page_2', async () => {
        const mockRecords = Array.from({ length: 10 }, (_, i) => ({ id: i + 1, fields: [] }));
        const mockResponse: pharosClient.PharosResponse = { type: 'matches', count: 10, records: mockRecords };
        vi.mocked(pharosClient.executePharosQuery).mockResolvedValue(mockResponse);

        const result = await searchMdb('all', 2, 5);
        
        expect(result.records?.length).toBe(5);
        expect(result.records?.[0].id).toBe(6);
        expect(result.records?.[4].id).toBe(10);
        expect(result.count).toBe(10);
    });

    it('test_should_return_empty_records_if_page_is_out_of_bounds', async () => {
        const mockRecords = Array.from({ length: 10 }, (_, i) => ({ id: i + 1, fields: [] }));
        const mockResponse: pharosClient.PharosResponse = { type: 'matches', count: 10, records: mockRecords };
        vi.mocked(pharosClient.executePharosQuery).mockResolvedValue(mockResponse);

        const result = await searchMdb('all', 3, 5);
        
        expect(result.records?.length).toBe(0);
        expect(result.count).toBe(10);
    });
});
