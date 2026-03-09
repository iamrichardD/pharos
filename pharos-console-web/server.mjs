/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: pharos-console-web/server.mjs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Mandatory HTTPS entry point for the Pharos Sandbox Web Console. 
 * Enables direct TLS handling without an external proxy, ensuring 
 * end-to-end encryption in isolated environments.
 * ======================================================================== */

import https from 'node:https';
import fs from 'node:fs';
import express from 'express';
import { handler as ssrHandler } from './dist/server/entry.mjs';

const PORT = process.env.PORT || 3000;
const CERT_PATH = process.env.PHAROS_TLS_CERT;
const KEY_PATH = process.env.PHAROS_TLS_KEY;

if (!CERT_PATH || !KEY_PATH) {
  console.error('ERROR: PHAROS_TLS_CERT and PHAROS_TLS_KEY must be provided for mandatory HTTPS.');
  process.exit(1);
}

try {
  const options = {
    cert: fs.readFileSync(CERT_PATH),
    key: fs.readFileSync(KEY_PATH),
  };

  const app = express();
  app.use(express.static('dist/client'));
  app.use(ssrHandler);

  const server = https.createServer(options, app);

  server.listen(PORT, '0.0.0.0', () => {
    console.log(`Pharos Web Console running on https://0.0.0.0:${PORT} (Direct HTTPS)`);
  });
} catch (error) {
  console.error('Failed to start HTTPS server:', error);
  process.exit(1);
}
