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
    lastSeen: Date;
    cpuBrand?: string;
    cpuCores?: string;
    memTotalKb?: string;
    osName?: string;
    osVersion?: string;
    kernelVersion?: string;
    serialNumber?: string;
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
        
        const cpuBrand = record.fields.find(f => f.key === 'cpu_brand')?.value;
        const cpuCores = record.fields.find(f => f.key === 'cpu_cores')?.value;
        const memTotalKb = record.fields.find(f => f.key === 'mem_total_kb')?.value;
        const osName = record.fields.find(f => f.key === 'os_name')?.value;
        const osVersion = record.fields.find(f => f.key === 'os_version')?.value;
        const kernelVersion = record.fields.find(f => f.key === 'kernel_version')?.value;
        const serialNumber = record.fields.find(f => f.key === 'serial_number')?.value;
        
        // In the absence of an explicit timestamp from the server, 
        // we'll assume the record exists and is recent if it was returned.
        const lastSeen = new Date(); 

        const status: MachineStatus = {
            name,
            hostname,
            status: statusField.toUpperCase() === 'OFFLINE' ? 'OFFLINE' : 'ONLINE',
            lastSeen,
            cpuBrand,
            cpuCores,
            memTotalKb,
            osName,
            osVersion,
            kernelVersion,
            serialNumber
        };

        machineMap.set(hostname, status);
    }

    return Array.from(machineMap.values());
}
