// A private, finite pinned TLS invitation for this owned capture only.
// The supplied installed peer signs its own actual offer and owns its terminal.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { chmod, mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import net from 'node:net';

export async function openPeerRoute(binary, state, cwd) {
  const route = spawnSync('ip', ['-j', 'route', 'get', '192.0.2.1'], { encoding: 'utf8', timeout: 3000 });
  assert.equal(route.status, 0, 'capture needs an actual routed IPv4 interface');
  const address = JSON.parse(route.stdout)[0]?.prefsrc;
  assert.ok(net.isIPv4(address) && !address.startsWith('127.'), 'no non-loopback routed IPv4 interface');
  const privateRoot = await mkdtemp(path.join(tmpdir(), 'conduit-thermostat-route-'));
  await chmod(privateRoot, 0o700);
  const cert = path.join(privateRoot, 'owner.pem');
  const key = path.join(privateRoot, 'owner.key');
  const generated = spawnSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', key,
    '-out', cert, '-days', '1', '-subj', `/CN=${address}`, '-addext', `subjectAltName=IP:${address}`],
    { encoding: 'utf8', timeout: 15_000, maxBuffer: 1024 * 1024 });
  assert.equal(generated.status, 0, 'could not prepare the explicit capture TLS identity');
  await chmod(key, 0o600);
  const reservation = net.createServer();
  await new Promise((resolve, reject) => { reservation.once('error', reject); reservation.listen(0, address, resolve); });
  const port = reservation.address().port;
  await new Promise((resolve, reject) => reservation.close(error => error ? reject(error) : resolve()));
  const child = spawn(binary, ['body', 'invite', '--state-dir', state, '--ttl-seconds', '60',
    '--route-bind', `${address}:${port}`, '--route-url', `wss://${address}:${port}/owner`,
    '--route-tls-cert', cert, '--route-tls-key', key, '--authorize-route'], { cwd, stdio: ['ignore', 'pipe', 'pipe'] });
  let stderr = '';
  child.stderr.on('data', bytes => { if (stderr.length < 1024 * 1024) stderr += bytes; });
  try {
    const invitation = await new Promise((resolve, reject) => {
      let stdout = '';
      let ready = false;
      const timeout = setTimeout(() => reject(new Error('capture invitation did not become ready')), 15_000);
      child.stdout.on('data', bytes => {
        if (ready) return;
        stdout += bytes;
        if (stdout.length > 512 * 1024) { clearTimeout(timeout); reject(new Error('invitation exceeded its bound')); return; }
        const end = stdout.indexOf('\n');
        if (end >= 0) { ready = true; clearTimeout(timeout); resolve(stdout.slice(0, end)); }
      });
      child.once('error', error => { clearTimeout(timeout); reject(error); });
      child.once('exit', code => { clearTimeout(timeout); reject(new Error(`capture invitation stopped (${code})`)); });
    });
    JSON.parse(invitation);
    const invitationPath = path.join(privateRoot, 'invitation.json');
    await writeFile(invitationPath, invitation, { mode: 0o600 });
    return { child, invitationPath, diagnostic: () => stderr };
  } catch (error) { child.kill('SIGTERM'); throw error; }
}
