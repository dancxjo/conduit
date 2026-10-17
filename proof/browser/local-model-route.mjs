// A producer-owned loopback route to a separately running Ollama service.
// A private control socket can withdraw and restore this exact listener while
// the shared model service keeps running. Without one, process exit withdraws
// the separate diagnostic route as before.
import { createConnection, createServer } from 'node:net';
import { unlinkSync } from 'node:fs';
import { chmod } from 'node:fs/promises';

const upstream = new URL(process.argv[2]);
if (upstream.protocol !== 'http:' ||
    !['127.0.0.1', 'localhost', '[::1]'].includes(upstream.hostname) ||
    upstream.pathname !== '/' || upstream.search || upstream.hash) {
  throw new Error('model route requires one explicit local HTTP Ollama origin');
}
const port = Number(upstream.port || 80);
const controlPath = process.argv[3];
const listenPort = process.argv[4] === undefined ? 0 : Number(process.argv[4]);
if (!Number.isInteger(listenPort) || listenPort < 0 || listenPort > 65535 ||
    (process.argv[4] !== undefined && listenPort === 0)) {
  throw new Error('model route listen port must be an explicit nonzero TCP port');
}
const connections = new Set();
const server = createServer(client => {
  const model = createConnection({ host: upstream.hostname === '[::1]' ? '::1' : upstream.hostname, port });
  connections.add(client);
  connections.add(model);
  client.pipe(model);
  model.pipe(client);
  client.on('error', () => model.destroy());
  model.on('error', () => client.destroy());
  client.on('close', () => { connections.delete(client); model.destroy(); });
  model.on('close', () => { connections.delete(model); client.destroy(); });
});
let endpoint;
let withdrawn = false;
let controlStarted = false;
server.listen(listenPort, '127.0.0.1', async () => {
  const address = server.address();
  endpoint ??= `http://127.0.0.1:${address.port}`;
  if (controlPath && !controlStarted) {
    controlStarted = true;
    const control = createServer(client => {
      let command = '';
      client.setTimeout(5_000, () => client.destroy());
      client.on('data', chunk => {
        command += chunk.toString();
        if (command.length > 64) { client.destroy(); return; }
        if (!command.includes('\n')) return;
        const action = command.trim();
        if (!['status', 'withdraw', 'restore'].includes(action)) {
          client.end(`${JSON.stringify({ error: 'unknown route control action' })}\n`);
          return;
        }
        const respond = () => client.end(`${JSON.stringify({ endpoint, upstream: upstream.origin,
          state: withdrawn ? 'withdrawn' : 'available', action })}\n`);
        if (action === 'withdraw' && !withdrawn) {
          withdrawn = true;
          for (const connection of connections) connection.destroy();
          server.close(respond);
        } else if (action === 'restore' && withdrawn) {
          server.listen(address.port, '127.0.0.1', () => {
            withdrawn = false;
            respond();
          });
        } else {
          respond();
        }
      });
    });
    control.listen(controlPath, async () => {
      await chmod(controlPath, 0o600);
      process.stdout.write(`${JSON.stringify({ endpoint, control_socket: controlPath })}\n`);
    });
    process.on('exit', () => { try { unlinkSync(controlPath); } catch {} });
    const stop = () => {
      control.close();
      for (const connection of connections) connection.destroy();
      server.close(() => process.exit(0));
    };
    process.on('SIGTERM', stop);
    if (process.argv[5] === '--supervised') {
      process.stdin.on('end', stop);
      process.stdin.resume();
    }
  } else if (!controlPath) {
    process.stdout.write(`${JSON.stringify({ endpoint })}\n`);
  }
});
if (!controlPath) process.on('SIGTERM', () => {
  for (const connection of connections) connection.destroy();
  server.close(() => process.exit(0));
});
