// One installed owner, one live QMP guest, and one pinned Chromium Host.
// The driver coordinates user actions; each participant creates its own
// membership, Mask Play, Show, and semantic return through product entrances.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, readFile, rename, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { startStaticProduct } from './static-product-server.mjs';
import { captureLlmChapter } from './three-host-llm-chapter.mjs';
import { captureOwnerLlmSpeaker, captureOwnerModelRouteLoss } from './three-host-owner-llm.mjs';
import { captureRunId } from './three-host-run-identity.mjs';
import { captureOwnerSelectedSpeech, observeOwnerSpeech } from './three-host-owner-speech.mjs';
import { retainOwnerSpeechArtifacts } from './three-host-owner-speech-artifacts.mjs';
import { writeThreeHostWalkthrough } from './three-host-walkthrough.mjs';
import { capturePresentationRecovery } from './three-host-presentation-recovery.mjs';
import { recordBrowserCapture } from './three-host-capture-observation.mjs';
import { verifyWalkthroughAssets } from './three-host-walkthrough-assets.mjs';

const [xtaskArgument, ownerArgument, stateArgument, handbookArgument, sporeArgument,
  candidateId, ownerForward, outputArgument, playwrightArgument,
  speechExecutableArgument, speechDataArgument, speechEngineArgument, speechLanguageCoverageArgument,
  modelArgument, modelEndpointArgument, modelMemoryArgument,
  ownerModelRouteControlArgument] = process.argv.slice(2);
if (!playwrightArgument) {
  throw new Error('usage: three-host-owner-journey.mjs XTASK INSTALLED-OWNER OWNER-STATE HANDBOOK SPORE CANDIDATE-ID OWNER-FORWARD NEW-EVIDENCE-DIR PINNED-PLAYWRIGHT');
}
const xtask = path.resolve(xtaskArgument);
const owner = path.resolve(ownerArgument);
const state = path.resolve(stateArgument);
const handbook = path.resolve(handbookArgument);
const spore = path.resolve(sporeArgument);
const output = path.resolve(outputArgument);
const directSpeechEnabled = Boolean(speechExecutableArgument);
assert.equal(Boolean(speechDataArgument), directSpeechEnabled);
assert.equal(Boolean(speechEngineArgument), directSpeechEnabled);
assert.equal(Boolean(speechLanguageCoverageArgument), directSpeechEnabled);
assert.ok(!modelArgument || directSpeechEnabled, 'LLM chapter requires direct speech provider');
assert.equal(Boolean(modelArgument), Boolean(modelEndpointArgument));
assert.equal(Boolean(modelArgument), Boolean(modelMemoryArgument));
assert.equal(existsSync(output), false, 'evidence directory must be new');
await mkdir(output, { mode: 0o700 });
const native = path.join(output, 'native');
await mkdir(native, { mode: 0o700 });
const installed = JSON.parse(await readFile(path.join(state, 'installation.json')));
assert.equal(installed.product_executable, owner, 'owner binary must match the installed release');
const expectedPlaywright = JSON.parse(await readFile(new URL('./package.json', import.meta.url)))
  .devDependencies['@playwright/test'];
