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

 export async function executePharosQuery(clientId: string, queryStr: string, host?: string, port?: number): Promise<PharosResponse> {
    const hostEnv = host || process.env.PHAROS_HOST || '127.0.0.1';
    const portEnv = port || parseInt(process.env.PHAROS_PORT || '2378', 10);
    
    // Diagnostic logging for Bug #122 - will appear in container logs
    if (process.env.NODE_ENV !== 'test') {
        console.log(`[PharosClient] Connecting to ${hostEnv}:${portEnv} (Query: ${queryStr.substring(0, 20)}...)`);
    }

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

                const parts = line.split(':');
                if (parts.length < 2) return;
                
                const codeStr = parts[0];
                const code = parseInt(codeStr, 10);
                const message = parts.slice(1).join(':').trim();

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
                    // 401:Authentication required. Challenge: <hex>
                    const challengeMatch = message.match(/Challenge:\s*([0-9a-fA-F]+)/);
                    const privKeyEnv = process.env.PHAROS_PRIVATE_KEY;
                    const pubKeyEnv = process.env.PHAROS_PUBLIC_KEY;

                    if (challengeMatch && privKeyEnv && pubKeyEnv) {
                        try {
                            const challenge = challengeMatch[1];
                            const privateKey = privKeyEnv.includes('PRIVATE KEY') 
                                ? privKeyEnv 
                                : Buffer.from(privKeyEnv, 'base64').toString();
                            
                            const signature = crypto.sign(null, Buffer.from(challenge), privateKey);
                            const signatureBase64 = signature.toString('base64');
                            
                            stage = 'auth';
                            client.write(`auth ${pubKeyEnv} ${signatureBase64}\r\n`);
                            return;
                        } catch (err: any) {
                            cleanup();
                            resolve({ type: 'error', code: 500, message: `Signing failed: ${err.message}` });
                            return;
                        }
                    }
                    
                    cleanup();
                    resolve({ type: 'error', code: 401, message: 'Authentication required but no keys provided' });
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
