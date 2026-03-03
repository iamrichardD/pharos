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
        handlers['data'](Buffer.from('Pharos Protocol v1.0\n'));
        
        // Expect 'id test-client\r\n' to be written
        expect(mockSocket.write).toHaveBeenCalledWith('id test-client\r\n');

        // 2. Receive 200 ID OK
        handlers['data'](Buffer.from('200:ID:Accepted\n'));
        
        // Expect 'query all\r\n' to be written
        expect(mockSocket.write).toHaveBeenCalledWith('query all\r\n');

        // 3. Receive 102 MATCHES
        handlers['data'](Buffer.from('102:QUERY:Matches found: 1\n'));

        // 4. Receive data record
        handlers['data'](Buffer.from('-200:1:hostname:node-01\n'));
        handlers['data'](Buffer.from('-200:1:ip:192.168.1.1\n'));

        // 5. Receive 200 OK (end of response)
        handlers['data'](Buffer.from('200:QUERY:Complete\n'));
        
        // End of response (empty line)
        handlers['data'](Buffer.from('\n'));

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

        handlers['data'](Buffer.from('Pharos Protocol v1.0\n'));
        handlers['data'](Buffer.from('200:ID:Accepted\n'));
        handlers['data'](Buffer.from('404:QUERY:Record not found\n'));

        const result = await queryPromise;
        
        expect(result.type).toBe('error');
        expect(result.code).toBe(404);
        expect(result.message).toBe('QUERY:Record not found');
    });

    it('test_should_automatically_sign_and_resend_when_challenged', async () => {
        const handlers: any = {};
        const writeHistory: string[] = [];
        
        mockSocket.on.mockImplementation((event: string, handler: Function) => {
            handlers[event] = handler;
        });

        mockSocket.write.mockImplementation((data: string) => {
            writeHistory.push(data);
        });

        // Use a real Ed25519 PEM key for node:crypto compatibility
        const testPrivKey = `-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEINAWgXDHidaozC45Z1P4DpUoi4WvDjRvexfn+vrCQ/KZ
-----END PRIVATE KEY-----`;
        const testPubKey = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGYPkCSEQYwmkd+V190X/L8b4H9lhjkPDO0ClTwEyzOz test";

        vi.stubEnv('PHAROS_PRIVATE_KEY', testPrivKey);
        vi.stubEnv('PHAROS_PUBLIC_KEY', testPubKey);

        const queryPromise = executePharosQuery('test-client', 'add name=test');
        
        await new Promise(resolve => setTimeout(resolve, 10));

        // 1. Banner
        handlers['data'](Buffer.from('Pharos Protocol v1.0\n'));
        // 2. ID Accepted
        handlers['data'](Buffer.from('200:ID:Accepted\n'));
        
        expect(writeHistory).toContain('add name=test\r\n');

        // 3. Challenged
        handlers['data'](Buffer.from('401:Authentication required. Challenge: deadbeef\n'));
        
        // Check if auth was sent
        const authCmd = writeHistory.find(w => w.startsWith('auth ssh-ed25519'));
        expect(authCmd).toBeDefined();

        // 4. Auth Accepted
        handlers['data'](Buffer.from('200:AUTH:Accepted\n'));
        
        // Original command should be resent (it should appear twice in history)
        const addCommands = writeHistory.filter(w => w === 'add name=test\r\n');
        expect(addCommands).toHaveLength(2);

        // 5. Final OK
        handlers['data'](Buffer.from('200:ADD:Complete\n'));
        handlers['data'](Buffer.from('\n'));

        const result = await queryPromise;
        expect(result.type).toBe('ok');
        
        vi.unstubAllEnvs();
    });
});
