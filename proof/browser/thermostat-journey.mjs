// Fresh installed-owner capture. Own only this process and this new Body;
// preserve observations on failure and never substitute fixture screenshots.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

const [stateArg, binaryArg, handbookArg, outputArg, playwrightArg] = process.argv.slice(2);
assert.ok(playwrightArg, 'expected STATE INSTALLED-BIN HANDBOOK NEW-OUTPUT PINNED-PLAYWRIGHT');
const [state, binary, handbook, output, playwright] = [stateArg, binaryArg, handbookArg, outputArg, playwrightArg].map(value => path.resolve(value));
assert.ok(!existsSync(output), 'capture output must be new');
const installed = JSON.parse(await readFile(path.join(state, 'installation.json')));
assert.equal(path.resolve(installed.product_executable), binary);
assert.ok(!installed.body_state && !installed.joined_body_state, 'capture requires a fresh unowned installed Host');
assert.ok(!existsSync(path.join(state, 'control.sock')), 'capture must own its foreground service');
assert.ok(Buffer.byteLength(path.join(state, 'control.sock')) <= 107);
await mkdir(output, { mode: 0o700 });
let service;
try {
  const birth = spawnSync(binary, ['body', 'own', 'plots/thermostat/main.conduit', '--state-dir', state, '--name', 'Living room'],
    { encoding: 'utf8', input: '', timeout: 30_000, maxBuffer: 1024 * 1024 });
  await writeFile(path.join(output, 'birth.txt'), `${birth.stdout ?? ''}${birth.stderr ?? ''}`);
  assert.equal(birth.status, 0, birth.stderr);
  service = spawn(binary, ['host', 'service', 'run', '--state-dir', state], { stdio: ['ignore', 'ignore', 'pipe'] });
  let serviceLog = '';
  service.stderr.on('data', bytes => { if (serviceLog.length < 1024 * 1024) serviceLog += bytes; });
  const deadline = Date.now() + 15_000;
  while (!existsSync(path.join(state, 'control.sock'))) {
    assert.equal(service.exitCode, null, serviceLog);
    assert.ok(Date.now() < deadline, 'service did not open its socket');
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  const driver = spawn(process.execPath, ['proof/browser/thermostat-owner-browser.mjs', process.cwd(), binary,
    state, handbook, path.join(output, 'capture'), playwright, 'journey'], { stdio: 'inherit' });
  const exit = await new Promise(resolve => driver.once('exit', resolve));
  await writeFile(path.join(output, 'service.txt'), serviceLog);
  assert.equal(exit, 0, 'thermostat journey driver failed; inspect retained capture diagnostics');
  const receipt = JSON.parse(await readFile(path.join(output, 'capture/receipt.json')));
  const files = [];
  for (const name of await readdir(path.join(output, 'capture'))) {
    const bytes = await readFile(path.join(output, 'capture', name));
    assert.ok(bytes.length <= 16 * 1024 * 1024);
    files.push({ path: `capture/${name}`, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') });
  }
  assert.ok(files.length <= 32);
  await writeFile(path.join(output, 'manifest.json'), `${JSON.stringify({ schema: 'conduit.thermostat-journey/capture@1',
    capture_entrance: 'cargo xtask prove journey thermostat --capture',
    capture_tool_commit: spawnSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).stdout.trim(),
    capture_tool_dirty: Boolean(spawnSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).stdout.trim()),
    source_commit: installed.release_source_identity, source_relation: receipt.source_relation,
    body_id: receipt.body_id, body_plan_id: receipt.body_plan_id, body_play_id: receipt.body_play_id,
    result: 'browser-terminal-complete-speech-unavailable', files }, null, 2)}\n`);
  console.log(`Thermostat journey capture retained: ${output}`);
} catch (error) {
  await writeFile(path.join(output, 'failure.json'), `${JSON.stringify({ error: String(error) }, null, 2)}\n`);
  throw error;
} finally {
  if (service && service.exitCode === null) {
    service.kill('SIGTERM');
    await new Promise(resolve => service.once('exit', resolve));
  }
}
