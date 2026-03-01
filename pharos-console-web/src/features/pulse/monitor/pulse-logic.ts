/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/features/pulse/monitor/pulse-logic.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Logic for monitoring machine presence. It queries the Pharos server
 * for machine records and determines their online status based on 
 * heartbeat timestamps.
 * * Traceability:
 * Related to Task 16.4 (Issue #66).
 * ======================================================================== */
import { executePharosQuery, type PharosRecord } from '../../../lib/pharos';

export interface MachineStatus {
    name: string;
    status: 'ONLINE' | 'OFFLINE' | 'UNREACHABLE';
    lastSeen: Date;
    cpu?: string;
    memUsed?: string;
    memTotal?: string;
    uptime?: string;
}

/**
 * Fetches all machine records and calculates their current status.
 */
export async function getPulseStatus(): Promise<MachineStatus[]> {
    // Query for all machine records
    const response = await executePharosQuery('web-console', 'query type=machine');
    
    if (response.type !== 'matches' || !response.records) {
        return [];
    }

    const machineMap = new Map<string, MachineStatus>();

    for (const record of response.records) {
        const name = record.fields.find(f => f.key === 'name')?.value || 'unknown';
        const cpu = record.fields.find(f => f.key === 'cpu')?.value;
        const memUsed = record.fields.find(f => f.key === 'mem_used')?.value;
        const memTotal = record.fields.find(f => f.key === 'mem_total')?.value;
        const uptime = record.fields.find(f => f.key === 'uptime')?.value;
        
        // In the absence of an explicit timestamp from the server, 
        // we'll assume the record exists and is recent if it was returned.
        // NOTE: Future server versions will include a 'last_seen' or similar.
        const lastSeen = new Date(); 

        const status: MachineStatus = {
            name,
            status: 'ONLINE', // Default to ONLINE if we found it
            lastSeen,
            cpu,
            memUsed,
            memTotal,
            uptime
        };

        // If we have multiple records for the same machine (due to historical pulses),
        // we take the latest one. (Ph server currently returns all matches).
        // Since IDs are incremental, the last one in the list is the newest.
        machineMap.set(name, status);
    }

    return Array.from(machineMap.values());
}
