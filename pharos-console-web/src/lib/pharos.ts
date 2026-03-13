/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/lib/pharos.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * A lightweight Node.js TCP client for the Pharos protocol. It connects 
 * directly to the backend server (typically port 2378) to execute queries
 * and supports automated SSH-challenge signing for authentication.
 * * Traceability:
 * Related to Task 16.2 (Mobile-First MDB) and Debt #01 (Issue #83).
 * ======================================================================== */
 import * as net from 'node:net';
 import * as tls from 'node:tls';
 import * as fs from 'node:fs';
 import * as crypto from 'node:crypto';

 export interface PharosRecord {
    id: number;
    fields: { key: string; value: string }[];
 }

 export interface PharosResponse {
    type: 'ok' | 'matches' | 'error';
    message?: string;
    records?: PharosRecord[];
    count?: number;
    code?: number;
 }

 /**
  * Formats a PharosRecord into a human-readable string matching the 'mdb' CLI style.
  * * Rationale: Ensures consistent UX between the CLI and the Web Sandbox Terminal.
  * * Style: "{:>15}: {}" (15-character right-aligned keys).
  */
 export function formatPharosRecord(record: PharosRecord): string {
    return record.fields
        .map(field => `${field.key.padStart(15, ' ')}: ${field.value}`)
        .join('\n');
 }

 export async function executePharosQuery(clientId: string, queryStr: string, host?: string, port?: number): Promise<PharosResponse> {
    const hostEnv = host || process.env.PHAROS_HOST || '127.0.0.1';
    const portEnv = port || parseInt(process.env.PHAROS_PORT || '2378', 10);
    
    return new Promise((resolve, reject) => {
        const useTls = !!process.env.PHAROS_CA_CERT || !!process.env.PHAROS_TLS_CERT || process.env.PHAROS_SANDBOX === 'true';

        let client: net.Socket;
        if (useTls) {
            client = tls.connect(portEnv, hostEnv, {
                ca: process.env.PHAROS_CA_CERT ? fs.readFileSync(process.env.PHAROS_CA_CERT) : undefined,
                rejectUnauthorized: !!process.env.PHAROS_CA_CERT
            });
        } else {
            client = net.connect(portEnv, hostEnv);
        }
        
        let buffer = '';
        let stage = 'banner';
        
        let currentRecord: PharosRecord | null = null;
        const records: PharosRecord[] = [];
        let matchCount = 0;
        
        const cleanup = () => {
            client.destroy();
        };

        const sendQuery = () => {
            // Prepend 'query ' if not a recognized top-level command
            let cmd = queryStr.trim();
            const lowerCmd = cmd.toLowerCase();
            const topLevelCommands = ['query', 'ph', 'add', 'change', 'delete', 'status', 'siteinfo', 'help', 'id', 'auth', 'quit'];
            
            const isTopLevel = topLevelCommands.some(c => lowerCmd === c || lowerCmd.startsWith(c + ' '));
            
            if (!isTopLevel) {
                cmd = `query ${cmd}`;
            }
            client.write(`${cmd}\r\n`);
        };

        const onLine = (line: string) => {
            if (stage === 'banner') {
                stage = 'id';
                client.write(`id ${clientId}\r\n`);
                return;
            }
            if (stage === 'id') {
                if (!line.startsWith('200')) {
                    cleanup();
                    resolve({ type: 'error', code: parseInt(line.split(':')[0]) || 500, message: `ID rejected: ${line}` });
                    return;
                }
                stage = 'query';
                sendQuery();
                return;
            }

            if (stage === 'login') {
                if (line.startsWith('301')) {
                    // 301:<challenge>
                    const challenge = line.split(':')[1]?.trim();
                    
                    let privKey = process.env.PHAROS_PRIVATE_KEY;
                    let pubKey = process.env.PHAROS_PUBLIC_KEY;

                    // Resolve from files if paths provided
                    if (privKey && fs.existsSync(privKey)) {
                        privKey = fs.readFileSync(privKey, 'utf8');
                    }
                    if (pubKey && fs.existsSync(pubKey)) {
                        pubKey = fs.readFileSync(pubKey, 'utf8');
                    }

                    if (challenge && privKey && pubKey) {
                        try {
                            const privateKey = privKey.includes('PRIVATE KEY') 
                                ? privKey 
                                : Buffer.from(privKey, 'base64').toString();
                            
                            const signature = crypto.sign(null, Buffer.from(challenge), privateKey);
                            const signatureBase64 = signature.toString('base64');
                            
                            stage = 'auth';
                            // Wrap public key in quotes to handle potential spaces/comments
                            client.write(`auth "${pubKey.trim()}" "${signatureBase64}"\r\n`);
                            return;
                        } catch (err: any) {
                            cleanup();
                            resolve({ type: 'error', code: 500, message: `Signing failed: ${err.message}` });
                            return;
                        }
                    }
                    cleanup();
                    resolve({ type: 'error', code: 401, message: 'Authentication required but keys/challenge missing' });
                } else {
                    cleanup();
                    resolve({ type: 'error', code: 403, message: `Login failed: ${line}` });
                }
                return;
            }

            if (stage === 'auth') {
                if (line.startsWith('200')) {
                    stage = 'query';
                    sendQuery();
                } else {
                    cleanup();
                    resolve({ type: 'error', code: 403, message: `Authentication failed: ${line}` });
                }
                return;
            }

            if (stage === 'query') {
                const parts = line.split(':');
                const code = parseInt(parts[0], 10);
                const message = parts.slice(1).join(':').trim();

                if (line === '') {
                    // end of response
                    if (currentRecord) {
                        records.push(currentRecord);
                        currentRecord = null;
                    }
                    cleanup();
                    if (matchCount > 0 || records.length > 0) {
                        resolve({ type: 'matches', count: matchCount, records });
                    } else {
                        resolve({ type: 'ok', message: 'Ok' });
                    }
                    return;
                }

                if (code === 200) {
                    if (currentRecord) {
                        records.push(currentRecord);
                        currentRecord = null;
                    }
                    cleanup();
                    if (matchCount > 0 || records.length > 0) {
                        resolve({ type: 'matches', count: matchCount, records });
                    } else {
                        resolve({ type: 'ok', message });
                    }
                } else if (code === 401) {
                    // 401:Authentication required.
                    // Start handshake: send login
                    stage = 'login';
                    client.write(`login ${clientId}\r\n`);
                } else if (code === 102) {
                    const matchParts = message.split(/\s+/);
                    if (matchParts.length >= 3) {
                        matchCount = parseInt(matchParts[2], 10) || 0;
                    }
                } else if (code >= 400) {
                    cleanup();
                    resolve({ type: 'error', code, message });
                } else if (code < 0) {
                    // -200:ID:FIELD:VALUE
                    const dataParts = message.split(':');
                    if (dataParts.length >= 3) {
                        const id = parseInt(dataParts[0], 10) || 0;
                        const field = dataParts[1];
                        const value = dataParts.slice(2).join(':').trim();

                        if (currentRecord) {
                            if (currentRecord.id !== id) {
                                records.push(currentRecord);
                                currentRecord = { id, fields: [{ key: field, value }] };
                            } else {
                                currentRecord.fields.push({ key: field, value });
                            }
                        } else {
                            currentRecord = { id, fields: [{ key: field, value }] };
                        }
                    }
                }
            }
        };

        client.on('data', (data: Buffer) => {
            buffer += data.toString();
            let newlineIdx;
            while ((newlineIdx = buffer.indexOf('\n')) !== -1) {
                const line = buffer.substring(0, newlineIdx).replace('\r', '');
                buffer = buffer.substring(newlineIdx + 1);
                onLine(line);
            }
        });

        client.on('error', (err: Error) => {
            cleanup();
            reject(err);
        });

    });
}

