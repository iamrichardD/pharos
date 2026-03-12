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
 * Related to Task 16.6 (Issue #69) and Bug #122.
 * ======================================================================== */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { executePharosQuery } from './pharos';
import * as net from 'node:net';
import * as fs from 'node:fs';
import * as crypto from 'node:crypto';

vi.mock('node:fs', () => ({
    readFileSync: vi.fn(),
    existsSync: vi.fn(),
}));

vi.mock('node:crypto', () => ({
    sign: vi.fn(),
}));

const mockSocket = {
    write: vi.fn(),
    on: vi.fn(),
    destroy: vi.fn(),
    emit: vi.fn(),
};

vi.mock('node:net', () => ({
    connect: vi.fn(() => mockSocket),
    Socket: vi.fn(() => mockSocket),
}));

vi.mock('node:tls', () => ({
    connect: vi.fn(() => mockSocket),
}));

describe('executePharosQuery', () => {
    
    beforeEach(() => {
        vi.clearAllMocks();
        // Reset handlers for each test
        mockSocket.on.mockImplementation((event, handler) => {
            mockSocket[event + 'Handler'] = handler;
        });
    });

    it('test_should_return_matches_when_server_responds_with_data', async () => {
        const queryPromise = executePharosQuery('test-client', 'query all');
        
        await new Promise(resolve => setTimeout(resolve, 10));

        const dataHandler = (mockSocket as any).dataHandler;
        expect(dataHandler).toBeDefined();

        // 1. Receive banner
        dataHandler(Buffer.from('Pharos Protocol v1.0\n'));
        expect(mockSocket.write).toHaveBeenCalledWith('id test-client\r\n');

        // 2. Receive 200 ID OK
        dataHandler(Buffer.from('200:ID:Accepted\n'));
        expect(mockSocket.write).toHaveBeenCalledWith('query all\r\n');

        // 3. Receive 102 MATCHES
        dataHandler(Buffer.from('102:QUERY:Matches found: 1\n'));

        // 4. Receive data record
        dataHandler(Buffer.from('-200:1:hostname:node-01\n'));
        dataHandler(Buffer.from('-200:1:ip:192.168.1.1\n'));

        // 5. Receive 200 OK (end of response)
        dataHandler(Buffer.from('200:QUERY:Complete\n'));
        dataHandler(Buffer.from('\n'));

        const result = await queryPromise;
        
        expect(result.type).toBe('matches');
        expect(result.count).toBe(1);
        expect(result.records).toHaveLength(1);
        expect(result.records![0].fields).toContainEqual({ key: 'hostname', value: 'node-01' });
    });

    it('test_should_prioritize_environment_variables_over_defaults', async () => {
        vi.stubEnv('PHAROS_HOST', 'pharos-server-env');
        vi.stubEnv('PHAROS_PORT', '9999');

        // Trigger the call
        executePharosQuery('test-client', 'query test');
        
        expect(net.connect).toHaveBeenCalledWith(9999, 'pharos-server-env');

        vi.unstubAllEnvs();
    });

    it('test_should_use_default_host_and_port_if_env_is_missing', async () => {
        // Ensure env is clean
        vi.stubEnv('PHAROS_HOST', '');
        vi.stubEnv('PHAROS_PORT', '');

        executePharosQuery('test-client', 'query test');
        
        // Should use defaults from function signature: 127.0.0.1:2378
        expect(net.connect).toHaveBeenCalledWith(2378, '127.0.0.1');

        vi.unstubAllEnvs();
    });

    it('test_should_resolve_keys_from_files_during_authentication', async () => {
        vi.stubEnv('PHAROS_PRIVATE_KEY', '/path/to/private.key');
        vi.stubEnv('PHAROS_PUBLIC_KEY', '/path/to/public.pub');

        vi.mocked(fs.existsSync).mockReturnValue(true);
        vi.mocked(fs.readFileSync).mockImplementation((path: any) => {
            if (path === '/path/to/private.key') return '-----BEGIN PRIVATE KEY-----\nsecret\n-----END PRIVATE KEY-----';
            if (path === '/path/to/public.pub') return 'ssh-ed25519 AAAAC3...';
            return '';
        });
        vi.mocked(crypto.sign).mockReturnValue(Buffer.from('signed-challenge'));

        const queryPromise = executePharosQuery('test-client', 'add test=val');
        
        await new Promise(resolve => setTimeout(resolve, 10));
        const dataHandler = (mockSocket as any).dataHandler;

        // 1. Banner
        dataHandler(Buffer.from('Pharos Protocol v1.0\n'));
        // 2. ID OK
        dataHandler(Buffer.from('200:ID:Accepted\n'));
        // 3. Receive 401 Challenge
        dataHandler(Buffer.from('401:Authentication required. Challenge: abcdef123456\n'));

        expect(fs.readFileSync).toHaveBeenCalledWith('/path/to/private.key', 'utf8');
        expect(fs.readFileSync).toHaveBeenCalledWith('/path/to/public.pub', 'utf8');
        expect(crypto.sign).toHaveBeenCalled();
        expect(mockSocket.write).toHaveBeenCalledWith(expect.stringContaining('auth ssh-ed25519 AAAAC3...'));

        // 4. Auth OK
        dataHandler(Buffer.from('200:Auth:Accepted\n'));
        // 5. Query OK
        dataHandler(Buffer.from('200:Ok\n'));
        dataHandler(Buffer.from('\n'));

        const result = await queryPromise;
        expect(result.type).toBe('ok');

        vi.unstubAllEnvs();
    });
});
