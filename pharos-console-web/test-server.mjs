import net from 'node:net';
import http from 'node:http';
import https from 'node:https';
import fs from 'node:fs';
import express from 'express';
import { handler as ssrHandler } from './dist/server/entry.mjs';

const PORT = process.env.PORT || 3000;
const app = express();

app.use(express.static('dist/client'));
app.use(ssrHandler);

let httpsServer;
try {
    // Attempt to load certs from the known location used by pre-flight
    const certPath = '/tmp/e2e-certs/pharos-web.crt';
    const keyPath = '/tmp/e2e-certs/pharos-web.key';
    
    if (fs.existsSync(certPath) && fs.existsSync(keyPath)) {
        const options = {
            cert: fs.readFileSync(certPath),
            key: fs.readFileSync(keyPath),
        };
        httpsServer = https.createServer(options, app);
        console.log('E2E Test server enabled HTTPS support');
    }
} catch (e) {
    console.warn('E2E Test server could not initialize HTTPS:', e.message);
}

const httpServer = http.createServer(app);

const redirectServer = http.createServer((req, res) => {
  const host = req.headers.host || `localhost:${PORT}`;
  res.writeHead(301, { "Location": `https://${host}${req.url}` });
  res.end();
});

const universalServer = net.createServer((socket) => {
  socket.once('data', (buffer) => {
    const firstByte = buffer[0];
    const isTls = firstByte === 0x16;
    
    socket.pause();
    socket.unshift(buffer);

    if (isTls && httpsServer) {
        httpsServer.emit('connection', socket);
    } else if (isTls && !httpsServer) {
        // We got TLS but have no certs, fallback to plain HTTP (will likely fail SSL handshake)
        httpServer.emit('connection', socket);
    } else if (!isTls && httpsServer) {
        // Plain HTTP but we have HTTPS enabled, redirect!
        redirectServer.emit('connection', socket);
    } else {
        // Plain HTTP, serve app
        httpServer.emit('connection', socket);
    }
    
    process.nextTick(() => socket.resume());
  });

  socket.on('error', (err) => {
    if (err.code !== 'ECONNRESET') {
      console.error('E2E TCP Socket Error:', err);
    }
  });
});

universalServer.listen(PORT, '0.0.0.0', () => {
  console.log(`E2E Test server active on port ${PORT}`);
});
