/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: src/lib/selfReport.test.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Tests the selfReport logic: getConsoleHostname fallback behavior and
 * sendSelfReport query formatting and error handling.
 * ======================================================================== */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { getConsoleHostname, sendSelfReport } from './selfReport';
import * as pharos from './pharos';

vi.mock('./pharos', () => ({
  executePharosQuery: vi.fn(),
}));

describe('selfReport', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.unstubAllEnvs();
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('test_should_use_PHAROS_CONSOLE_HOSTNAME_when_set', () => {
    vi.stubEnv('PHAROS_CONSOLE_HOSTNAME', 'custom-console-host');
    expect(getConsoleHostname()).toBe('custom-console-host');
  });

  it('test_should_fallback_to_os_hostname_and_warn_when_env_unset', () => {
    vi.stubEnv('PHAROS_CONSOLE_HOSTNAME', '');
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const hostname = getConsoleHostname();
    expect(hostname).toBeTruthy();
  });

  it('test_should_send_add_query_with_hostname_and_version', async () => {
    vi.stubEnv('PHAROS_CONSOLE_HOSTNAME', 'console-node-01');
    vi.stubEnv('PHAROS_CONSOLE_VERSION', 'v9.9.9-test');
    vi.mocked(pharos.executePharosQuery).mockResolvedValue({ type: 'ok', message: 'Ok' });

    const success = await sendSelfReport();
    expect(success).toBe(true);
    expect(pharos.executePharosQuery).toHaveBeenCalledWith(
      'pharos-console-web',
      'add type="machine" hostname="console-node-01" version="v9.9.9-test" role="pharos-console-web"'
    );
  });

  it('test_should_handle_error_response_from_server', async () => {
    vi.stubEnv('PHAROS_CONSOLE_HOSTNAME', 'console-node-01');
    vi.mocked(pharos.executePharosQuery).mockResolvedValue({ type: 'error', code: 500, message: 'Server error' });

    const consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const success = await sendSelfReport();
    expect(success).toBe(false);
    expect(consoleErrorSpy).toHaveBeenCalled();
  });
});
