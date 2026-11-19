// Real TLS and streaming proxy for the MU8 fixture. All data lives under tmp/mu8.
import https from 'node:https';
import http from 'node:http';
import { readFileSync, existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve, extname } from 'node:path';
import { spawnSync } from 'node:child_process';
const root = resolve('../../tmp/mu8');
mkdirSync(root, { recursive: true });
const key = resolve(root, 'localhost.key'), cert = resolve(root, 'localhost.crt');
if (!existsSync(cert) || !existsSync(key)) {
  const config = resolve(root, 'localhost.cnf');
  writeFileSync(config, '[req]\ndistinguished_name=dn\nx509_extensions=v3\nprompt=no\n[dn]\nCN=localhost\n[v3]\nsubjectAltName=DNS:localhost\n');
  const result = spawnSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', key, '-out', cert,
    '-days', '2', '-config', config], { stdio: 'pipe' });
  if (result.status !== 0) throw new Error(`MU8 local certificate: ${result.error ?? result.stderr}`);
}
https.createServer({ key: readFileSync(key), cert: readFileSync(cert) }, (request, response) => {
  if (request.url.startsWith('/api/')) {
    const upstream = http.request({ hostname: '127.0.0.1', port: 4188, path: request.url,
      method: request.method, headers: request.headers }, result => {
      response.writeHead(result.statusCode, result.headers); result.pipe(response);
    });
    upstream.on('error', () => { response.writeHead(502); response.end(); });
    response.on('close', () => upstream.destroy());
    request.pipe(upstream);
    return;
  }
  const path = new URL(request.url, 'https://localhost:4189').pathname;
  const file = resolve(root, 'web-dist', path === '/' ? 'index.html' : '.' + path);
  try {
    const types = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.woff2': 'font/woff2' };
    response.writeHead(200, { 'Content-Type': types[extname(file)] ?? 'application/octet-stream', 'Cache-Control': 'no-store' });
    response.end(readFileSync(file));
  } catch { response.writeHead(404); response.end(); }
}).listen(4189, 'localhost', () => console.log('MU8 HTTPS fixture ready'));
