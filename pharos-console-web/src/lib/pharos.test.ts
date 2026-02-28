/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/lib/pharos.test.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Tests the Pharos protocol client logic (executePharosQuery).
 * Ensures correct parsing of protocol codes (102, 200, 4xx, etc.).
 * * Traceability:
 * Related to Task 16.6 (Issue #69).
 * ======================================================================== */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { executePharosQuery } from './pharos';
import * as net from 'node:net';

const mockSocket = {
    connect: vi.fn(),
    write: vi.fn(),
    on: vi.fn(),
    destroy: vi.fn(),
};

// Mock node:net
vi.mock('node:net', () => {
    return { Socket: vi.fn(() => mockSocket) };
});

describe('executePharosQuery', () => {
    
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('test_should_return_matches_when_server_responds_with_data', async () => {
        // Simulate event listeners
        const handlers: any = {};
        mockSocket.on.mockImplementation((event: string, handler: Function) => {
            handlers[event] = handler;
        });

        const queryPromise = executePharosQuery('test-client', 'query all');
        
        // Wait a bit for the promise to set up listeners
        await new Promise(resolve => setTimeout(resolve, 10));

        // 1. Receive banner
        handlers['data'](Buffer.from('Pharos Protocol v1.0\r\n'));
        
        // Expect 'id test-client\r\n' to be written
        expect(mockSocket.write).toHaveBeenCalledWith('id test-client\r\n');

        // 2. Receive 200 ID OK
        handlers['data'](Buffer.from('200:ID:Accepted\r\n'));
        
        // Expect 'query all\r\n' to be written
        expect(mockSocket.write).toHaveBeenCalledWith('query all\r\n');

        // 3. Receive 102 MATCHES
        handlers['data'](Buffer.from('102:QUERY:Matches found: 1\r\n'));

        // 4. Receive data record
        handlers['data'](Buffer.from('-200:1:hostname:node-01\r\n'));
        handlers['data'](Buffer.from('-200:1:ip:192.168.1.1\r\n'));

        // 5. Receive 200 OK (end of response)
        handlers['data'](Buffer.from('200:QUERY:Complete\r\n'));
        
        // End of response (empty line)
        handlers['data'](Buffer.from('\r\n'));

        const result = await queryPromise;
        
        expect(result.type).toBe('matches');
        expect(result.count).toBe(1);
        expect(result.records).toHaveLength(1);
        expect(result.records![0].fields).toContainEqual({ key: 'hostname', value: 'node-01' });
        expect(result.records![0].fields).toContainEqual({ key: 'ip', value: '192.168.1.1' });
    });

    it('test_should_return_error_when_server_responds_with_error_code', async () => {
        const handlers: any = {};
        mockSocket.on.mockImplementation((event: string, handler: Function) => {
            handlers[event] = handler;
        });

        const queryPromise = executePharosQuery('test-client', 'query fail');
        
        await new Promise(resolve => setTimeout(resolve, 10));

        handlers['data'](Buffer.from('Pharos Protocol v1.0\r\n'));
        handlers['data'](Buffer.from('200:ID:Accepted\r\n'));
        handlers['data'](Buffer.from('404:QUERY:Record not found\r\n'));

        const result = await queryPromise;
        
        expect(result.type).toBe('error');
        expect(result.code).toBe(404);
        expect(result.message).toBe('QUERY:Record not found');
    });
});
