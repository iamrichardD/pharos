/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/lib/pharos.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * A lightweight Node.js TCP client for the Pharos protocol. It connects 
 * directly to the backend server (typically port 1050) to execute queries
 * on behalf of the web interface (SSR).
 * * Traceability:
 * Related to Task 16.2 (Mobile-First MDB).
 * ======================================================================== */
import * as net from 'node:net';

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

export async function executePharosQuery(clientId: string, queryStr: string, host = '127.0.0.1', port = 1050): Promise<PharosResponse> {
    return new Promise((resolve, reject) => {
        const client = new net.Socket();
        
        let buffer = '';
        let stage = 'banner';
        
        let currentRecord: PharosRecord | null = null;
        const records: PharosRecord[] = [];
        let matchCount = 0;
        
        const cleanup = () => {
            client.destroy();
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
                // Always prepend 'query ' if not present for basic queries, but for simplicity here we just send the command.
                let cmd = queryStr.trim();
                if (!cmd.startsWith('query ') && !cmd.startsWith('ph ') && !cmd.startsWith('add ') && !cmd.startsWith('change ') && !cmd.startsWith('delete ')) {
                    cmd = `query ${cmd}`;
                }
                client.write(`${cmd}\r\n`);
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

        client.on('data', (data) => {
            buffer += data.toString();
            let newlineIdx;
            while ((newlineIdx = buffer.indexOf('\n')) !== -1) {
                const line = buffer.substring(0, newlineIdx).replace('\r', '');
                buffer = buffer.substring(newlineIdx + 1);
                onLine(line);
            }
        });

        client.on('error', (err) => {
            cleanup();
            reject(err);
        });

        const hostEnv = process.env.PHAROS_HOST || host;
        const portEnv = parseInt(process.env.PHAROS_PORT || '', 10) || port;

        client.connect(portEnv, hostEnv);
    });
}
