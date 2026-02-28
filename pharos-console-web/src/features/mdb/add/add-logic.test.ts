/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/features/mdb/add/add-logic.test.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Tests the MDB addition logic (Vertical Slice).
 * Ensures correct protocol command generation for various input sets.
 * * Traceability:
 * Related to Task 16.6 (Issue #69).
 * ======================================================================== */

import { describe, it, expect, vi } from 'vitest';
import { commitMdbRecord } from './add-logic';
import * as pharosClient from '../../../lib/pharos';

vi.mock('../../../lib/pharos', () => ({
    executePharosQuery: vi.fn(),
}));

describe('commitMdbRecord', () => {
    it('test_should_generate_minimal_add_command_when_only_required_fields_provided', async () => {
        const mockResponse: pharosClient.PharosResponse = { type: 'ok', message: 'Ok' };
        vi.mocked(pharosClient.executePharosQuery).mockResolvedValue(mockResponse);

        await commitMdbRecord({ hostname: 'node-01', ip: '1.2.3.4' });
        
        expect(pharosClient.executePharosQuery).toHaveBeenCalledWith(
            'web-console-add', 
            'add type="machine" hostname="node-01" ip="1.2.3.4"'
        );
    });

    it('test_should_generate_full_add_command_when_all_fields_provided', async () => {
        const mockResponse: pharosClient.PharosResponse = { type: 'ok', message: 'Ok' };
        vi.mocked(pharosClient.executePharosQuery).mockResolvedValue(mockResponse);

        await commitMdbRecord({ 
            hostname: 'node-01', 
            ip: '1.2.3.4', 
            mac: '00:11:22', 
            os: 'Ubuntu', 
            alias: 'primary' 
        });
        
        expect(pharosClient.executePharosQuery).toHaveBeenCalledWith(
            'web-console-add', 
            'add type="machine" hostname="node-01" ip="1.2.3.4" mac="00:11:22" os="Ubuntu" alias="primary"'
        );
    });

    it('test_should_escape_double_quotes_in_input_to_prevent_injection', async () => {
        const mockResponse: pharosClient.PharosResponse = { type: 'ok', message: 'Ok' };
        vi.mocked(pharosClient.executePharosQuery).mockResolvedValue(mockResponse);

        await commitMdbRecord({ 
            hostname: 'node" delete all "', 
            ip: '1.2.3.4'
        });
        
        expect(pharosClient.executePharosQuery).toHaveBeenCalledWith(
            'web-console-add', 
            'add type="machine" hostname="node\\" delete all \\"" ip="1.2.3.4"'
        );
    });
});
