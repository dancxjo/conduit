// A private Unix socket controls the exact loopback model route selected by
// the installed owner. The owning producer must keep the route process alive.
import assert from 'node:assert/strict';
import { createConnection } from 'node:net';
import { stat } from 'node:fs/promises';
import path from 'node:path';

export async function ownerModelRouteControl(socketPath, action, selectedEndpoint) {
  assert.ok(['status', 'withdraw', 'restore'].includes(action));
  const socket = await stat(socketPath);
  assert.equal(socket.isSocket(), true, 'owner model route control is not a socket');
  const mode = socket.mode & 0o777;
  assert.equal(mode, 0o600, 'owner model route control must be private');
  assert.equal((await stat(path.dirname(socketPath))).mode & 0o077, 0,
    'owner model route control directory must be private');
  const result = await new Promise((resolve, reject) => {
    const socket = createConnection(socketPath);
    let output = '';
    socket.setTimeout(5_000, () => socket.destroy(new Error('route control deadline')));
    socket.once('connect', () => socket.write(`${action}\n`));
    socket.on('data', chunk => {
      output += chunk.toString();
      if (output.length > 2048) socket.destroy(new Error('route control reply exceeds bound'));
    });
    socket.once('error', reject);
    socket.once('close', () => {
      try { resolve(JSON.parse(output)); } catch (error) { reject(error); }
    });
  });
  assert.equal(result.endpoint, selectedEndpoint,
    'controlled route differs from installed owner model endpoint');
  assert.ok(['available', 'withdrawn'].includes(result.state));
  assert.equal(result.action, action);
  return result;
}