const actualPlaywright = JSON.parse(await readFile(path.join(path.dirname(playwrightArgument), 'package.json'))).version;
assert.equal(actualPlaywright, expectedPlaywright, 'browser proof must use the pinned Playwright release');
const { chromium } = await import(pathToFileURL(path.resolve(playwrightArgument)));
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const waitForFile = async (file, timeoutMillis = 120_000) => {
  const deadline = Date.now() + timeoutMillis;
  while (Date.now() < deadline) {
    if (existsSync(file)) return JSON.parse(await readFile(file, 'utf8'));
    if (nativeProof && (nativeProof.exitCode !== null || nativeProof.signalCode !== null)) {
      throw new Error(`native proof exited before ${path.basename(file)}: ${nativeOutput.join('')}`);
    }
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error(`checkpoint deadline: ${path.basename(file)}`);
};
const run = (args) => {
  const result = spawnSync(owner, args, { encoding: 'utf8', timeout: 10_000 });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
};
const runAttachedTerminal = (input, onLine = () => {}, timeoutMillis = 30_000) =>
  new Promise((resolve, reject) => {
    const child = spawn(owner, ['body', 'terminal', '--owner-show', '--state-dir', state], {
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    let scanned = 0;
    let timedOut = false;
    let protocolError;
    const deadline = setTimeout(() => {
      timedOut = true;
      child.kill();
    }, timeoutMillis);
    child.stdout.on('data', chunk => {
      stdout += chunk.toString('utf8');
      if (Buffer.byteLength(stdout) > 1024 * 1024) {
        protocolError = new Error('terminal transcript exceeded its 1 MiB bound');
        child.kill();
        return;
      }
      let end;
      while ((end = stdout.indexOf('\n', scanned)) !== -1) {
        const line = stdout.slice(scanned, end).replace(/\r$/, '');
        scanned = end + 1;
        try { onLine(line, child); } catch (error) {
          protocolError = error;
          child.kill();
          return;
        }
      }
    });
    child.stderr.on('data', chunk => {
      stderr += chunk.toString('utf8');
      if (Buffer.byteLength(stderr) > 1024 * 1024) {
        protocolError = new Error('terminal stderr exceeded its 1 MiB bound');
        child.kill();
      }
    });
    child.stdin.on('error', error => { protocolError ??= error; });
    child.once('error', error => { clearTimeout(deadline); reject(error); });
    child.once('close', (status, signal) => {
      clearTimeout(deadline);
      resolve({ status, signal, stdout, stderr, timedOut, protocolError });
    });
    if (input) child.stdin.end(input);
  });
let server, browser, nativeProof, speechObserver;
const nativeOutput = [];
try {
  server = await startStaticProduct(handbook);
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await context.newPage();
  speechObserver = observeOwnerSpeech(page);
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const ownerBefore = run(['body', 'status', '--state-dir', state, '--json']);
  const bodyId = ownerBefore.biography.body_id;
  const ownerPartAtCapture = ownerBefore.biography.membership.parts[0].current;
  const runId = captureRunId(bodyId, ownerPartAtCapture.host_id, ownerPartAtCapture.boot_id);
  const eventObservedAt = {};
  const checkpointHandoffs = [];
  const checkpointDirectory = path.join(output, 'screen-free-checkpoints');
  if (process.env.CONDUIT_SCREEN_FREE_CHECKPOINTS === '1') {
    await mkdir(checkpointDirectory, { mode: 0o700 });
  }
  const screenFreeCheckpoint = async (phase, routeId, routeAvailable, details = {}) => {
    if (process.env.CONDUIT_SCREEN_FREE_CHECKPOINTS !== '1') return undefined;
    assert.ok(['model-provider-unavailable', 'model-provider-restored',
      'browser-presentation-unavailable', 'browser-presentation-restored'].includes(phase));
    assert.match(routeId, /^route\//);
    const face = run(['body', 'face', '--state-dir', state, '--json']);
    assert.equal(face.presentation.basis.body_id, bodyId);
    const ready = {
      schema: 'conduit.proof/screen-free-checkpoint@1', phase,
      source_commit: installed.release_source_identity, run_id: runId, body_id: bodyId,
      owner_host_id: ownerPartAtCapture.host_id, owner_boot_id: ownerPartAtCapture.boot_id,
      face_id: face.presentation.identity,
      face_revision: face.presentation_revision_decimal,
      route_id: routeId, route_available: routeAvailable,
      ...details,
    };
    const readyPath = path.join(checkpointDirectory, `${phase}.ready.json`);
    const temporary = `${readyPath}.tmp`;
    await writeFile(temporary, `${JSON.stringify(ready, null, 2)}\n`, { mode: 0o600 });
    await rename(temporary, readyPath);
    const resumePath = path.join(checkpointDirectory, `${phase}.resume.json`);
    const deadline = Date.now() + 600_000;
    while (!existsSync(resumePath) && Date.now() < deadline) {
      await new Promise(resolve => setTimeout(resolve, 250));
    }
    assert.ok(existsSync(resumePath), `screen-free operator did not finish ${phase}`);
    const resumeBytes = await readFile(resumePath);
    const resume = JSON.parse(resumeBytes);
    for (const key of ['schema', 'phase', 'body_id', 'owner_host_id', 'owner_boot_id',
      'route_id', 'route_available']) {
      assert.deepEqual(resume[key], ready[key], `${phase} resume ${key} differs`);
    }
    assert.equal(typeof resume.speaker_playback_selected, 'boolean');
    assert.ok(Number.isSafeInteger(resume.selected_playback_receipts) &&
      resume.selected_playback_receipts >= 0);
    if (resume.speaker_playback_selected) {
      assert.ok(resume.selected_playback_receipts > 0,
        `${phase} has no completed selected speaker Play`);
    } else {
      assert.equal(resume.selected_playback_receipts, 0,
        `${phase} text-only readout cannot claim selected speaker Plays`);
    }
    assert.ok(resume.face_id && resume.source_show_id,
      `${phase} omitted final Face or Show`);
    const transcriptPath = path.resolve(checkpointDirectory, resume.transcript?.path ?? '');
    assert.ok(transcriptPath.startsWith(`${output}${path.sep}`),
      `${phase} transcript must remain within this run`);
    const transcript = await readFile(transcriptPath);
    assert.equal(transcript.length, resume.transcript.bytes);
    assert.equal(digest(transcript), resume.transcript.sha256);
    checkpointHandoffs.push({ phase, route_id: routeId, route_available: routeAvailable,
      ready: { path: path.relative(output, readyPath), sha256: digest(await readFile(readyPath)) },
      resume: { path: path.relative(output, resumePath), sha256: digest(resumeBytes) },
      transcript: resume.transcript, source_show_id: resume.source_show_id,
      selected_playback_receipts: resume.selected_playback_receipts });
    return { ready, resume };
  };
  const observations = [];
  const observeBrowserCapture = async (name, face, cause, screenshot) => {
    observations.push(await recordBrowserCapture({ output, name,
      sourceCommit: installed.release_source_identity, runId, bodyId,
      hostId: identity.hostId, bootId: identity.bootId, face, cause, screenshot }));
  };

  await page.goto(`${server.url}?participate=owner#your-handbook`);
  await page.locator('[data-owner-key]').waitFor();
  const identity = await page.evaluate(() => globalThis.__conduitOwnerParticipation.admissionIdentity());
  const window = run(['body', 'browser-window', '--state-dir', state,
    '--expected-host-id', identity.hostId,
    '--new-host-verifying-key', JSON.stringify(identity.verifyingKey),
    '--maximum-millis', '60000', '--authorize-window']);
  assert.equal(window.body_id, bodyId);
  await page.getByLabel('Body ID').fill(bodyId);
  await page.getByLabel('Owner window URL').fill(window.url);
  await page.getByRole('button', { name: 'Join this Body' }).click();
  await page.waitForFunction(() => globalThis.__conduitOwnerParticipation?.presence() === 'available',
    null, { timeout: 12_000 });
  await page.locator('[data-owner-face-document] [data-owner-action]').first().waitFor();
  const joined = await page.evaluate(() => ({
    credential: globalThis.__conduitOwnerParticipation.credential(),
    face: globalThis.__conduitOwnerParticipation.face(),
  }));
  assert.equal(joined.credential.body_id, bodyId);
  assert.equal(joined.face.body_id, bodyId);
  assert.equal(joined.face.show_state, 'available');
  eventObservedAt.browser_join = Date.now();
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-before.png') });
  await observeBrowserCapture('browser-joined', joined.face,
    { kind: 'browser-membership', part_id: joined.credential.part_id }, 'browser-before.png');
  const wardrobeEvidence = page.locator('[data-owner-wardrobe-evidence]');
  const readWardrobe = async () => JSON.parse(await wardrobeEvidence.textContent());
  const awaitWardrobeRevision = async prior => {
    await page.waitForFunction(revision => {
      try {
        const report = JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent);
        return report.wardrobe_revision_decimal !== revision;
      } catch { return false; }
    }, prior, { timeout: 12_000 });
    return readWardrobe();
  };
  // Join the browser first: its membership changes the owner Face. The QMP
  // guest must then receive that fresh revision before it submits an action.
  nativeProof = spawn(xtask, [
    'make', 'conduitos', 'live-owner-action-proof', '--spore', spore,
    '--candidate-id', candidateId, '--owner-forward', ownerForward,
    '--output-dir', native, '--coordinate',
  ], { stdio: ['ignore', 'pipe', 'pipe'], detached: process.platform !== 'win32' });
  nativeProof.stdout.on('data', chunk => nativeOutput.push(chunk.toString()));
  nativeProof.stderr.on('data', chunk => nativeOutput.push(chunk.toString()));
  const standby = await waitForFile(path.join(native, 'native-standby.json'));
  assert.equal(standby.stage, 'standby');
  assert.equal(standby.guest_part.membership_installed, true);
  assert.equal(standby.guest_part.body_id, bodyId);
  assert.equal(standby.face.interactions_admitted, false);
  eventObservedAt.guest_join = Date.now();
  const nativeWardrobeDeadline = Date.now() + 60_000;
  let nativeWardrobeObserved = false;
  while (Date.now() < nativeWardrobeDeadline && !nativeWardrobeObserved) {
    await page.getByRole('button', { name: 'Inspect current wardrobe' }).click();
    try {
      await page.waitForFunction(() => {
        try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent)
          .route_descriptions.some(route => route.mask_name === 'native-graphical'); } catch { return false; }
      }, null, { timeout: Math.min(8_000, nativeWardrobeDeadline - Date.now()) });
      nativeWardrobeObserved = true;
    } catch (error) {
      if (error.name !== 'TimeoutError') throw error;
    }
  }
  if (!nativeWardrobeObserved) {
    const diagnostic = await page.evaluate(() => ({
      status: document.querySelector('[data-owner-wardrobe-status]')?.textContent,
      evidence: document.querySelector('[data-owner-wardrobe-evidence]')?.textContent,
      presence: globalThis.__conduitOwnerParticipation?.presence(),
    }));
    await writeFile(path.join(output, 'native-wardrobe-refusal.json'),
      `${JSON.stringify(diagnostic, null, 2)}\n`, { mode: 0o600 });
    throw new Error(`native Mask route absent after fresh wardrobe inspection: ${diagnostic.status}`);
  }
  const nativeWardrobeBefore = await readWardrobe();
  const nativeDescription = nativeWardrobeBefore.route_descriptions.find(route =>
    route.mask_name === 'native-graphical');
  const nativeRoute = nativeWardrobeBefore.admitted_routes.find(route =>
    route.route_id === nativeDescription.route_id && route.currently_available);
  assert.ok(nativeRoute, 'native Mask must have a current sealed route before user selection');
  assert.notEqual(nativeWardrobeBefore.selected?.route_id, nativeRoute.route_id);
  const initialBrowserDescription = nativeWardrobeBefore.route_descriptions.find(route =>
    route.route_id === nativeWardrobeBefore.selected?.route_id);
  assert.ok(initialBrowserDescription, 'initial browser Show needs its owner route name');
  await page.getByRole('button', { name: 'Wear native-graphical', exact: true }).click();
  const nativeWorn = await awaitWardrobeRevision(nativeWardrobeBefore.wardrobe_revision_decimal);
  assert.equal(nativeWorn.owner_plan_id, nativeWardrobeBefore.owner_plan_id);
  assert.equal(nativeWorn.selected?.route_id, nativeWardrobeBefore.selected.route_id);
  await page.getByRole('button', { name: `Doff ${initialBrowserDescription.mask_name}`, exact: true }).click();
  const browserDoffed = await awaitWardrobeRevision(nativeWorn.wardrobe_revision_decimal);
  assert.equal(browserDoffed.owner_plan_id, nativeWardrobeBefore.owner_plan_id);
  assert.equal(browserDoffed.selected?.route_id, nativeRoute.route_id);
  await page.getByRole('button', { name: 'Prefer only native-graphical', exact: true }).click();
  const nativePreferred = await awaitWardrobeRevision(browserDoffed.wardrobe_revision_decimal);
  assert.equal(nativePreferred.owner_plan_id, nativeWardrobeBefore.owner_plan_id);
  assert.equal(nativePreferred.selected?.route_id, nativeRoute.route_id);
  await writeFile(path.join(native, 'resume-native-activation'), 'continue\n');
  const arrived = await waitForFile(path.join(native, 'native-arrived.json'));
  assert.equal(arrived.stage, 'arrived');
  assert.equal(arrived.guest_part.membership_installed, true);
  assert.equal(arrived.guest_part.body_id, bodyId);
  assert.equal(arrived.face.interactions_admitted, true);
  assert.equal(arrived.show_ack.show_id, arrived.face.show_id);
  assert.equal(arrived.face.face_id, standby.face.face_id);
  assert.equal(arrived.face.face_revision, standby.face.face_revision);
  const threeHosts = run(['body', 'status', '--state-dir', state, '--json']);
  assert.equal(threeHosts.biography.membership.parts.length, 3);
  const ownerPart = threeHosts.biography.membership.parts.find(part =>
    part.part_id !== arrived.guest_part.part_id && part.part_id !== joined.credential.part_id);
  assert.ok(ownerPart?.current);
  for (const partId of [arrived.guest_part.part_id, joined.credential.part_id]) {
    assert.equal(threeHosts.biography.membership.parts.some(part =>
      part.part_id === partId && part.current !== null), true);
  }
  assert.equal(new Set(threeHosts.biography.membership.parts.map(part => part.current.host_id)).size, 3);
  assert.equal(new Set(threeHosts.biography.membership.parts.map(part => part.current.boot_id)).size, 3);
  await writeFile(path.join(native, 'resume-native-action'), 'continue\n');
  const nativeAction = await waitForFile(path.join(native, 'native-action.json'), 45_000);
  assert.equal(nativeAction.stage, 'action');
  assert.equal(nativeAction.action.status, 'accepted');
  assert.equal(nativeAction.action.requested_interval_ms, 500);
  assert.equal(nativeAction.show_ack.show_id, nativeAction.face.show_id);
  eventObservedAt.native_action = Date.now();
  await page.getByRole('button', { name: 'Inspect current wardrobe' }).click();
  await page.waitForFunction(prior => {
    try {
      const report = JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent);
      return report.face_id !== prior && report.route_descriptions.length > 0;
    } catch { return false; }
  }, nativePreferred.face_id, { timeout: 12_000 });
  const beforeBrowserRestore = await readWardrobe();
  const browserDescription = beforeBrowserRestore.route_descriptions.find(route =>
    route.host_id === identity.hostId);
  const currentBrowserRoute = beforeBrowserRestore.admitted_routes.find(route =>
    route.route_id === browserDescription?.route_id && route.currently_available);
  assert.ok(currentBrowserRoute, 'current browser route needs a sealed available witness');
  const currentNativeDescription = beforeBrowserRestore.route_descriptions.find(route =>
    route.mask_name === 'native-graphical');
  assert.ok(currentNativeDescription, 'current native route needs its owner name');
  await page.getByRole('button', { name: `Wear ${browserDescription.mask_name}`, exact: true }).click();
  const browserWorn = await awaitWardrobeRevision(beforeBrowserRestore.wardrobe_revision_decimal);
  assert.equal(browserWorn.selected?.route_id, beforeBrowserRestore.selected?.route_id);
  await page.getByRole('button', { name: `Doff ${currentNativeDescription.mask_name}`, exact: true }).click();
  const nativeDoffed = await awaitWardrobeRevision(browserWorn.wardrobe_revision_decimal);
  assert.equal(nativeDoffed.selected?.route_id, currentBrowserRoute.route_id);
  await page.getByRole('button', { name: `Prefer only ${browserDescription.mask_name}`, exact: true }).click();
  const browserRestored = await awaitWardrobeRevision(nativeDoffed.wardrobe_revision_decimal);
  assert.equal(browserRestored.selected?.route_id, currentBrowserRoute.route_id);
  await page.getByRole('button', { name: 'Refresh this Face' }).click();
  await page.waitForFunction(prior => {
    const face = globalThis.__conduitOwnerParticipation.face();
    return face && face.face_revision !== prior && face.subjects.some(subject =>
      subject.text.some(text => text.includes('500 milliseconds')));
  }, joined.face.face_revision, { timeout: 12_000 });
  const afterNative = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(afterNative.body_id, bodyId);
  const browserAction = afterNative.actions.find(action => action.intent === 'conduit.intent/change-clock-interval@1');
  assert.equal(browserAction?.availability, 'available');
  assert.equal(await page.locator('[data-handbook-application]').getAttribute('data-owner-show-acknowledged'),
    afterNative.show_id);
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-after-native.png') });
  await observeBrowserCapture('after-native-action', afterNative, {
    kind: 'conduitos-semantic-action', action_id: nativeAction.action.action_id,
    source_face_id: nativeAction.action.face_id,
    source_face_revision: nativeAction.action.face_revision,
    source_show_id: nativeAction.action.prior_show_id,
    native_result_face_id: nativeAction.face.face_id,
    native_result_face_revision: nativeAction.face.face_revision,
    native_result_show_id: nativeAction.face.show_id,
    outcome: nativeAction.action.status,
  }, 'browser-after-native.png');
  const control = page.locator('[data-owner-action]').filter({
    has: page.getByRole('button', { name: 'Change clock interval' }),
  });
  await control.getByRole('combobox').selectOption('1000');
  await control.getByRole('button', { name: 'Change clock interval' }).click();
  await page.waitForFunction(prior => {
    const face = globalThis.__conduitOwnerParticipation.face();
    return face && face.face_revision !== prior && face.subjects.some(subject =>
      subject.text.some(text => text.includes('1000 milliseconds')));
  }, afterNative.face_revision, { timeout: 12_000 });
  const afterBrowser = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  eventObservedAt.browser_action = Date.now();
  assert.equal(afterBrowser.body_id, bodyId);
  assert.notEqual(afterBrowser.face_id, afterNative.face_id);
  assert.equal(await page.locator('[data-owner-action-result]').textContent(),
    'The owner accepted Change clock interval.');
  assert.equal(await page.locator('[data-handbook-application]').getAttribute('data-owner-show-acknowledged'),
    afterBrowser.show_id);
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-after-browser.png') });
  await observeBrowserCapture('after-browser-action', afterBrowser, {
    kind: 'browser-semantic-action', action_id: browserAction.identity,
    source_face_id: afterNative.face_id, source_face_revision: afterNative.face_revision,
    source_show_id: afterNative.show_id, outcome: 'accepted',
  }, 'browser-after-browser.png');
  const stillJoined = run(['body', 'status', '--state-dir', state, '--json']);
  assert.equal(stillJoined.biography.membership.parts.length, 3);
  for (const partId of [ownerPart.part_id, arrived.guest_part.part_id, joined.credential.part_id]) {
    assert.equal(stillJoined.biography.membership.parts.some(part =>
      part.part_id === partId && part.current !== null), true);
  }
  assert.match(await readFile(path.join(state, 'body/source.conduit'), 'utf8'), /time\/every\(1000ms\)/);
  assert.deepEqual(errors, []);
  await page.getByRole('button', { name: 'Inspect current wardrobe' }).click();
  await page.waitForFunction(faceId => {
    try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent)
      .face_id === faceId; } catch { return false; }
  }, afterBrowser.face_id, { timeout: 12_000 });
  const beforeTerminal = await readWardrobe();
  const browserBeforeTerminal = beforeTerminal.route_descriptions.find(route =>
    route.host_id === identity.hostId);
  assert.equal(beforeTerminal.selected?.route_id, browserBeforeTerminal?.route_id);
  await page.getByRole('button', { name: `Doff ${browserBeforeTerminal.mask_name}`, exact: true }).click();
  const browserDoffedForTerminal = await awaitWardrobeRevision(beforeTerminal.wardrobe_revision_decimal);
  assert.equal(browserDoffedForTerminal.owner_plan_id, beforeTerminal.owner_plan_id,
    'doffing an already sealed browser route is a same-Plan wardrobe choice');
  assert.equal(browserDoffedForTerminal.selected, null,
    'native route was doffed earlier; the person must explicitly choose the terminal');
  let terminalSetupStage = 'initial';
  const terminalSetup = await runAttachedTerminal(null, (line, child) => {
    if (line.startsWith('Wardrobe refused:')) {
      throw new Error(`terminal setup refused an explicit choice: ${line}`);
    }
    const report = /^Owner Body wardrobe: (\d+) worn terminal Mask, .*selected route: ([^;]+);/.exec(line);
    if (!report) return;
    if (terminalSetupStage === 'initial') {
      // The prior native and browser Masks were explicitly doffed. The
      // installed owner may already wear this new terminal route on attach.
      assert.ok(Number(report[1]) <= 1, 'unexpected preexisting worn Mask');
      if (Number(report[1]) === 0) {
        terminalSetupStage = 'after-wear';
        child.stdin.write('wardrobe wear\n');
      } else {
        terminalSetupStage = 'after-prefer';
        child.stdin.write('wardrobe prefer\n');
      }
    } else if (terminalSetupStage === 'after-wear') {
      assert.equal(Number(report[1]), 1, 'wear did not admit the terminal Mask');
      terminalSetupStage = 'after-prefer';
      child.stdin.write('wardrobe prefer\n');
    } else if (terminalSetupStage === 'after-prefer') {
      assert.match(report[2], /^route\//, 'preference did not select the terminal route');
      terminalSetupStage = 'done';
      child.stdin.end('quit\n');
    }
  });
  await writeFile(path.join(output, 'terminal-setup.txt'), terminalSetup.stdout ?? '');
  await writeFile(path.join(output, 'terminal-setup.stderr.txt'), terminalSetup.stderr ?? '');
  assert.equal(terminalSetup.status, 0,
    `terminal setup status ${terminalSetup.status}, signal ${terminalSetup.signal}, timed out ${terminalSetup.timedOut}; stdout:\n${terminalSetup.stdout}\nstderr:\n${terminalSetup.stderr}`);
  if (terminalSetup.protocolError) throw terminalSetup.protocolError;
  assert.equal(terminalSetupStage, 'done', 'terminal wardrobe choice did not complete');
  assert.doesNotMatch(terminalSetup.stdout, /Wardrobe refused:/);
  assert.match(terminalSetup.stdout, /selected route: route\//);
  let terminalActionStage = 'inspect';
  const terminal = await runAttachedTerminal(null, (line, child) => {
    if (!line.startsWith('Owner Body wardrobe:')) return;
    if (terminalActionStage === 'inspect') {
      if (line.includes('current Show: none;')) {
        terminalActionStage = 'refresh';
        child.stdin.write('show\n');
      } else {
        terminalActionStage = 'apply';
        child.stdin.end('apply 500\nquit\n');
      }
    } else if (terminalActionStage === 'refresh') {
      assert.doesNotMatch(line, /current Show: none;/,
        'explicit same-attachment Show did not become current');
      terminalActionStage = 'apply';
      child.stdin.end('apply 500\nquit\n');
    }
  });
  await writeFile(path.join(output, 'terminal-face.txt'), terminal.stdout ?? '');
  await writeFile(path.join(output, 'terminal-action.stderr.txt'), terminal.stderr ?? '');
  assert.equal(terminal.status, 0,
    `terminal action status ${terminal.status}, signal ${terminal.signal}, timed out ${terminal.timedOut}; stdout:\n${terminal.stdout}\nstderr:\n${terminal.stderr}`);
  if (terminal.protocolError) throw terminal.protocolError;
  assert.doesNotMatch(terminal.stdout, /Action refused:/);
  const terminalShows = [...terminal.stdout.matchAll(/Owner terminal Show (\S+) · route Plan (\S+) · Host (\S+) · Boot (\S+) · offer generation (\d+) · (\d+) bytes written and flushed/g)];
  assert.equal(terminalActionStage, 'apply', 'terminal action must follow a current Show');
  assert.ok(terminalShows.length === 2 || terminalShows.length === 3,
    'owner terminal Mask must acknowledge the action Show and changed Face');
  for (const show of terminalShows) {
    assert.equal(show[3], ownerPart.current.host_id);
    assert.equal(show[4], ownerPart.current.boot_id);
    assert.ok(Number(show[6]) > 0, 'Show needs an acknowledged terminal write');
  }
  const actionShow = terminalShows.at(-2);
  const changedShow = terminalShows.at(-1);
  if (terminalShows.length === 3) {
    assert.notEqual(terminalShows[0][1], actionShow[1],
      'explicit same-attachment presentation must produce a new Show');
    assert.equal(terminalShows[0][5], actionShow[5],
      'same-attachment presentation must not change the Host offer generation');
  }
  assert.notEqual(actionShow[1], changedShow[1],
    'a new Face cannot reuse the previous terminal Show');
  assert.notEqual(actionShow[2], changedShow[2],
    'the changed Face and detached terminal offer require a replacement route Plan');
  assert.ok(Number(changedShow[5]) > Number(actionShow[5]),
    'terminal reattachment must use the fresh Host offer generation');
  assert.match(terminal.stdout, /1000 milliseconds/);
  assert.match(terminal.stdout, /500 milliseconds/);
  const terminalActionLine = terminal.stdout.split('\n').find(line =>
    line.includes('"schema":"conduit.body/clock-interval-changed@1"'));
  assert.ok(terminalActionLine, 'terminal must submit a semantic clock action');
  const terminalAction = JSON.parse(terminalActionLine.slice(terminalActionLine.indexOf('{')));
  eventObservedAt.terminal_action = Date.now();
  assert.equal(terminalAction.body_id, bodyId);
  assert.equal(terminalAction.interval_ms, 500);
  assert.equal(terminalAction.prior_show_id, actionShow[1]);
  assert.match(await readFile(path.join(state, 'body/source.conduit'), 'utf8'), /time\/every\(500ms\)/);
  const ownerAfterTerminal = run(['body', 'face', '--state-dir', state, '--json']);
  assert.equal(ownerAfterTerminal.presentation.basis.body_id, bodyId);
  await page.getByRole('button', { name: 'Inspect current wardrobe' }).click();
  await page.waitForFunction(faceId => {
    try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent)
      .face_id === faceId; } catch { return false; }
  }, ownerAfterTerminal.presentation.identity, { timeout: 12_000 });
  const afterTerminalDoff = await readWardrobe();
  assert.notEqual(afterTerminalDoff.owner_plan_id, beforeTerminal.owner_plan_id,
    'changed Face and terminal Host offers require a replacement owner Plan');
  const browserAfterTerminal = afterTerminalDoff.route_descriptions.find(route =>
    route.host_id === identity.hostId);
  assert.ok(browserAfterTerminal);
  await page.getByRole('button', { name: `Wear ${browserAfterTerminal.mask_name}`, exact: true }).click();
  const browserReworn = await awaitWardrobeRevision(afterTerminalDoff.wardrobe_revision_decimal);
  assert.equal(browserReworn.selected?.route_id, browserAfterTerminal.route_id);
  await page.getByRole('button', { name: `Prefer only ${browserAfterTerminal.mask_name}`, exact: true }).click();
  const browserPreferredAgain = await awaitWardrobeRevision(browserReworn.wardrobe_revision_decimal);
  assert.equal(browserPreferredAgain.selected?.route_id, browserAfterTerminal.route_id);
  await page.getByRole('button', { name: 'Refresh this Face' }).click();
  await page.waitForFunction(prior => {
    const face = globalThis.__conduitOwnerParticipation.face();
    return face && face.face_revision !== prior && face.subjects.some(subject =>
      subject.text.some(text => text.includes('500 milliseconds')));
  }, afterBrowser.face_revision, { timeout: 12_000 });
  const afterTerminal = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(afterTerminal.body_id, bodyId);
  assert.equal(await page.locator('[data-handbook-application]').getAttribute('data-owner-show-acknowledged'),
    afterTerminal.show_id);
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-after-terminal.png') });
  await observeBrowserCapture('after-terminal-action', afterTerminal, {
    kind: 'terminal-semantic-action', interaction_id: terminalAction.interaction_id,
    source_face_id: terminalAction.prior_face_id,
    source_face_revision: terminalAction.prior_face_revision,
    source_show_id: terminalAction.prior_show_id, outcome: 'accepted',
  }, 'browser-after-terminal.png');
  await page.getByRole('button', { name: 'Inspect current wardrobe' }).click();
  await page.waitForFunction(() => {
    if (!document.querySelector('[data-owner-wardrobe-status]').textContent
      .startsWith('Owner wardrobe revision ')) return false;
    try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent)
      .schema === 'conduit.body/owner-mask-wardrobe@1'; } catch { return false; }
  }, null, { timeout: 12_000 });
  const wardrobeBefore = await readWardrobe();
  assert.equal(wardrobeBefore.body_id, bodyId);
  const browserRoute = wardrobeBefore.admitted_routes.find(route =>
    route.currently_available && route.route_id === wardrobeBefore.selected?.route_id);
  assert.ok(browserRoute, 'the browser must be the selected available Mask before doff');
  const browserMask = wardrobeBefore.route_descriptions.find(route =>
    route.route_id === browserRoute.route_id)?.mask_name;
  assert.ok(browserMask, 'the selected browser route needs its owner name');
  await page.getByRole('button', { name: `Doff ${browserMask}`, exact: true }).click();
  const wardrobeDoffed = await awaitWardrobeRevision(wardrobeBefore.wardrobe_revision_decimal);
  assert.equal(wardrobeDoffed.owner_plan_id, wardrobeBefore.owner_plan_id);
  assert.equal(wardrobeDoffed.selected, null);
  assert.equal(wardrobeDoffed.wardrobe.worn.some(mask =>
    mask.checked_plot_id === browserRoute.mask_plot.checked_plot_id), false);
  await page.getByRole('button', { name: `Wear ${browserMask}`, exact: true }).click();
  const wardrobeWorn = await awaitWardrobeRevision(wardrobeDoffed.wardrobe_revision_decimal);
  assert.equal(wardrobeWorn.owner_plan_id, wardrobeBefore.owner_plan_id);
  await page.getByRole('button', { name: `Prefer only ${browserMask}`, exact: true }).click();
  const wardrobePreferred = await awaitWardrobeRevision(wardrobeWorn.wardrobe_revision_decimal);
  assert.equal(wardrobePreferred.owner_plan_id, wardrobeBefore.owner_plan_id);
  assert.equal(wardrobePreferred.selected?.route_id, browserRoute.route_id);
  // The owner may retain a current Show when this route is selected again.
  // Refresh below must still produce a new acknowledged Show for the action.
  await page.getByRole('button', { name: 'Refresh this Face' }).click();
  await page.waitForFunction(() => Boolean(document.querySelector('[data-handbook-application]')
    ?.dataset.ownerShowAcknowledged), null, { timeout: 12_000 });
  await page.getByRole('button', { name: 'Inspect current wardrobe' }).click();
  await page.waitForFunction(prior => {
    try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent)
      .show_id !== prior; } catch { return false; }
  }, wardrobePreferred.show_id, { timeout: 12_000 });
  const wardrobeRecovered = await readWardrobe();
  eventObservedAt.wardrobe_show = Date.now();
  assert.equal(wardrobeRecovered.owner_plan_id, wardrobeBefore.owner_plan_id);
  assert.equal(wardrobeRecovered.selected?.route_id, browserRoute.route_id);
  assert.ok(wardrobeRecovered.show_id && !wardrobeRecovered.fresh_show_required);
  const currentBrowserFace = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(currentBrowserFace.show_id, wardrobeRecovered.show_id);
  assert.equal(currentBrowserFace.face_id, afterTerminal.face_id);
  await page.locator('.owner-wardrobe').screenshot({ path: path.join(output, 'browser-wardrobe.png') });
  await observeBrowserCapture('wardrobe-restored', currentBrowserFace, {
    kind: 'owner-wardrobe-transition', owner_plan_id: wardrobeBefore.owner_plan_id,
    revision_before: wardrobeBefore.wardrobe_revision_decimal,
    revision_after: wardrobeRecovered.wardrobe_revision_decimal,
    selected_route_id: wardrobeRecovered.selected.route_id,
    selected_show_id: wardrobeRecovered.show_id,
  }, 'browser-wardrobe.png');
  const wardrobeRecord = { schema: 'conduit.proof/owner-browser-wardrobe@1',
    source_commit: installed.release_source_identity, run_id: runId, body_id: bodyId,
    browser_route_id: browserRoute.route_id,
    native_selection: { standby, before: nativeWardrobeBefore, native_worn: nativeWorn,
      browser_doffed: browserDoffed, native_preferred: nativePreferred,
      before_browser_restore: beforeBrowserRestore, browser_worn: browserWorn,
      native_doffed: nativeDoffed, browser_restored: browserRestored },
    before: wardrobeBefore, doffed: wardrobeDoffed, worn: wardrobeWorn,
    preferred: wardrobePreferred, recovered: wardrobeRecovered };
  const wardrobeBytes = Buffer.from(`${JSON.stringify(wardrobeRecord, null, 2)}\n`);
  await writeFile(path.join(output, 'browser-wardrobe.json'), wardrobeBytes);
  const afterTerminalStatus = run(['body', 'status', '--state-dir', state, '--json']);
  assert.equal(afterTerminalStatus.biography.membership.parts.length, 3);
  for (const partId of [ownerPart.part_id, arrived.guest_part.part_id, joined.credential.part_id]) {
    assert.equal(afterTerminalStatus.biography.membership.parts.some(part =>
      part.part_id === partId && part.current !== null), true);
  }
  let ownerSelectedSpeech;
  if (installed.selected_speech) {
    const receipt = await captureOwnerSelectedSpeech(page, speechObserver, {
      face: currentBrowserFace, bodyId, ownerHostId: ownerPart.current.host_id,
      ownerBootId: ownerPart.current.boot_id,
      providerSha256: installed.selected_speech.provider_sha256,
    }, path.join(output, 'owner-selected-speech-start.json'));
    const batches = await retainOwnerSpeechArtifacts(state, output, receipt.batches);
    const retained = { ...receipt, batches };
    const bytes = Buffer.from(`${JSON.stringify(retained, null, 2)}\n`);
    await writeFile(path.join(output, 'owner-selected-speech.json'), bytes);
    ownerSelectedSpeech = { ...retained, path: 'owner-selected-speech.json', sha256: digest(bytes) };
    assert.equal(run(['body', 'status', '--state-dir', state, '--json']).biography.body_id, bodyId);
  }
  let ownerDirectSpeech;
  if (installed.selected_speech) {
    const command = (verb, ...extra) => run(['body', 'spoken-mask', '--state-dir', state,
      verb, ...extra]);
    const before = run(['body', 'face', '--state-dir', state, '--json']);
    assert.equal(before.presentation.identity, currentBrowserFace.face_id);
    const admitted = command('admit');
    const selected = command('select');
    assert.equal(admitted.schema, 'conduit.body/direct-spoken-route@1');
    assert.equal(selected.schema, admitted.schema);
    assert.equal(selected.selected, true);
    const started = command('start');
    assert.equal(started.schema, 'conduit.body/direct-spoken-start@1');
    assert.equal(started.state, 'running');
    let terminal;
    const directReadingDeadline = Date.now() + 20 * 60_000;
    while (Date.now() < directReadingDeadline) {
      const status = command('status', started.operation_id);
      if (status.schema === 'conduit.body/owner-spoken-terminal@1') {
        terminal = status;
        break;
      }
      assert.equal(status.schema, 'conduit.body/owner-spoken-status@1');
      assert.equal(status.state, 'running');
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
    assert.ok(terminal, 'owner direct spoken Mask did not reach a terminal result');
    const directTerminalBytes = Buffer.from(`${JSON.stringify(terminal, null, 2)}\n`);
    await writeFile(path.join(output, 'owner-direct-spoken-terminal.json'), directTerminalBytes,
      { flag: 'wx', mode: 0o600 });
    assert.equal(terminal.outcome, 'available', terminal.detail);
    assert.equal(terminal.mode, 'direct');
    assert.equal(terminal.direct_reading_complete, true,
      terminal.speaker_playback?.detail ?? 'direct speaker reading was incomplete');
    assert.equal(terminal.speaker_played, true);
    assert.equal(terminal.route_plan_id, selected.route_plan_id);
    assert.equal(terminal.source_face_id, before.presentation.identity);
    assert.equal(terminal.mask_artifact_scope, 'opening');
    assert.equal(terminal.artifact.active_play_id, terminal.active_play_id);
    const played = terminal.speaker_playback;
    assert.equal(played.schema, 'conduit.body/selected-speech-terminal@1');
    assert.equal(played.outcome, 'completed');
    assert.equal(played.face_id, before.presentation.identity);
    assert.equal(played.face_revision_decimal, before.presentation_revision_decimal);
    assert.equal(played.source_show_id, terminal.show_id);
    assert.equal(played.host_id, ownerPart.current.host_id);
    assert.equal(played.boot_id, ownerPart.current.boot_id);
    assert.equal(played.provider_sha256, installed.selected_speech.provider_sha256);
    assert.ok(played.batches.length > 1, 'direct Face reading must span multiple speaker Plays');
    assert.equal(terminal.speaker_completed_batches, played.batches.length);
    const artifactRoot = path.join(state, 'spoken-artifacts');
    const directory = path.join(output, 'owner-direct-spoken');
    await mkdir(directory, { mode: 0o700 });
    const batches = [];
    const playIds = new Set();
    for (const batch of played.batches) {
      assert.equal(batch.outcome, 'completed');
      assert.equal(batch.provider_sha256, played.provider_sha256);
      assert.ok(batch.speaker_blocks_committed > 0 && batch.speaker_frames_committed > 0);
      assert.equal(batch.speaker_blocks_committed, batch.pcm_blocks);
      assert.equal(batch.speaker_frames_committed * 4, batch.pcm_bytes);
      assert.ok(!playIds.has(batch.play_id), 'speaker Play IDs must be unique');
      playIds.add(batch.play_id);
      assert.match(batch.wav_artifact_id, /^play-[0-9a-f]{64}\.wav$/);
      const originalWavPath = path.join(artifactRoot, batch.wav_artifact_id);
      const wav = await readFile(originalWavPath);
      const wavWrittenAtUnixMs = Math.trunc((await stat(originalWavPath)).mtimeMs);
      assert.ok(Number.isSafeInteger(wavWrittenAtUnixMs) && wavWrittenAtUnixMs > 0);
      assert.equal(wav.subarray(0, 4).toString(), 'RIFF');
      assert.equal(wav.subarray(8, 12).toString(), 'WAVE');
      assert.equal(wav.length, batch.wav_bytes);
      assert.equal(digest(wav), batch.wav_sha256);
      assert.equal(wav.length - 44, batch.pcm_bytes);
      assert.equal(digest(wav.subarray(44)), batch.pcm_sha256);
      assert.ok(wav.subarray(44).some(byte => byte !== 0), 'direct speaker WAV is silent');
      await writeFile(path.join(directory, batch.wav_artifact_id), wav, { flag: 'wx' });
      batches.push({ ...batch, observed_at_unix_ms: wavWrittenAtUnixMs,
        wav: { path: `owner-direct-spoken/${batch.wav_artifact_id}`,
        bytes: wav.length, sha256: batch.wav_sha256 } });
    }
    const after = run(['body', 'face', '--state-dir', state, '--json']);
    assert.equal(after.presentation.identity, before.presentation.identity,
      'direct Mask may not invent a new Face');
    const terminalBytes = Buffer.from(`${JSON.stringify(terminal, null, 2)}\n`);
    await writeFile(path.join(directory, 'terminal.json'), terminalBytes, { flag: 'wx' });
    ownerDirectSpeech = {
      proof_class: 'installed-owner-direct-mask-and-same-play-speaker',
      source_commit: installed.release_source_identity, run_id: runId, body_id: bodyId,
      face_id: before.presentation.identity,
      face_revision_decimal: before.presentation_revision_decimal,
      route_plan_id: selected.route_plan_id, show_id: terminal.show_id,
      mask_artifact_play_id: terminal.active_play_id,
      provider_sha256: played.provider_sha256,
      completed_segments: played.completed_segments,
      correlation_sha256: played.correlation_sha256,
      opening_wording: terminal.direct_opening_wording,
      batches, terminal: { path: 'owner-direct-spoken/terminal.json',
        sha256: digest(terminalBytes) }, human_hearing_observed: false,
    };
  }
  let directSpeech;
  if (directSpeechEnabled && !ownerDirectSpeech) {
    const directory = path.join(output, 'speech-direct');
    const speech = spawnSync(xtask, [
      'prove', 'one-body-spoken-chapter', '--mode', 'direct',
      '--conduit-bin', owner, '--state-dir', state, '--output', directory,
      '--run-id', runId, '--action-id', 'read-current-face-direct',
      '--speech-executable', speechExecutableArgument,
      '--speech-data', speechDataArgument,
      '--speech-engine', speechEngineArgument,
      '--speech-language-coverage', speechLanguageCoverageArgument,
    ], { encoding: 'utf8', timeout: 100_000 });
    assert.equal(speech.status, 0, speech.stderr || speech.stdout);
    const receiptBytes = await readFile(path.join(directory, 'speech-receipt.json'));
    const receipt = JSON.parse(receiptBytes);
    const manifestBytes = await readFile(path.join(directory, 'manifest.json'));
    const manifest = JSON.parse(manifestBytes);
    assert.equal(manifest.result, 'complete');
    assert.equal(receipt.source_commit, installed.release_source_identity);
    assert.equal(receipt.run_id, runId);
    assert.equal(receipt.body_id, bodyId);
    assert.equal(receipt.owner_host_id, ownerPart.current.host_id);
    assert.equal(receipt.owner_boot_id, ownerPart.current.boot_id);
    assert.equal(receipt.face_id, afterTerminal.face_id);
    assert.equal(receipt.face_revision_decimal, afterTerminal.face_revision);
    assert.equal(receipt.owner_snapshot_before_after_equal, true);
    assert.equal(receipt.direct_spoken_mask_show_observed, false);
    assert.equal(receipt.owner_sealed_spoken_mask_route_observed, false);
    assert.ok(receipt.batch_count > 1 && receipt.produced_pcm_bytes > 0);
    const wavs = [];
    for (let sequence = 1; sequence <= receipt.batch_count; sequence += 1) {
      const name = `direct-batch-${sequence}`;
      const batch = JSON.parse(await readFile(path.join(directory, `${name}-receipt.json`)));
      const bytes = await readFile(path.join(directory, `${name}.wav`));
      assert.equal(batch.run_id, runId);
      assert.equal(batch.body_id, bodyId);
      assert.equal(batch.wav_sha256, digest(bytes));
      wavs.push({ path: `speech-direct/${name}.wav`, bytes: bytes.length, sha256: batch.wav_sha256 });
    }
    directSpeech = {
      proof_class: receipt.proof_class,
      action_id: receipt.action_id,
      batch_count: receipt.batch_count,
      produced_pcm_bytes: receipt.produced_pcm_bytes,
      speech_receipt_sha256: digest(receiptBytes),
      speech_manifest_sha256: digest(manifestBytes),
      source_show_id: receipt.source_show_id,
      source_mask_kind: receipt.source_mask_kind,
      face_id: receipt.face_id,
      face_revision: afterTerminal.face_revision,
      playback_observed: receipt.playback_observed,
      human_hearing_observed: receipt.human_hearing_observed,
      wavs,
    };
  }
  let llmSpeech, modelRouteLoss, modelRouteRestoration;
  if (modelArgument && installed.selected_model?.model_name !== modelArgument) {
    const captured = await captureLlmChapter({
      xtask, owner, state, output, sourceCommit: installed.release_source_identity,
      runId, bodyId, ownerHostId: ownerPart.current.host_id,
      ownerBootId: ownerPart.current.boot_id, faceId: afterTerminal.face_id,
      faceRevision: afterTerminal.face_revision,
      speechExecutable: speechExecutableArgument, speechData: speechDataArgument,
      speechEngine: speechEngineArgument, speechLanguageCoverage: speechLanguageCoverageArgument, model: modelArgument,
      ollamaEndpoint: modelEndpointArgument, admittedMemoryMib: Number(modelMemoryArgument),
    });
    llmSpeech = captured.speech;
    modelRouteLoss = captured.routeLoss;
    modelRouteRestoration = captured.restoration;
  }
  const ownerLlmSpeech = modelArgument && installed.selected_model?.model_name === modelArgument
    ? await captureOwnerLlmSpeaker({ owner: run, state, output, installation: installed,
      bodyId, runId, sourceCommit: installed.release_source_identity, model: modelArgument })
    : undefined;
  if (ownerLlmSpeech) eventObservedAt.initial_model_play = Date.now();
  let modelWardrobeObservations = 0;
  const ownerModelRouteLoss = ownerLlmSpeech && ownerModelRouteControlArgument &&
    ownerModelRouteControlArgument !== '-'
    ? await captureOwnerModelRouteLoss({ owner: run, state, output, installation: installed,
      bodyId, runId, sourceCommit: installed.release_source_identity,
      model: modelArgument, controlSocket: ownerModelRouteControlArgument,
      successful: ownerLlmSpeech,
      observeWardrobe: async (routeId, available) => {
        const carrier = await page.evaluate(() => ({
          state: globalThis.__conduitOwnerParticipation.state(),
          presence: globalThis.__conduitOwnerParticipation.presence(),
          wardrobe_enabled: !document.querySelector('[data-owner-wardrobe-refresh]').disabled,
          status: document.querySelector('[data-owner-wardrobe-status]').textContent,
        }));
        await writeFile(path.join(output, 'model-wardrobe-browser-carrier.json'),
          `${JSON.stringify({ route_id: routeId, route_available: available, ...carrier }, null, 2)}\n`,
          { mode: 0o600 });
        assert.equal(carrier.presence, 'available',
          `model wardrobe needs current browser presence: ${JSON.stringify(carrier)}`);
        assert.equal(carrier.wardrobe_enabled, true,
          `model wardrobe control is not ready: ${JSON.stringify(carrier)}`);
        await page.getByRole('button', { name: 'Inspect current wardrobe' }).click();
        await page.waitForFunction(({ routeId, available }) => {
          try {
            const report = JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]')
              .textContent);
            return report.admitted_routes.some(route =>
              route.route_id === routeId && route.currently_available === available);
          } catch { return false; }
        }, { routeId, available }, { timeout: 12_000 });
        const report = await readWardrobe();
        modelWardrobeObservations += 1;
        const checkpoint = modelWardrobeObservations > 1
          ? await screenFreeCheckpoint(available ? 'model-provider-restored' : 'model-provider-unavailable',
            routeId, available, {
              wardrobe_revision: report.wardrobe_revision_decimal,
              observed_route_status: available ? 'restored' : 'provider-withdrawn',
            }) : undefined;
        if (checkpoint) {
          eventObservedAt[available ? 'model_restored' : 'model_unavailable'] = Date.now();
        }
        return { wardrobe: report, checkpoint };
      } }) : undefined;
  await page.getByRole('button', { name: 'Inspect current wardrobe' }).click();
  await page.waitForFunction(expectedShow => {
    try {
      const report = JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent);
      return report.schema === 'conduit.body/owner-mask-wardrobe@1'
        && (!expectedShow || report.show_id === expectedShow);
    } catch { return false; }
  }, ownerModelRouteLoss?.restored.show_id ?? ownerLlmSpeech?.show_id ??
    ownerDirectSpeech?.show_id, { timeout: 12_000 });
  if (ownerLlmSpeech || ownerDirectSpeech) {
    let report = await readWardrobe();
    const browserDescription = report.route_descriptions.find(route =>
      route.host_id === identity.hostId);
    assert.ok(browserDescription, 'owner model handoff needs the browser Mask route');
    const browserRoute = report.admitted_routes.find(route =>
      route.route_id === browserDescription.route_id && route.currently_available);
    assert.ok(browserRoute, 'owner model handoff needs an available browser Mask');
    const browserWorn = report.wardrobe.worn.some(plot =>
      plot.checked_plot_id === browserRoute.mask_plot.checked_plot_id);
    if (!browserWorn) {
      await page.getByRole('button', { name: `Wear ${browserDescription.mask_name}`, exact: true }).click();
      report = await awaitWardrobeRevision(report.wardrobe_revision_decimal);
    }
    const browserPreferredAlone = report.wardrobe.preference.length === 1 &&
      report.wardrobe.preference[0].checked_plot_id === browserRoute.mask_plot.checked_plot_id;
    if (report.selected?.route_id !== browserRoute.route_id && !browserPreferredAlone) {
      await page.getByRole('button', { name: `Prefer only ${browserDescription.mask_name}`, exact: true }).click();
      report = await awaitWardrobeRevision(report.wardrobe_revision_decimal);
    }
    // Preference alone retains a current Show. Doff each selected local
    // Mask through real owner actions until the preferred browser takes over.
    for (let remaining = report.admitted_routes.length;
      report.selected?.route_id !== browserRoute.route_id && remaining > 0; remaining--) {
      const selectedDescription = report.route_descriptions.find(route =>
        route.route_id === report.selected?.route_id);
      assert.ok(selectedDescription, 'owner model handoff needs the selected Mask name');
      await page.getByRole('button', { name: `Doff ${selectedDescription.mask_name}`, exact: true }).click();
      report = await awaitWardrobeRevision(report.wardrobe_revision_decimal);
    }
    assert.equal(report.selected?.route_id, browserRoute.route_id);
  }
  const ownerBeforeBrowserReturn = run(['body', 'face', '--state-dir', state, '--json']);
  assert.equal(ownerBeforeBrowserReturn.presentation.basis.body_id, bodyId);
  await page.getByRole('button', { name: 'Refresh this Face' }).click();
  await page.waitForFunction(faceId => {
    const face = globalThis.__conduitOwnerParticipation.face();
    return face?.face_id === faceId && face.show_state === 'available'
      && document.querySelector('[data-handbook-application]')
        ?.dataset.ownerShowAcknowledged === face.show_id;
  }, ownerBeforeBrowserReturn.presentation.identity, { timeout: 12_000 });
  await page.getByRole('button', { name: 'Inspect current wardrobe' }).click();
  await page.waitForFunction(() => {
    try {
      const report = JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent);
      return report.schema === 'conduit.body/owner-mask-wardrobe@1'
        && report.show_id === globalThis.__conduitOwnerParticipation.face()?.show_id;
    } catch { return false; }
  }, null, { timeout: 12_000 });
  const presentationRecovery = await capturePresentationRecovery({
    page, context, serverUrl: server.url, owner: run, state, bodyId,
    ownerPartId: ownerPart.part_id, guestPartId: arrived.guest_part.part_id,
    browserCredential: joined.credential, browserBootId: identity.bootId,
    initialOwnerWindowUrl: window.url,
    oldFace: await page.evaluate(() => globalThis.__conduitOwnerParticipation.face()),
    oldWardrobe: await readWardrobe(), output,
    sourceCommit: installed.release_source_identity, runId,
    observeTransition: async ({ phase, routeId, routeAvailable, wardrobe, partId, bootId }) => {
      await screenFreeCheckpoint(phase, routeId, routeAvailable, {
        wardrobe_revision: wardrobe?.wardrobe_revision_decimal,
        observed_route_status: routeAvailable ? 'restored' : 'browser-part-retired',
        part_id: partId, observed_boot_id: bootId,
      });
      eventObservedAt[phase === 'browser-presentation-unavailable'
        ? 'browser_unavailable' : 'browser_restored_checkpoint'] = Date.now();
    },
  });
  eventObservedAt.browser_return = Date.now();
  observations.push(presentationRecovery.observation);
  await writeFile(path.join(native, 'resume-native-finish'), 'continue\n');
  const nativeReceipt = await waitForFile(path.join(native, 'owner-action-proof.json'), 15_000);
  assert.equal(nativeReceipt.coordinated, true);
  assert.equal(installed.release_source_identity, nativeReceipt.source_commit);
  assert.equal(nativeReceipt.guest_part.body_id, bodyId);
  assert.equal(nativeReceipt.qemu_alive_at_capture, true);
  assert.equal(nativeReceipt.face_after.show_id, nativeAction.face.show_id);
  assert.equal(nativeReceipt.action.status, 'accepted');
  const packageManifest = await readFile(path.join(handbook, 'application.application.json'));
  const browserBundle = JSON.parse(await readFile(path.join(handbook, 'sdk/bundle/conduit-browser-image.json')));
  assert.equal(browserBundle.reviewed_distribution.source_commit, nativeReceipt.source_commit);
  const screenshotPaths = ['browser-before.png', 'browser-after-native.png', 'browser-after-browser.png',
    'browser-after-terminal.png', 'browser-wardrobe.png', 'browser-after-recovery.png',
    'native/owner-standby.png', 'native/owner-before.png', 'native/owner-after.png'];
  const screenshots = await Promise.all(screenshotPaths.map(async file => {
    const bytes = await readFile(path.join(output, file));
    return { path: file, bytes: bytes.length, sha256: digest(bytes) };
  }));
  assert.equal(observations.length, 6, 'every browser capture needs its action-time observation');
  for (const retained of observations) {
    const bytes = await readFile(path.join(output, retained.path));
    assert.equal(bytes.length, retained.bytes);
    assert.equal(digest(bytes), retained.sha256);
    const capture = JSON.parse(bytes).capture;
    assert.equal(capture.sha256, screenshots.find(item => item.path === capture.path)?.sha256,
      `browser capture changed after ${retained.path}`);
  }
  const nativeReceiptBytes = await readFile(path.join(native, 'owner-action-proof.json'));
  if (process.env.CONDUIT_SCREEN_FREE_CHECKPOINTS === '1') {
    const expected = ownerLlmSpeech && ownerModelRouteControlArgument &&
      ownerModelRouteControlArgument !== '-'
      ? ['model-provider-unavailable', 'model-provider-restored',
        'browser-presentation-unavailable', 'browser-presentation-restored']
      : ['browser-presentation-unavailable', 'browser-presentation-restored'];
    assert.deepEqual(checkpointHandoffs.map(item => item.phase), expected,
      'every held route transition needs a completed screen-free operator checkpoint');
  }
  const report = {
    schema: 'conduit.body/three-host-owner-journey@1',
    proof_class: ownerLlmSpeech
      ? 'live-local-installed-owner-qmp-pinned-chromium-selected-model-speaker'
      : ownerDirectSpeech
      ? 'live-local-installed-owner-qmp-pinned-chromium-direct-mask-speaker'
      : llmSpeech
      ? 'live-local-installed-owner-qmp-pinned-chromium-direct-and-llm-speech-route-loss-restoration'
      : directSpeech
        ? 'live-local-installed-owner-qmp-pinned-chromium-direct-speech'
        : 'live-local-installed-owner-qmp-pinned-chromium',
    run_id: runId,
    body_id: bodyId,
    owner_host_id: ownerPart.current.host_id,
    owner_boot_id: ownerPart.current.boot_id,
    guest_host_id: arrived.guest_part.host_id,
    guest_boot_id: arrived.guest_part.boot_id,
    guest_part_id: arrived.guest_part.part_id,
    browser_host_id: identity.hostId,
    browser_boot_id: identity.bootId,
    browser_part_id: joined.credential.part_id,
    native_source_commit: nativeReceipt.source_commit,
    native_receipt_sha256: digest(nativeReceiptBytes),
    handbook_manifest_sha256: digest(packageManifest),
    before_browser_face_id: joined.face.face_id,
    after_native_browser_face_id: afterNative.face_id,
    after_browser_face_id: afterBrowser.face_id,
    after_terminal_browser_face_id: afterTerminal.face_id,
    native_action: {
      action_id: nativeReceipt.action.action_id,
      status: nativeReceipt.action.status,
      requested_interval_ms: nativeReceipt.action.requested_interval_ms,
    },
    browser_action: { action_id: browserAction.identity, status: 'accepted', requested_interval_ms: 1000 },
    terminal_action: {
      interaction_id: terminalAction.interaction_id,
      status: 'accepted',
      requested_interval_ms: terminalAction.interval_ms,
      next_step: terminalAction.next_step,
    },
    terminal_show: {
      owner_plan_id_before: beforeTerminal.owner_plan_id,
      owner_plan_id_after: afterTerminalDoff.owner_plan_id,
      setup_path: 'terminal-setup.txt',
      setup_sha256: digest(Buffer.from(terminalSetup.stdout)),
      show_id_before: actionShow[1],
      route_plan_id_before: actionShow[2],
      offer_generation_before: Number(actionShow[5]),
      show_id_after: changedShow[1],
      route_plan_id_after: changedShow[2],
      offer_generation_after: Number(changedShow[5]),
      initial_show_id: terminalShows[0][1],
      explicit_show_refresh: terminalShows.length === 3,
      host_id: terminalShows[0][3],
      boot_id: terminalShows[0][4],
      path: 'terminal-face.txt',
      bytes: Buffer.byteLength(terminal.stdout),
      sha256: digest(Buffer.from(terminal.stdout)),
    },
    browser_wardrobe: {
      route_id: browserRoute.route_id, owner_plan_id: wardrobeBefore.owner_plan_id,
      revision_before: wardrobeBefore.wardrobe_revision_decimal,
      revision_after: wardrobeRecovered.wardrobe_revision_decimal,
      selected_show_id: wardrobeRecovered.show_id,
      path: 'browser-wardrobe.json', bytes: wardrobeBytes.length,
      sha256: digest(wardrobeBytes),
    },
    presentation_host_recovery: {
      path: 'browser-presentation-recovery.json',
      bytes: presentationRecovery.bytes.length,
      sha256: digest(presentationRecovery.bytes),
      lost_boot_id: presentationRecovery.receipt.lost_browser_boot_id,
      recovered_boot_id: presentationRecovery.receipt.recovered_browser_boot_id,
      old_show_id: presentationRecovery.receipt.old_show_id,
      recovered_show_id: presentationRecovery.receipt.recovered_show_id,
    },
    ...(directSpeech ? { direct_speech: directSpeech } : {}),
    ...(ownerSelectedSpeech ? { owner_selected_speech: ownerSelectedSpeech } : {}),
    ...(ownerDirectSpeech ? { owner_direct_speech: ownerDirectSpeech } : {}),
    ...(ownerLlmSpeech ? { owner_llm_speech: ownerLlmSpeech } : {}),
    ...(ownerModelRouteLoss ? { owner_model_route_loss: ownerModelRouteLoss } : {}),
    ...(checkpointHandoffs.length ? { screen_free_checkpoint_handoffs: checkpointHandoffs } : {}),
    ...(llmSpeech ? { llm_speech: llmSpeech, model_route_loss: modelRouteLoss,
      model_route_restoration: modelRouteRestoration } : {}),
    screenshots,
    observations,
    event_observed_at_unix_ms: eventObservedAt,
    concurrent_part_count: threeHosts.biography.membership.parts.length,
    qemu_alive_through_browser_actions: true,
  };
  const walkthrough = await writeThreeHostWalkthrough(output, handbook, report);
  report.walkthrough = {
    ...walkthrough,
    sha256: digest(await readFile(path.join(output, walkthrough.path))),
    assets: await Promise.all(['conduit.css', 'chrome.css'].map(async file => ({
      path: file,
      sha256: digest(await readFile(path.join(output, file))),
    }))),
  };
  await writeFile(path.join(output, 'report.json'), `${JSON.stringify(report, null, 2)}\n`);
  await verifyWalkthroughAssets(output, await readFile(path.join(output, walkthrough.path), 'utf8'));
  console.log(`Three-host journey proof: ${path.join(output, 'report.json')}`);
} finally {
  speechObserver?.close();
  if (nativeProof?.pid) {
    try {
      if (process.platform === 'win32') nativeProof.kill();
      else process.kill(-nativeProof.pid, 'SIGTERM');
    } catch (error) { if (error.code !== 'ESRCH') throw error; }
  }
  await browser?.close();
  if (server) server.child.kill();
  await writeFile(path.join(output, 'native-process.log'), nativeOutput.join(''));
}
