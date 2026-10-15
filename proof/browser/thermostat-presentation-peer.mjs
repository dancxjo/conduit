// One actual foreground peer, finite commands and retained output; no workflow retries.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { StringDecoder } from 'node:string_decoder';
import { parseCaptureJSON } from './exact-capture-json.mjs';

export async function startPresentationPeer(state, invitation, cwd, output) {
  const installed = JSON.parse(await readFile(path.join(state, 'installation.json')));
  assert.ok(!installed.body_state && !installed.joined_body_state, 'presentation peer must be fresh');
  const child = spawn(installed.product_executable, ['body', 'terminal', '--state-dir', state,
    '--invitation', invitation, '--authorize-join'], { cwd, stdio: ['pipe', 'pipe', 'pipe'] });
  let stdout = '', stderr = '', inputs = '', bytes = 0, error = null, ended = false;
  const decoder = new StringDecoder('utf8');
  child.stdout.on('data', chunk => {
    bytes += chunk.length;
    if (bytes > 1024 * 1024) { error = new Error('presentation output exceeded its capture bound'); child.kill('SIGTERM'); return; }
    stdout += decoder.write(chunk);
  });
  child.stderr.on('data', chunk => { if (stderr.length < 256 * 1024) stderr += chunk; });
  child.stdin.on('error', cause => { error = cause; });
  child.once('error', cause => { error = cause; ended = true; });
  child.once('exit', () => { stdout += decoder.end(); ended = true; });
  const lines = prefix => stdout.split(/\r?\n/).slice(0, -1).filter(line => line.startsWith(prefix))
    .map(line => parseCaptureJSON(line.slice(prefix.length)));
  const shows = () => stdout.split('Terminal ready.').length - 1;
  const wait = async predicate => {
    const deadline = Date.now() + 20_000;
    while (true) {
      if (error) throw error;
      if (predicate()) return;
      assert.ok(!ended, `presentation peer stopped (${child.exitCode}): ${stderr}`);
      assert.ok(Date.now() < deadline, 'presentation peer observation timed out');
      await new Promise(resolve => setTimeout(resolve, 25));
    }
  };
  const send = text => {
    if (error) throw error;
    assert.ok(!ended, 'presentation peer already stopped');
    assert.ok(Buffer.byteLength(`${text}\n`) <= 256);
    inputs += `${text}\n`;
    assert.ok(Buffer.byteLength(inputs) <= 4096);
    child.stdin.write(`${text}\n`);
  };
  const save = async () => {
    await Promise.all([
      writeFile(path.join(output, 'terminal.input'), inputs),
      writeFile(path.join(output, 'terminal.txt'), stdout),
      writeFile(path.join(output, 'terminal.stderr'), stderr),
    ]);
  };
  const peer = {
    hostId: installed.host_id,
    async command(text) {
      const count = shows();
      send(text);
      await wait(() => shows() === count + 1);
    },
    async evidence(spoken = false) {
      const terminals = lines('Peer terminal evidence ').length;
      const readings = lines('Peer spoken evidence ').length;
      send('evidence');
      await wait(() => lines('Peer terminal evidence ').length === terminals + 1
        && (!spoken || lines('Peer spoken evidence ').length === readings + 1));
      return { terminal: lines('Peer terminal evidence ').at(-1),
        spoken: spoken ? lines('Peer spoken evidence ').at(-1) : null };
    },
    frame() {
      const start = stdout.lastIndexOf('\x1b[2J\x1b[H');
      const end = stdout.indexOf('\nTerminal ready.', start);
      assert.ok(start >= 0 && end > start, 'current acknowledged terminal frame is absent');
      return stdout.slice(start, end);
    },
    actions: () => lines('Peer terminal action '),
    async close() {
      send('quit');
      const deadline = Date.now() + 10_000;
      while (!ended) {
        assert.ok(Date.now() < deadline, 'presentation peer did not close');
        await new Promise(resolve => setTimeout(resolve, 25));
      }
      await save();
      assert.equal(child.exitCode, 0, stderr);
    },
    async abort() { if (!ended) child.kill('SIGTERM'); await save(); },
  };
  try { await wait(() => shows() === 1); return peer; }
  catch (cause) { await peer.abort(); throw cause; }
}
