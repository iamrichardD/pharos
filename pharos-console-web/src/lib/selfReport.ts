/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/lib/selfReport.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Periodically self-reports the Web Console's hostname, version, and role
 * to pharos-server so that the hub can track node presence and alert on
 * version drift against expected_version.
 * ======================================================================== */

import * as os from 'node:os';
import { executePharosQuery } from './pharos.ts';

let warningLogged = false;

/**
 * Resolves the console hostname, prioritizing PHAROS_CONSOLE_HOSTNAME,
 * falling back to os.hostname() with a one-time startup warning.
 */
export function getConsoleHostname(): string {
  const envHost = process.env.PHAROS_CONSOLE_HOSTNAME;
  if (envHost && envHost.trim().length > 0) {
    return envHost.trim();
  }

  const fallback = `${os.hostname()}-console`;
  if (!warningLogged) {
    console.warn(
      `PHAROS_CONSOLE_HOSTNAME env var not set; falling back to container hostname "${fallback}" (appended -console suffix). This may be an opaque container ID unless explicitly set.`
    );
    warningLogged = true;
  }
  return fallback;
}

/**
 * Sends a single self-registration add command to pharos-server.
 */
export async function sendSelfReport(): Promise<boolean> {
  const hostname = getConsoleHostname();
  const version = process.env.PHAROS_CONSOLE_VERSION || 'dev';

  const esc = (s: string) => s.replace(/"/g, '\\"');
  const queryStr = `add type="machine" hostname="${esc(hostname)}" version="${esc(version)}" role="pharos-console-web"`;

  try {
    const res = await executePharosQuery('pharos-console-web', queryStr);
    if (res.type === 'error') {
      console.error(`Console self-registration failed: ${res.message} (code ${res.code})`);
      return false;
    }
    console.log(`Console self-registration successful for hostname "${hostname}" (version: ${version})`);
    return true;
  } catch (err: any) {
    console.error(`Console self-registration connection error: ${err.message || err}`);
    return false;
  }
}

/**
 * Starts the self-reporting lifecycle: attempts initial registration with backoff retries,
 * then registers an interval timer for periodic heartbeats (default 60 minutes).
 */
export async function startSelfReporting(intervalMs = 60 * 60 * 1000): Promise<void> {
  const maxRetries = 5;
  let delay = 2000;
  let success = false;

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    success = await sendSelfReport();
    if (success) break;

    if (attempt < maxRetries) {
      console.log(`Retrying console self-registration in ${delay / 1000}s (attempt ${attempt}/${maxRetries})...`);
      await new Promise(resolve => setTimeout(resolve, delay));
      delay *= 2;
    }
  }

  if (!success) {
    console.warn('Initial console self-registration attempts exhausted; background interval timer will continue retrying.');
  }

  setInterval(async () => {
    await sendSelfReport();
  }, intervalMs);
}
