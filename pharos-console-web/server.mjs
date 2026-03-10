/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: pharos-console-web/server.mjs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Dual-protocol entry point for the Pharos Sandbox Web Console. 
 * Detects plain HTTP traffic on the HTTPS port and issues a redirect,
 * ensuring a seamless UX while maintaining mandatory encryption.
 * ======================================================================== */

import net from 'node:net';
import http from 'node:http';
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

// Wait for certificates to appear (up to 30 seconds)
const waitForFiles = async (paths, timeoutMs = 30000) => {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (paths.every(p => fs.existsSync(p))) {
      return true;
    }
    await new Promise(resolve => setTimeout(resolve, 500));
  }
  return false;
};

async function startServer() {
  console.log('Waiting for TLS certificates...');
  const certsAvailable = await waitForFiles([CERT_PATH, KEY_PATH]);
  if (!certsAvailable) {
    console.error(`ERROR: Timeout waiting for certificates at ${CERT_PATH} and ${KEY_PATH}`);
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

    // 1. Create the dedicated HTTPS server (not listening on a port yet)
    const httpsServer = https.createServer(options, app);

    // 2. Create a small HTTP server just for redirects
    const redirectServer = http.createServer((req, res) => {
      const host = req.headers.host || `localhost:${PORT}`;
      res.writeHead(301, { "Location": `https://${host}${req.url}` });
      res.end();
    });

    // 3. Create a raw TCP listener to sniff the protocol
    const universalServer = net.createServer((socket) => {
      socket.once('data', (buffer) => {
        // 0x16 is the first byte of a TLS Handshake (ClientHello)
        const isTls = buffer[0] === 0x16;
        const targetServer = isTls ? httpsServer : redirectServer;

        // Push the initial buffer back into the stream so the target server can read it
        socket.pause();
        socket.unshift(buffer);
        
        // Hand off the connection to the appropriate server
        targetServer.emit('connection', socket);
        socket.resume();
      });

      socket.on('error', (err) => {
        if (err.code !== 'ECONNRESET') {
          console.error('TCP Socket Error:', err);
        }
      });
    });

    universalServer.listen(PORT, '0.0.0.0', () => {
      console.log(`Pharos Web Console active on port ${PORT}`);
      console.log(`- HTTPS: https://0.0.0.0:${PORT} (Direct)`);
      console.log(`- HTTP:  http://0.0.0.0:${PORT}  (Redirecting)`);
    });

  } catch (error) {
    console.error('Failed to start server stack:', error);
    process.exit(1);
  }
}

startServer();