/**
 * Executes a stateless authentication check against the Pharos server.
 */
export async function executeAuthCheck(publicKey: string, signature: string, challenge: string, hostParam?: string, portParam?: number): Promise<boolean> {
    const host = hostParam || process.env.PHAROS_HOST || '127.0.0.1';
    const port = portParam || parseInt(process.env.PHAROS_PORT || '2378', 10);
    const useTls = !!process.env.PHAROS_CA_CERT || !!process.env.PHAROS_TLS_CERT || process.env.PHAROS_SANDBOX === 'true';

    return new Promise((resolve, reject) => {
        let client: net.Socket;
        if (useTls) {
            client = tls.connect(port, host, {
                ca: process.env.PHAROS_CA_CERT ? fs.readFileSync(process.env.PHAROS_CA_CERT) : undefined,
                rejectUnauthorized: !!process.env.PHAROS_CA_CERT
            });
        } else {
            client = net.connect(port, host);
        }
        
        let buffer = '';
        let stage = 'banner';

        const cleanup = () => {
            client.destroy();
        };

        const onLine = (line: string) => {
            if (stage === 'banner') {
                stage = 'id';
                client.write('id web-console-auth\r\n');
                return;
            }
            if (stage === 'id') {
                if (line.startsWith('200')) {
                    stage = 'auth-check';
                    client.write(`auth-check "${publicKey}" "${signature}" "${challenge}"\r\n`);
                } else {
                    cleanup();
                    resolve(false);
                }
                return;
            }
            if (stage === 'auth-check') {
                cleanup();
                resolve(line.startsWith('200'));
                return;
            }
        };

        client.on('data', (data: Buffer) => {
            buffer += data.toString();
            let newlineIdx;
            while ((newlineIdx = buffer.indexOf('\n')) !== -1) {
                const line = buffer.substring(0, newlineIdx).replace('\r', '');
                buffer = buffer.substring(newlineIdx + 1);
                onLine(line);
            }
        });

        client.on('error', (err: Error) => {
            cleanup();
            reject(err);
        });

        client.on('close', () => {
            resolve(false);
        });

        // Timeout after 5 seconds
        setTimeout(() => {
            cleanup();
            resolve(false);
        }, 5000);
    });
}
