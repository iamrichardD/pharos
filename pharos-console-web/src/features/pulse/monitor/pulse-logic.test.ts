/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/features/pulse/monitor/pulse-logic.test.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Tests the pulse monitoring logic. Ensures that Pharos machine records
 * are correctly mapped to the MachineStatus interface using snake_case.
 * * Traceability:
 * Related to Snake-Case Harmonization Task.
 * ======================================================================== */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { getPulseStatus } from './pulse-logic';
import { executePharosQuery } from '../../../lib/pharos';

// Mock the pharos client
vi.mock('../../../lib/pharos', () => ({
    executePharosQuery: vi.fn()
}));

describe('pulse-logic', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('test_should_correctly_map_machine_records_to_snake_case_status_when_query_returns_matches', async () => {
        const mockRecords = [
            {
                id: 1,
                fields: [
                    { key: 'hostname', value: 'node-01' },
                    { key: 'status', value: 'online' },
                    { key: 'cpu_brand', value: 'Apple M2' },
                    { key: 'cpu_cores', value: '8' },
                    { key: 'mem_total_kb', value: '16777216' },
                    { key: 'os_name', value: 'macOS' },
                    { key: 'os_version', value: '13.4' },
                    { key: 'kernel_version', value: '22.5.0' },
                    { key: 'serial_number', value: 'ABC123XYZ' }
                ]
            }
        ];

        vi.mocked(executePharosQuery).mockResolvedValue({
            type: 'matches',
            count: 1,
            records: mockRecords
        });

        const status = await getPulseStatus();

        expect(status).toHaveLength(1);
        expect(status[0]).toEqual(expect.objectContaining({
            hostname: 'node-01',
            status: 'ONLINE',
            cpu_brand: 'Apple M2',
            cpu_cores: '8',
            mem_total_kb: '16777216',
            os_name: 'macOS',
            os_version: '13.4',
            kernel_version: '22.5.0',
            serial_number: 'ABC123XYZ'
        }));
        
        // Ensure last_seen is a Date
        expect(status[0].last_seen).toBeInstanceOf(Date);
    });

    it('test_should_return_empty_array_when_no_records_found', async () => {
        vi.mocked(executePharosQuery).mockResolvedValue({
            type: 'matches',
            count: 0,
            records: []
        });

        const status = await getPulseStatus();
        expect(status).toEqual([]);
    });

    it('test_should_handle_missing_optional_fields', async () => {
        const mockRecords = [
            {
                id: 2,
                fields: [
                    { key: 'hostname', value: 'node-minimal' },
                    { key: 'status', value: 'online' }
                ]
            }
        ];

        vi.mocked(executePharosQuery).mockResolvedValue({
            type: 'matches',
            count: 1,
            records: mockRecords
        });

        const status = await getPulseStatus();

        expect(status[0]).toEqual({
            name: 'node-minimal',
            hostname: 'node-minimal',
            status: 'ONLINE',
            last_seen: expect.any(Date),
            cpu_brand: undefined,
            cpu_cores: undefined,
            mem_total_kb: undefined,
            os_name: undefined,
            os_version: undefined,
            kernel_version: undefined,
            serial_number: undefined
        });
    });
});
