// Live, attended-local acceptance over an installed Linux owner and the
// packaged Handbook. No membership or runtime state is constructed by this driver.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawn, spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import { setTimeout as pause } from 'node:timers/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { startStaticProduct } from './static-product-server.mjs';

const [binaryArgument, handbookArgument, outputArgument, playwrightArgument] = process.argv.slice(2);
if (!binaryArgument || !handbookArgument || !outputArgument || !playwrightArgument) {
  throw new Error('usage: handbook-owner-join.mjs INSTALLED-OWNER HANDBOOK-PACKAGE NEW-EVIDENCE-DIR PINNED-PLAYWRIGHT');
}
const binary = path.resolve(binaryArgument);
const handbook = path.resolve(handbookArgument);
const output = path.resolve(outputArgument);
if (existsSync(output)) throw new Error(`evidence directory already exists: ${output}`);
if (Buffer.byteLength(path.join(output, 'owner-state', 'control.sock')) >= 108) {
  throw new Error('evidence directory is too long for the installed owner control socket');
}
const { chromium } = await import(pathToFileURL(path.resolve(playwrightArgument)));
const digest = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
const ownerBytes = await readFile(binary);
const app = await readFile(path.join(handbook, 'application.application.json'));
const releaseDir = path.join(output, 'release');
await mkdir(releaseDir, { recursive: true });
const name = 'conduit-linux-x86_64';
await writeFile(path.join(releaseDir, name), ownerBytes);
const file = { path: name, bytes: ownerBytes.length, sha256: digest(ownerBytes) };
await writeFile(path.join(releaseDir, 'release.json'), JSON.stringify({
  schema: 'conduit.release/host-bundle@1', source_identity: 'live-handbook-owner-join',
  bundle_sha256: digest(Buffer.from(`conduit.release/host-bundle-content@1\0${name}\0${file.sha256}\n`)),
  files: [file],
}));
const state = path.join(output, 'owner-state');
const install = spawnSync(binary, ['host', 'service', 'install', path.join(releaseDir, 'release.json'), '--state-dir', state, '--no-start'], { encoding: 'utf8' });
assert.equal(install.status, 0, install.stderr);
const installation = JSON.parse(await readFile(path.join(state, 'installation.json')));
const source = path.join(output, 'clock.conduit');
await writeFile(source, await readFile(new URL('../../plots/clock/main.conduit', import.meta.url)));
const owner = spawn(installation.product_executable, ['body', 'own', source, '--state-dir', state], { stdio: ['pipe', 'pipe', 'pipe'] });
const records = [], errors = [], waiters = [];
owner.stderr.on('data', chunk => errors.push(chunk.toString()));
createInterface({ input: owner.stdout }).on('line', line => {
  try { records.push(JSON.parse(line)); } catch (error) { errors.push(String(error)); }
  for (const waiter of [...waiters]) waiter();
});
const waitRecord = predicate => new Promise((resolve, reject) => {
  const timeout = setTimeout(() => { cleanup(); reject(new Error(`owner record deadline: ${errors.join('')}`)); }, 25_000);
  function cleanup() { clearTimeout(timeout); const index = waiters.indexOf(check); if (index >= 0) waiters.splice(index, 1); }
  function check() { const found = records.find(predicate); if (found) { cleanup(); resolve(found); } }
  waiters.push(check); check();
});
const exitOf = child => child.exitCode !== null ? Promise.resolve(child.exitCode) : new Promise((resolve, reject) => {
  const timeout = setTimeout(() => reject(new Error('owner exit deadline')), 20_000);
  child.once('exit', code => { clearTimeout(timeout); resolve(code); });
});
let server, browser, service;
try {
  server = await startStaticProduct(handbook);
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await context.newPage();
  const pageErrors = [], browserMessages = [];
  page.on('pageerror', error => pageErrors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') browserMessages.push(message.text()); });
  const initial = await waitRecord(record => record.schema === 'conduit.body/owner-truth@1');
  owner.stdin.write('{"operation":"close"}\n');
  owner.stdin.end();
  assert.equal(await exitOf(owner), 0, errors.join(''));
  service = spawn(installation.product_executable, ['host', 'service', 'run', '--state-dir', state],
    { stdio: ['ignore', 'ignore', 'pipe'] });
  service.stderr.on('data', chunk => errors.push(chunk.toString()));
  for (let attempt = 0; attempt < 100 && !existsSync(path.join(state, 'control.sock')); attempt++) {
    if (service.exitCode !== null) throw new Error(`installed service exited: ${errors.join('')}`);
    await pause(50);
  }
  assert.equal(existsSync(path.join(state, 'control.sock')), true, `installed service did not start: ${errors.join('')}`);
  await page.goto(`${server.url}?participate=owner#your-handbook`);
  await page.locator('[data-owner-key]').waitFor();
  const identity = await page.evaluate(() => globalThis.__conduitOwnerParticipation.admissionIdentity());
  assert.equal(identity.hostId, await page.locator('[data-owner-host]').textContent());
  assert.equal(identity.bootId, await page.locator('[data-owner-boot]').textContent());
  const opened = spawnSync(installation.product_executable,
    ['body', 'browser-window', '--state-dir', state, '--expected-host-id', identity.hostId,
      '--new-host-verifying-key', JSON.stringify(identity.verifyingKey), '--maximum-millis', '60000', '--authorize-window'],
    { encoding: 'utf8', timeout: 10_000 });
  assert.equal(opened.status, 0, opened.stderr);
  const window = JSON.parse(opened.stdout);
  assert.equal(window.body_id, initial.biography.body_id);
  await page.getByLabel('Body ID').fill(window.body_id);
  await page.getByLabel('Owner window URL').fill(window.url);
  await page.getByRole('button', { name: 'Join this Body' }).click();
  try {
    await page.waitForFunction(() => globalThis.__conduitOwnerParticipation?.presence() === 'available', null, { timeout: 12_000 });
  } catch (error) {
    throw new Error(`browser presence deadline: ${await page.locator('[data-owner-status]').textContent()}; ${pageErrors.join('; ')}; ${browserMessages.join('; ')}`, { cause: error });
  }
  const admitted = await page.evaluate(() => ({
    credential: globalThis.__conduitOwnerParticipation.credential(),
    biography: globalThis.__conduitOwnerParticipation.biography(),
    host: globalThis.__conduitOwnerParticipation.host(),
  }));
  assert.equal(admitted.credential.body_id, initial.biography.body_id);
  assert.equal(admitted.credential.host_id, identity.hostId);
  assert.equal(admitted.credential.boot_id, identity.bootId);
  assert.equal(admitted.biography.membership.parts.length, 2);
  assert.equal(admitted.biography.membership.parts.find(part => part.part_id === admitted.credential.part_id).current.boot_id, identity.bootId);
  assert.equal(await page.locator('[data-owner-result]').textContent(),
    `Body ${window.body_id} admitted Part ${admitted.credential.part_id} on this Host and Boot. The Linux owner remains authoritative; this browser can present its Face through an owner-issued Mask Plan.`);
  const screenshot = path.join(output, 'browser-admitted.png');
  await page.screenshot({ path: screenshot, fullPage: true });
  await page.locator('[data-owner-face-document] [data-owner-action]').first().waitFor();
  const beforeFace = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(beforeFace.body_id, initial.biography.body_id);
  assert.equal(await page.locator('[data-handbook-application]').getAttribute('data-owner-show-acknowledged'), beforeFace.show_id);
  assert.equal(beforeFace.show_state, 'available');
  assert.equal(typeof beforeFace.mask_plan_id, 'string');
  assert.ok(beforeFace.mask_plan_id.length > 0);
  assert.equal(typeof beforeFace.mask_play_id, 'string');
  assert.ok(beforeFace.mask_play_id.length > 0);
  assert.match(beforeFace.show_id, /^show\/[a-f0-9]{64}$/);
  const clockAction = beforeFace.actions.find(action => action.intent === 'conduit.intent/change-clock-interval@1');
  assert.equal(clockAction.availability, 'available');
  const clockControl = page.locator('[data-owner-action]').filter({ has: page.getByRole('button', { name: 'Change ticker pace' }) });
  await clockControl.getByRole('combobox').selectOption('500');
  await clockControl.getByRole('button', { name: 'Change ticker pace' }).click();
  await page.waitForFunction(previous => {
    const face = globalThis.__conduitOwnerParticipation.face();
    return document.querySelector('[data-owner-action-result]')?.textContent === 'The owner accepted Change ticker pace.'
      && face?.face_revision !== previous && face?.subjects?.some(subject =>
        subject.text.some(text => text.includes('500 milliseconds')));
  }, beforeFace.face_revision);
  const afterFace = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(afterFace.body_id, beforeFace.body_id);
  assert.equal(await page.locator('[data-handbook-application]').getAttribute('data-owner-show-acknowledged'), afterFace.show_id);
  assert.notEqual(afterFace.face_revision, beforeFace.face_revision);
  assert.equal(afterFace.show_state, 'available');
  assert.notEqual(afterFace.show_id, beforeFace.show_id);
  assert.match(await readFile(path.join(state, 'body/source.conduit'), 'utf8'), /time\/every\(500ms\)/);
  const changedScreenshot = path.join(output, 'browser-clock-changed.png');
  await page.screenshot({ path: changedScreenshot, fullPage: true });
  const startAction = afterFace.actions.find(action => action.intent === 'conduit.intent/start-clock@1');
  assert.equal(startAction?.availability, 'available');
  await page.getByRole('button', { name: 'Start the ticker' }).click();
  await page.waitForFunction(() => document.querySelector('[data-owner-action-result]')?.textContent
    === 'The owner accepted Start the ticker.');
  const status = () => {
    const result = spawnSync(installation.product_executable,
      ['body', 'status', '--state-dir', state, '--json'], { encoding: 'utf8', timeout: 5_000 });
    assert.equal(result.status, 0, result.stderr);
    return JSON.parse(result.stdout);
  };
  let live;
  for (let attempt = 0; attempt < 100; attempt++) {
    live = status();
    if (live.realization?.play?.active_play_id) break;
    await pause(25);
  }
  assert.equal(live.biography.body_id, initial.biography.body_id);
  assert.equal(typeof live.realization?.play?.active_play_id, 'string');
  await page.getByRole('button', { name: 'Refresh this Face' }).click();
  try {
    await page.waitForFunction(() => globalThis.__conduitOwnerParticipation.face()?.actions
      .some(action => action.intent === 'conduit.intent/lull-clock@1' && action.availability === 'available'));
  } catch (error) {
    await page.screenshot({ path: path.join(output, 'browser-clock-playing-diagnostic.png'), fullPage: true });
    const faceDiagnostic = await page.evaluate(() => ({
      status: document.querySelector('[data-owner-face-status]')?.textContent,
      actionResult: document.querySelector('[data-owner-action-result]')?.textContent,
      presence: globalThis.__conduitOwnerParticipation?.presence(),
      face: globalThis.__conduitOwnerParticipation?.face(),
    }));
    throw new Error(`playing Face did not expose Stop: ${JSON.stringify(faceDiagnostic)}`, { cause: error });
  }
  const playingScreenshot = path.join(output, 'browser-clock-playing.png');
  await page.screenshot({ path: playingScreenshot, fullPage: true });
  const playingFaceScreenshot = path.join(output, 'browser-clock-playing-face.png');
  await page.locator('[data-owner-face]').screenshot({ path: playingFaceScreenshot });
  const playingFace = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(await page.locator('[data-handbook-application]').getAttribute('data-owner-show-acknowledged'), playingFace.show_id);
  assert.equal(playingFace.show_state, 'available');
  assert.notEqual(playingFace.show_id, afterFace.show_id);
  await page.getByRole('button', { name: 'Stop the ticker' }).click();
  try {
    await page.waitForFunction(() => document.querySelector('[data-owner-action-result]')?.textContent
      === 'The owner accepted Stop the ticker.');
  } catch (error) {
    await page.screenshot({ path: path.join(output, 'browser-clock-stop-diagnostic.png'), fullPage: true });
    const actionDiagnostic = await page.evaluate(() => ({
      status: document.querySelector('[data-owner-face-status]')?.textContent,
      actionResult: document.querySelector('[data-owner-action-result]')?.textContent,
      presence: globalThis.__conduitOwnerParticipation?.presence(),
    }));
    throw new Error(`Stop was not accepted: ${JSON.stringify(actionDiagnostic)}`, { cause: error });
  }
  let stopped;
  for (let attempt = 0; attempt < 100; attempt++) {
    stopped = status();
    if (stopped.realization === null) break;
    await pause(25);
  }
  assert.equal(stopped.biography.body_id, initial.biography.body_id);
  assert.equal(stopped.realization, null);
  assert.equal(stopped.last_execution.play.active_play_id, live.realization.play.active_play_id);
  await page.getByRole('button', { name: 'Refresh this Face' }).click();
  await page.waitForFunction(previous => {
    const face = globalThis.__conduitOwnerParticipation.face();
    return face?.face_revision !== previous && face.actions.some(action =>
      action.intent === 'conduit.intent/start-clock@1' && action.availability === 'available');
  }, playingFace.face_revision);
  const stoppedScreenshot = path.join(output, 'browser-clock-stopped.png');
  await page.screenshot({ path: stoppedScreenshot, fullPage: true });
  const stoppedFaceScreenshot = path.join(output, 'browser-clock-stopped-face.png');
  await page.locator('[data-owner-face]').screenshot({ path: stoppedFaceScreenshot });
  await page.getByRole('button', { name: 'Leave this window' }).click();
  assert.deepEqual(pageErrors, []);
  let final;
  for (let attempt = 0; attempt < 100; attempt++) {
    final = JSON.parse(await readFile(path.join(state, 'body/biography.json')));
    if (final.membership.parts.find(part => part.part_id === admitted.credential.part_id)?.current === null) break;
    await pause(50);
  }
  assert.equal(final.body_id, initial.biography.body_id);
  assert.equal(final.membership.parts.length, 2);
  assert.equal(final.membership.parts.find(part => part.part_id === admitted.credential.part_id).current, null);
  const terminal = commands => {
    const run = spawnSync(installation.product_executable,
      ['body', 'terminal', '--state-dir', state], {
        input: `${commands.join('\n')}\n`, encoding: 'utf8', timeout: 10_000, maxBuffer: 512 * 1024,
      });
    assert.equal(run.status, 0, run.stderr);
    assert.doesNotMatch(run.stdout, /Action refused:/);
    return run.stdout;
  };
  const terminalStartText = terminal(['control next', 'control next', 'control next', 'apply', 'quit']);
  assert.match(terminalStartText, /"schema":"conduit\.body\/clock-start-requested@1"/);
  const terminalStartTranscript = path.join(output, 'terminal-start.txt');
  await writeFile(terminalStartTranscript, terminalStartText);
  let terminalLive;
  for (let attempt = 0; attempt < 100; attempt++) {
    terminalLive = status();
    if (terminalLive.realization?.play?.active_play_id) break;
    await pause(25);
  }
  assert.equal(terminalLive.biography.body_id, final.body_id);
  assert.equal(typeof terminalLive.realization?.play?.active_play_id, 'string');
  assert.notEqual(terminalLive.realization.play.active_play_id, live.realization.play.active_play_id);
  const terminalStopText = terminal(['control next', 'apply', 'quit']);
  assert.match(terminalStopText, /"schema":"conduit\.body\/clock-lull-requested@1"/);
  const terminalStopTranscript = path.join(output, 'terminal-stop.txt');
  await writeFile(terminalStopTranscript, terminalStopText);
  let terminalStopped;
  for (let attempt = 0; attempt < 100; attempt++) {
    terminalStopped = status();
    if (terminalStopped.realization === null) break;
    await pause(25);
  }
  assert.equal(terminalStopped.biography.body_id, final.body_id);
  assert.equal(terminalStopped.realization, null);
  assert.equal(terminalStopped.last_execution.play.active_play_id, terminalLive.realization.play.active_play_id);
  assert.deepEqual(terminalStopped.last_execution.terminal, { Cancelled: { reason: 'OperatorRequested' } });
  await writeFile(path.join(output, 'report.json'), `${JSON.stringify({
    schema: 'conduit.body/handbook-owner-join@1', ownerSha256: digest(ownerBytes),
    handbookManifestSha256: digest(app), browserBundleSource: JSON.parse(await readFile(path.join(handbook, 'sdk/bundle/conduit-browser-image.json'))).reviewed_distribution.source_commit,
    bodyId: final.body_id, browserHostId: identity.hostId, browserBootId: identity.bootId,
    browserPartId: admitted.credential.part_id, finalPartCount: final.membership.parts.length,
    screenshot: 'browser-admitted.png', screenshotSha256: digest(await readFile(screenshot)),
    actionScreenshot: 'browser-clock-changed.png', actionScreenshotSha256: digest(await readFile(changedScreenshot)),
    actionId: clockAction.identity, priorFaceId: beforeFace.face_id, priorFaceRevision: beforeFace.face_revision,
    priorMaskPlanId: beforeFace.mask_plan_id, priorMaskPlayId: beforeFace.mask_play_id,
    priorShowId: beforeFace.show_id, priorShowState: beforeFace.show_state,
    ownerShowAcknowledged: true,
    resultingFaceId: afterFace.face_id, resultingFaceRevision: afterFace.face_revision,
    resultingMaskPlanId: afterFace.mask_plan_id, resultingShowId: afterFace.show_id,
    resultingWorksetIntervalMs: 500,
    startActionId: startAction.identity, playingFaceId: playingFace.face_id,
    playId: live.realization.play.active_play_id, stopActionId: playingFace.actions.find(action =>
      action.intent === 'conduit.intent/lull-clock@1').identity,
    terminal: stopped.last_execution.terminal,
    playingScreenshot: 'browser-clock-playing.png', playingScreenshotSha256: digest(await readFile(playingScreenshot)),
    playingFaceScreenshot: 'browser-clock-playing-face.png', playingFaceScreenshotSha256: digest(await readFile(playingFaceScreenshot)),
    stoppedScreenshot: 'browser-clock-stopped.png', stoppedScreenshotSha256: digest(await readFile(stoppedScreenshot)),
    stoppedFaceScreenshot: 'browser-clock-stopped-face.png', stoppedFaceScreenshotSha256: digest(await readFile(stoppedFaceScreenshot)),
    terminalStartTranscript: 'terminal-start.txt', terminalStartTranscriptSha256: digest(await readFile(terminalStartTranscript)),
    terminalStopTranscript: 'terminal-stop.txt', terminalStopTranscriptSha256: digest(await readFile(terminalStopTranscript)),
    terminalPlayId: terminalLive.realization.play.active_play_id,
    terminalTerminal: terminalStopped.last_execution.terminal,
    ownerWindow: 'loopback', remoteExecution: false, serviceOwned: true,
  }, null, 2)}\n`);
  console.log(`PASS: ${path.join(output, 'report.json')}`);
} finally {
  if (owner.exitCode === null) owner.kill();
  await browser?.close();
  if (server) server.child.kill();
  if (service?.exitCode === null) service.kill();
  await writeFile(path.join(output, 'owner-records.json'), JSON.stringify({ records, errors }, null, 2));
}
