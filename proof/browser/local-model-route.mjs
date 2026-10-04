// A producer-owned loopback route to a separately running Ollama service.
// Closing this process withdraws only this configured route, not the model.
import { createConnection, createServer } from 'node:net';

const upstream = new URL(process.argv[2]);
if (upstream.protocol !== 'http:' ||
    !['127.0.0.1', 'localhost', '[::1]'].includes(upstream.hostname) ||
    upstream.pathname !== '/' || upstream.search || upstream.hash) {
  throw new Error('model route requires one explicit local HTTP Ollama origin');
}
const port = Number(upstream.port || 80);
const server = createServer(client => {
  const model = createConnection({ host: upstream.hostname === '[::1]' ? '::1' : upstream.hostname, port });
  client.pipe(model);
  model.pipe(client);
  client.on('error', () => model.destroy());
  model.on('error', () => client.destroy());
  client.on('close', () => model.destroy());
  model.on('close', () => client.destroy());
});
server.listen(0, '127.0.0.1', () => {
  const address = server.address();
  process.stdout.write(`${JSON.stringify({ endpoint: `http://127.0.0.1:${address.port}` })}\n`);
});
process.on('SIGTERM', () => server.close(() => process.exit(0)));
