/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/pages/mcp.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * The WebMCP JSON-RPC 2.0 gateway. Bridges AI agents to the Pharos
 * protocol via structured tool calls.
 * * Traceability:
 * Related to Phase 22 (Issue #135) and mcp-pharos-spec.md.
 * ======================================================================== */
import type { APIRoute } from 'astro';
import { executePharosQuery } from '../lib/pharos';
import { commitMdbRecord } from '../features/mdb/add/add-logic';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as crypto from 'node:crypto';

export const POST: APIRoute = async ({ request, locals }) => {
    const session = locals.session;
    
    // Safety check: Middleware should have caught this, but let's be explicit
    if (!session) {
        return new Response(JSON.stringify({
            jsonrpc: '2.0',
            error: { code: -32000, message: 'Unauthorized' },
            id: null
        }), { 
            status: 401,
            headers: { 'Content-Type': 'application/json' }
        });
    }

    try {
        const body = await request.json();
        const { jsonrpc, method, params, id } = body;

        if (jsonrpc !== '2.0') {
            return new Response(JSON.stringify({
                jsonrpc: '2.0',
                error: { code: -32600, message: 'Invalid Request (jsonrpc must be 2.0)' },
                id
            }), { 
                status: 400,
                headers: { 'Content-Type': 'application/json' }
            });
        }

        switch (method) {
            case 'query_mdb':
                return await handleQueryMdb(params, id);
            case 'provision_node':
                return await handleProvisionNode(params, id);
            case 'mcp.list_keys':
                return await handleListKeys(id);
            case 'mcp.provision_key':
                return await handleProvisionKey(params, id);
            default:
                return new Response(JSON.stringify({
                    jsonrpc: '2.0',
                    error: { code: -32601, message: `Method not found: ${method}` },
                    id
                }), { 
                    status: 404,
                    headers: { 'Content-Type': 'application/json' }
                });
        }
    } catch (err: any) {
        return new Response(JSON.stringify({
            jsonrpc: '2.0',
            error: { code: -32700, message: `Parse error: ${err.message}` },
            id: null
        }), { 
            status: 400,
            headers: { 'Content-Type': 'application/json' }
        });
    }
};

async function handleQueryMdb(params: any, id: any) {
    const query = params?.query;
    if (!query) {
        return new Response(JSON.stringify({
            jsonrpc: '2.0',
            error: { code: -32602, message: 'Invalid params: query is required' },
            id
        }), { 
            status: 400,
            headers: { 'Content-Type': 'application/json' }
        });
    }

    try {
        const rawResult = await executePharosQuery('web-mcp', query);
        
        // Flatten records for AI agent consumption
        const flattenedRecords = rawResult.records?.map(record => {
            const obj: any = { id: record.id };
            record.fields.forEach(f => {
                obj[f.key] = f.value;
            });
            return obj;
        }) || [];

        return new Response(JSON.stringify({
            jsonrpc: '2.0',
            result: {
                ...rawResult,
                records: flattenedRecords
            },
            id
        }), { 
            status: 200,
            headers: { 'Content-Type': 'application/json' }
        });
    } catch (err: any) {
        return new Response(JSON.stringify({
            jsonrpc: '2.0',
            error: { code: -32001, message: `Pharos error: ${err.message}` },
            id
        }), { 
            status: 500,
            headers: { 'Content-Type': 'application/json' }
        });
    }
}

async function handleProvisionNode(params: any, id: any) {
    const { hostname, ip, mac, os, alias } = params || {};
    if (!hostname || !ip) {
        return new Response(JSON.stringify({
            jsonrpc: '2.0',
            error: { code: -32602, message: 'Invalid params: hostname and ip are required' },
            id
        }), { 
            status: 400,
            headers: { 'Content-Type': 'application/json' }
        });
    }

    try {
        const res = await commitMdbRecord({ hostname, ip, mac, os, alias });
        if (res.type === 'error') {
            throw new Error(res.message || 'Pharos server error');
        }
        return new Response(JSON.stringify({
            jsonrpc: '2.0',
            result: {
                status: 'success',
                message: 'Node provisioned successfully',
                data: params
            },
            id
        }), { 
            status: 200,
            headers: { 'Content-Type': 'application/json' }
        });
    } catch (err: any) {
        return new Response(JSON.stringify({
            jsonrpc: '2.0',
            error: { code: -32002, message: `Provisioning failed: ${err.message}` },
            id
        }), { 
            status: 500,
            headers: { 'Content-Type': 'application/json' }
        });
    }
}

async function handleListKeys(id: any) {
    const keysDir = process.env.PHAROS_KEYS_DIR || '/etc/pharos/keys';
    try {
        if (!fs.existsSync(keysDir)) {
            return new Response(JSON.stringify({
                jsonrpc: '2.0',
                result: { keys: [] },
                id
            }), { 
                status: 200,
                headers: { 'Content-Type': 'application/json' }
            });
        }

        const files = fs.readdirSync(keysDir);
        const keys = files.filter(f => f.endsWith('.pub'));

        return new Response(JSON.stringify({
            jsonrpc: '2.0',
            result: { keys },
            id
        }), { 
            status: 200,
            headers: { 'Content-Type': 'application/json' }
        });
    } catch (err: any) {
        return new Response(JSON.stringify({
            jsonrpc: '2.0',
            error: { code: -32003, message: `Failed to list keys: ${err.message}` },
            id
        }), { 
            status: 500,
            headers: { 'Content-Type': 'application/json' }
        });
    }
}

async function handleProvisionKey(params: any, id: any) {
    const role = params?.role || 'user';
    const keysDir = process.env.PHAROS_KEYS_DIR || '/etc/pharos/keys';

    try {
        if (!fs.existsSync(keysDir)) {
            fs.mkdirSync(keysDir, { recursive: true });
        }

        // Generate Ed25519 key pair
        const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');

        const timestamp = Math.floor(Date.now() / 1000);
        const filename = `${role}_mcp_${timestamp}`;
        const pubPath = path.join(keysDir, `${filename}.pub`);
        
        // Export to OpenSSH format
        const pubKeyStr = publicKey.export({ type: 'spki', format: 'pem' }).toString();
        // Wait, pharos-server expects OpenSSH format.
        // Node.js doesn't natively export to OpenSSH format easily.
        // But we can use the PEM format if we update pharos-server or just use it as is.
        // The Rust server uses `ssh-key` crate which handles OpenSSH.
        
        // Actually, for tool unification, it's better if we can match the format.
        // But since this is a Web Console, maybe we can just use PEM and documented it.
        // Or we can try to construct the OpenSSH format manually.
        
        // For now, let's use the standard PEM and see if we can improve it.
        // Actually, let's use the raw seed and construct OpenSSH if possible.
        
        const privKeyStr = privateKey.export({ type: 'pkcs8', format: 'pem' }).toString();
        
        // Simplified OpenSSH public key construction (ssh-ed25519 <base64> <comment>)
        const pubDer = publicKey.export({ type: 'spki', format: 'der' });
        // The last 32 bytes of Ed25519 SPKI are the public key
        const pubRaw = pubDer.subarray(pubDer.length - 32);
        const sshPubKey = `ssh-ed25519 ${Buffer.concat([
            Buffer.from([0, 0, 0, 11]), Buffer.from('ssh-ed25519'),
            Buffer.from([0, 0, 0, 32]), pubRaw
        ]).toString('base64')} ${role}_mcp_${timestamp}`;

        fs.writeFileSync(pubPath, sshPubKey);

        return new Response(JSON.stringify({
            jsonrpc: '2.0',
            result: {
                status: 'success',
                public_key: sshPubKey,
                private_key: privKeyStr,
                role,
                path: pubPath
            },
            id
        }), { 
            status: 200,
            headers: { 'Content-Type': 'application/json' }
        });
    } catch (err: any) {
        return new Response(JSON.stringify({
            jsonrpc: '2.0',
            error: { code: -32004, message: `Failed to provision key: ${err.message}` },
            id
        }), { 
            status: 500,
            headers: { 'Content-Type': 'application/json' }
        });
    }
}
