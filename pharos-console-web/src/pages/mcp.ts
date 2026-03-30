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

/**
 * JSON-RPC 2.0 Error Codes
 */
const RPC_ERRORS = {
    PARSE_ERROR: -32700,
    INVALID_REQUEST: -32600,
    METHOD_NOT_FOUND: -32601,
    INVALID_PARAMS: -32602,
    INTERNAL_ERROR: -32603,
    UNAUTHORIZED: -32000,
    PHAROS_ERROR: -32001,
    PROVISIONING_FAILED: -32002,
    KEY_MANAGEMENT_FAILED: -32003,
};

function errorResponse(code: number, message: string, id: any = null, status: number = 400) {
    return new Response(JSON.stringify({
        jsonrpc: '2.0',
        error: { code, message },
        id
    }), { 
        status,
        headers: { 'Content-Type': 'application/json' }
    });
}

export const POST: APIRoute = async ({ request, locals }) => {
    const session = locals.session;
    
    let body: any;
    try {
        body = await request.json();
    } catch (err: any) {
        return errorResponse(RPC_ERRORS.PARSE_ERROR, `Parse error: ${err.message}`, null, 400);
    }

    const { jsonrpc, method, params, id } = body;

    if (jsonrpc !== '2.0') {
        return errorResponse(RPC_ERRORS.INVALID_REQUEST, 'Invalid Request (jsonrpc must be 2.0)', id, 400);
    }

    if (!method || typeof method !== 'string') {
        return errorResponse(RPC_ERRORS.INVALID_REQUEST, 'Invalid Request (method must be a string)', id, 400);
    }

    try {
        switch (method) {
            case 'query_mdb':
                return await handleQueryMdb(params, id);
            case 'provision_node':
            case 'mcp.list_keys':
            case 'mcp.provision_key':
                // Mutation/Management methods require authentication
                if (!session) {
                    return errorResponse(RPC_ERRORS.UNAUTHORIZED, 'Unauthorized (Authentication required for this method)', id, 401);
                }
                
                if (method === 'provision_node') return await handleProvisionNode(params, id);
                if (method === 'mcp.list_keys') return await handleListKeys(id);
                return await handleProvisionKey(params, id);
            default:
                return errorResponse(RPC_ERRORS.METHOD_NOT_FOUND, `Method not found: ${method}`, id, 404);
        }
    } catch (err: any) {
        return errorResponse(RPC_ERRORS.INTERNAL_ERROR, `Internal error: ${err.message}`, id, 500);
    }
};

async function handleQueryMdb(params: any, id: any) {
    const query = params?.query;
    if (!query) {
        return errorResponse(RPC_ERRORS.INVALID_PARAMS, 'Invalid params: query is required', id, 400);
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
        return errorResponse(RPC_ERRORS.PHAROS_ERROR, `Pharos error: ${err.message}`, id, 500);
    }
}

async function handleProvisionNode(params: any, id: any) {
    const { hostname, ip, mac, os_name, alias } = params || {};
    if (!hostname || !ip) {
        return errorResponse(RPC_ERRORS.INVALID_PARAMS, 'Invalid params: hostname and ip are required', id, 400);
    }

    try {
        const res = await commitMdbRecord({ hostname, ip, mac, os_name, alias });
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
        return errorResponse(RPC_ERRORS.PROVISIONING_FAILED, `Provisioning failed: ${err.message}`, id, 500);
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
        return errorResponse(RPC_ERRORS.KEY_MANAGEMENT_FAILED, `Failed to list keys: ${err.message}`, id, 500);
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
        
        // Simplified OpenSSH public key construction (ssh-ed25519 <base64> <comment>)
        const pubDer = publicKey.export({ type: 'spki', format: 'der' });
        // The last 32 bytes of Ed25519 SPKI are the public key
        const pubRaw = pubDer.subarray(pubDer.length - 32);
        const sshPubKey = `ssh-ed25519 ${Buffer.concat([
            Buffer.from([0, 0, 0, 11]), Buffer.from('ssh-ed25519'),
            Buffer.from([0, 0, 0, 32]), pubRaw
        ]).toString('base64')} ${role}_mcp_${timestamp}`;

        fs.writeFileSync(pubPath, sshPubKey);
        const privKeyStr = privateKey.export({ type: 'pkcs8', format: 'pem' }).toString();

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
        return errorResponse(RPC_ERRORS.KEY_MANAGEMENT_FAILED, `Failed to provision key: ${err.message}`, id, 500);
    }
}
