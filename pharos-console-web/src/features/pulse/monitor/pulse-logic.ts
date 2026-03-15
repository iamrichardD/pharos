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
import { executePharosQuery } from '../../../lib/pharos';

export interface MachineStatus {
    name: string;
    hostname: string;
    status: 'ONLINE' | 'OFFLINE' | 'UNREACHABLE';
    last_seen_at?: string;
    cpu_brand?: string;
    cpu_cores?: string;
    mem_total_kb?: string;
    os_name?: string;
    os_version?: string;
    kernel_version?: string;
    serial_number?: string;
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
        const name = record.fields.find(f => f.key === 'name')?.value || 
                     record.fields.find(f => f.key === 'hostname')?.value || 
                     'unknown';
        const hostname = record.fields.find(f => f.key === 'hostname')?.value || name;
        const statusField = record.fields.find(f => f.key === 'status')?.value || 'online';
        
        const cpu_brand = record.fields.find(f => f.key === 'cpu_brand')?.value;
        const cpu_cores = record.fields.find(f => f.key === 'cpu_cores')?.value;
        const mem_total_kb = record.fields.find(f => f.key === 'mem_total_kb')?.value;
        const os_name = record.fields.find(f => f.key === 'os_name')?.value;
        const os_version = record.fields.find(f => f.key === 'os_version')?.value;
        const kernel_version = record.fields.find(f => f.key === 'kernel_version')?.value;
        const serial_number = record.fields.find(f => f.key === 'serial_number')?.value;
        
        const last_seen_at = record.fields.find(f => f.key === 'last_seen_at')?.value;

        const status: MachineStatus = {
            name,
            hostname,
            status: statusField.toUpperCase() === 'OFFLINE' ? 'OFFLINE' : 'ONLINE',
            last_seen_at,
            cpu_brand,
            cpu_cores,
            mem_total_kb,
            os_name,
            os_version,
            kernel_version,
            serial_number
        };

        machineMap.set(hostname, status);
    }

    return Array.from(machineMap.values());
}
