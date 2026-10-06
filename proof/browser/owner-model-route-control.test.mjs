import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { ownerModelRouteControl } from './owner-model-route-control.mjs';

test('selected loopback route withdraws and restores without stopping upstream', async () => {
  const directory = await mkdtemp(path.join(tmpdir(), 'conduit-model-route-'));
  const upstream = createServer((_request, response) => response.end('real-upstream'));
  await new Promise(resolve => upstream.listen(0, '127.0.0.1', resolve));
  const upstreamUrl = `http://127.0.0.1:${upstream.address().port}`;
  const socket = path.join(directory, 'control.sock');
  const route = spawn(process.execPath, [new URL('./local-model-route.mjs', import.meta.url).pathname,
    upstreamUrl, socket], { stdio: ['ignore', 'pipe', 'pipe'] });
  try {
    const ready = await new Promise((resolve, reject) => {
      let output = '';
      route.stdout.on('data', chunk => {
        output += chunk;
        if (output.includes('\n')) resolve(JSON.parse(output.split('\n')[0]));
      });
      route.once('error', reject);
      route.once('exit', code => reject(new Error(`route exited before ready: ${code}`)));
    });
    assert.equal(ready.control_socket, socket);
    assert.equal((await ownerModelRouteControl(socket, 'status', ready.endpoint)).state,
      'available');
    assert.equal(await (await fetch(ready.endpoint)).text(), 'real-upstream');
    assert.equal((await ownerModelRouteControl(socket, 'withdraw', ready.endpoint)).state,
      'withdrawn');
    await assert.rejects(fetch(ready.endpoint));
    assert.equal(await (await fetch(upstreamUrl)).text(), 'real-upstream');
    assert.equal((await ownerModelRouteControl(socket, 'restore', ready.endpoint)).state,
      'available');
    assert.equal(await (await fetch(ready.endpoint)).text(), 'real-upstream');
    await assert.rejects(ownerModelRouteControl(socket, 'status', upstreamUrl),
      /differs from installed owner/);
  } finally {
    route.kill('SIGTERM');
    await new Promise(resolve => route.once('exit', resolve));
    await new Promise(resolve => upstream.close(resolve));
    await rm(directory, { recursive: true, force: true });
  }
});
