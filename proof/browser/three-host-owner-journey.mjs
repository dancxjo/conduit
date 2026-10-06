// One installed owner, one live QMP guest, and one pinned Chromium Host.
// The driver coordinates user actions; each participant creates its own
// membership, Mask Play, Show, and semantic return through product entrances.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { startStaticProduct } from './static-product-server.mjs';
import { captureLlmChapter } from './three-host-llm-chapter.mjs';
import { captureRunId } from './three-host-run-identity.mjs';
import { captureOwnerSelectedSpeech, observeOwnerSpeech } from './three-host-owner-speech.mjs';
import { retainOwnerSpeechArtifacts } from './three-host-owner-speech-artifacts.mjs';
import { writeThreeHostWalkthrough } from './three-host-walkthrough.mjs';

const [xtaskArgument, ownerArgument, stateArgument, handbookArgument, sporeArgument,
  candidateId, ownerForward, outputArgument, playwrightArgument,
  speechExecutableArgument, speechDataArgument, speechEngineArgument, speechLanguageCoverageArgument,
  modelArgument, modelEndpointArgument, modelMemoryArgument] = process.argv.slice(2);
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
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-before.png') });
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
  ], { stdio: ['ignore', 'pipe', 'pipe'] });
  nativeProof.stdout.on('data', chunk => nativeOutput.push(chunk.toString()));
  nativeProof.stderr.on('data', chunk => nativeOutput.push(chunk.toString()));
  const standby = await waitForFile(path.join(native, 'native-standby.json'));
  assert.equal(standby.stage, 'standby');
  assert.equal(standby.guest_part.membership_installed, true);
  assert.equal(standby.guest_part.body_id, bodyId);
  assert.equal(standby.face.interactions_admitted, false);
  await page.getByRole('button', { name: 'Inspect current wardrobe' }).click();
  await page.waitForFunction(() => {
    try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent)
      .route_descriptions.some(route => route.mask_name === 'native-graphical'); } catch { return false; }
  }, null, { timeout: 12_000 });
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
  assert.equal(afterBrowser.body_id, bodyId);
  assert.notEqual(afterBrowser.face_id, afterNative.face_id);
  assert.equal(await page.locator('[data-owner-action-result]').textContent(),
    'The owner accepted Change clock interval.');
  assert.equal(await page.locator('[data-handbook-application]').getAttribute('data-owner-show-acknowledged'),
    afterBrowser.show_id);
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-after-browser.png') });
  const stillJoined = run(['body', 'status', '--state-dir', state, '--json']);
  assert.equal(stillJoined.biography.membership.parts.length, 3);
  for (const partId of [ownerPart.part_id, arrived.guest_part.part_id, joined.credential.part_id]) {
    assert.equal(stillJoined.biography.membership.parts.some(part =>
      part.part_id === partId && part.current !== null), true);
  }
  assert.match(await readFile(path.join(state, 'body/source.conduit'), 'utf8'), /time\/every\(1000ms\)/);
  assert.deepEqual(errors, []);
  const terminal = spawnSync(owner, ['body', 'terminal', '--state-dir', state], {
    // The lulled Face exposes Wake first, then the checked clock argument.
    input: 'inspect\ncontrol next\ncontrol next\ntype 500\napply\nquit\n', encoding: 'utf8', timeout: 10_000,
  });
  assert.equal(terminal.status, 0, terminal.stderr);
  const terminalShows = [...terminal.stdout.matchAll(/Owner Face revision (\d+) · Show (\S+) · Host (\S+) · Boot (\S+)/g)];
  assert.ok(terminalShows.length >= 2, 'terminal Mask must acknowledge both Faces and Shows');
  for (const show of terminalShows) {
    assert.equal(show[3], ownerPart.current.host_id);
    assert.equal(show[4], ownerPart.current.boot_id);
  }
  assert.match(terminal.stdout, /1000 milliseconds/);
  assert.match(terminal.stdout, /500 milliseconds/);
  const terminalActionLine = terminal.stdout.split('\n').find(line =>
    line.includes('"schema":"conduit.body/clock-interval-changed@1"'));
  assert.ok(terminalActionLine, 'terminal must submit a semantic clock action');
  const terminalAction = JSON.parse(terminalActionLine.slice(terminalActionLine.indexOf('{')));
  assert.equal(terminalAction.body_id, bodyId);
  assert.equal(terminalAction.interval_ms, 500);
  assert.equal(terminalAction.prior_show_id, terminalShows.at(-2)[2]);
  assert.match(await readFile(path.join(state, 'body/source.conduit'), 'utf8'), /time\/every\(500ms\)/);
  await writeFile(path.join(output, 'terminal-face.txt'), terminal.stdout);
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
  assert.equal(wardrobeRecovered.owner_plan_id, wardrobeBefore.owner_plan_id);
  assert.equal(wardrobeRecovered.selected?.route_id, browserRoute.route_id);
  assert.ok(wardrobeRecovered.show_id && !wardrobeRecovered.fresh_show_required);
  const currentBrowserFace = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(currentBrowserFace.show_id, wardrobeRecovered.show_id);
  assert.equal(currentBrowserFace.face_id, afterTerminal.face_id);
  await page.locator('.owner-wardrobe').screenshot({ path: path.join(output, 'browser-wardrobe.png') });
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
    });
    const batches = await retainOwnerSpeechArtifacts(state, output, receipt.batches);
    const retained = { ...receipt, batches };
    const bytes = Buffer.from(`${JSON.stringify(retained, null, 2)}\n`);
    await writeFile(path.join(output, 'owner-selected-speech.json'), bytes);
    ownerSelectedSpeech = { ...retained, path: 'owner-selected-speech.json', sha256: digest(bytes) };
    assert.equal(run(['body', 'status', '--state-dir', state, '--json']).biography.body_id, bodyId);
  }
  let directSpeech;
  if (directSpeechEnabled) {
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
    assert.equal(receipt.face_revision.toString(), afterTerminal.face_revision);
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
  if (modelArgument) {
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
    'browser-after-terminal.png', 'browser-wardrobe.png',
    'native/owner-standby.png', 'native/owner-before.png', 'native/owner-after.png'];
  const screenshots = await Promise.all(screenshotPaths.map(async file => {
    const bytes = await readFile(path.join(output, file));
    return { path: file, bytes: bytes.length, sha256: digest(bytes) };
  }));
  const nativeReceiptBytes = await readFile(path.join(native, 'owner-action-proof.json'));
  const report = {
    schema: 'conduit.body/three-host-owner-journey@1',
    proof_class: llmSpeech
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
      face_revision_before: terminalShows[0][1],
      show_id_before: terminalShows[0][2],
      face_revision_after: terminalShows.at(-1)[1],
      show_id_after: terminalShows.at(-1)[2],
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
    ...(directSpeech ? { direct_speech: directSpeech } : {}),
    ...(ownerSelectedSpeech ? { owner_selected_speech: ownerSelectedSpeech } : {}),
    ...(llmSpeech ? { llm_speech: llmSpeech, model_route_loss: modelRouteLoss,
      model_route_restoration: modelRouteRestoration } : {}),
    screenshots,
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
  console.log(`Three-host journey proof: ${path.join(output, 'report.json')}`);
} finally {
  speechObserver?.close();
  if (nativeProof?.exitCode === null) nativeProof.kill();
  await browser?.close();
  if (server) server.child.kill();
  await writeFile(path.join(output, 'native-process.log'), nativeOutput.join(''));
}
