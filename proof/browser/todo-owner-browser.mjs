// Live browser Todo action against one already running installed Body owner.
// The owner and Handbook package are supplied by the caller; this driver never
// creates a second Body or constructs Face state for the browser.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { startStaticProduct } from './static-product-server.mjs';
import { observedTodoMutation } from './todo-owner-events.mjs';
import { captureDirectTodoSpeech } from './todo-owner-spoken.mjs';

const [ownerCwdArg, binaryArg, stateArg, handbookArg, outputArg, playwrightArg,
  itemText = 'Pick up prescription', scenario = 'first-add'] = process.argv.slice(2);
assert.ok(['first-add', 'cross-mask', 'cross-mask-speech'].includes(scenario), 'unknown Todo capture scenario');
if (!ownerCwdArg || !binaryArg || !stateArg || !handbookArg || !outputArg || !playwrightArg) {
  throw new Error('usage: todo-owner-browser.mjs OWNER-CWD INSTALLED-OWNER OWNER-STATE HANDBOOK-PACKAGE NEW-EVIDENCE-DIR PINNED-PLAYWRIGHT [NEW-ITEM-TEXT]');
}
const ownerCwd = path.resolve(ownerCwdArg);
const binary = path.resolve(ownerCwd, binaryArg);
const state = path.resolve(ownerCwd, stateArg);
const handbook = path.resolve(handbookArg);
const output = path.resolve(outputArg);
assert.equal(existsSync(output), false, 'Todo evidence directory must be new');
assert.ok(itemText && Buffer.byteLength(itemText, 'utf8') <= 64, 'Todo text must fit its admitted bound');
const installed = JSON.parse(await readFile(path.join(state, 'installation.json')));
assert.equal(path.resolve(ownerCwd, installed.product_executable), binary,
  'use the owner installed for this Body');
if (scenario === 'cross-mask-speech') assert.equal(installed.selected_speech?.artifact_only, true,
  'direct speech capture requires explicit selected artifact output');
const packageManifest = JSON.parse(await readFile(path.join(handbook, 'application.application.json')));
const browserBundle = JSON.parse(await readFile(path.join(handbook, 'sdk/bundle/browser-bundle-release.json')));
const uiResource = packageManifest.resources.find(resource => resource.role === 'owner-participation');
const digest = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
assert.equal(digest(await readFile(path.join(ownerCwd, 'targets/browser/handbook/owner-participation.mjs'))),
  uiResource.sha256, 'packaged Handbook UI differs from this source checkout');
const head = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: ownerCwd, encoding: 'utf8' });
assert.equal(head.status, 0, head.stderr);
const handbookUiSource = head.stdout.trim();
const uiStatus = spawnSync('git', ['status', '--porcelain', '--', 'targets/browser/handbook'],
  { cwd: ownerCwd, encoding: 'utf8' });
assert.equal(uiStatus.status, 0, uiStatus.stderr);
const handbookUiSourceClean = uiStatus.stdout.length === 0;
const sourceRecord = {
  owner_source_commit: installed.release_source_identity,
  browser_runtime_source_commit: browserBundle.source_identity,
  handbook_ui_source_commit: handbookUiSource,
  handbook_ui_source_clean: handbookUiSourceClean,
  handbook_package_digest: packageManifest.package_digest,
  source_relation: installed.release_source_identity === browserBundle.source_identity
    && installed.release_source_identity === handbookUiSource && handbookUiSourceClean
    ? 'exact-source' : 'development-cross-source',
};
const expectedPlaywright = JSON.parse(await readFile(new URL('./package.json', import.meta.url)))
  .devDependencies['@playwright/test'];
const actualPlaywright = JSON.parse(await readFile(path.join(path.dirname(playwrightArg), 'package.json'))).version;
assert.equal(actualPlaywright, expectedPlaywright, 'use pinned Playwright');
const { chromium } = await import(pathToFileURL(playwrightArg));
const owner = args => {
  const result = spawnSync(binary, args, { cwd: ownerCwd, encoding: 'utf8', timeout: 10_000 });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
};
const face = () => {
  const value = owner(['body', 'face', '--state-dir', state, '--json']);
  // Keep the actual semantic Face and exact revision. Repeated whole Host offer
  // catalogues are outside this capture and can exceed the publication bound.
  return { schema: value.schema, presentation: value.presentation,
    presentation_revision_decimal: value.presentation_revision_decimal,
    advertisement: { host_id: value.advertisement.host_id, boot_id: value.advertisement.boot_id } };
};
const beforeOwner = face();
const bodyId = beforeOwner.presentation.basis.body_id;
await mkdir(output, { mode: 0o700 });
let server, browser, page;
const errors = [];
try {
  server = await startStaticProduct(handbook);
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  page = await context.newPage();
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(`${server.url}?participate=owner#your-handbook`);
  await page.locator('[data-owner-key]').waitFor();
  const identity = await page.evaluate(() => globalThis.__conduitOwnerParticipation.admissionIdentity());
  const window = owner(['body', 'browser-window', '--state-dir', state,
    '--expected-host-id', identity.hostId,
    '--new-host-verifying-key', JSON.stringify(identity.verifyingKey),
    '--maximum-millis', '60000', '--authorize-window']);
  assert.equal(window.body_id, bodyId);
  await page.getByLabel('Body ID').fill(bodyId);
  await page.getByLabel('Owner window URL').fill(window.url);
  await page.getByRole('button', { name: 'Join this Body' }).click();
  await page.waitForFunction(() => globalThis.__conduitOwnerParticipation?.presence() === 'available',
    null, { timeout: 12_000 });
  await page.getByRole('button', { name: 'Inspect current wardrobe' }).click();
  const wardrobe = async () => JSON.parse(await page.locator('[data-owner-wardrobe-evidence]').textContent());
  const wardrobeIdle = () => page.waitForFunction(() =>
    document.querySelector('[data-owner-wardrobe-refresh]')?.disabled === false,
  null, { timeout: 12_000 });
  await page.waitForFunction(hostId => {
    try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent)
      .route_descriptions.some(route => route.host_id === hostId); }
    catch { return false; }
  }, identity.hostId, { timeout: 12_000 });
  await wardrobeIdle();
  let currentWardrobe = await wardrobe();
  const description = currentWardrobe.route_descriptions.find(route => route.host_id === identity.hostId);
  assert.ok(description, 'owner must describe this browser Mask route');
  const changeWardrobe = async (verb) => {
    const previous = currentWardrobe.wardrobe_revision_decimal;
    await page.getByRole('button', { name: `${verb} ${description.mask_name}`, exact: true }).click();
    await page.waitForFunction(prior => {
      try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent)
        .wardrobe_revision_decimal !== prior; } catch { return false; }
    }, previous, { timeout: 12_000 });
    await wardrobeIdle();
    currentWardrobe = await wardrobe();
  };
  if (await page.getByRole('button', { name: `Wear ${description.mask_name}`, exact: true }).isEnabled()) {
    await changeWardrobe('Wear');
  }
  if (await page.getByRole('button', { name: `Prefer only ${description.mask_name}`, exact: true }).isEnabled()) {
    await changeWardrobe('Prefer only');
  }
  assert.equal(currentWardrobe.selected?.route_id, description.route_id,
    'browser Mask must be selected before the Todo Play');
  const beforeStart = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  owner(['body', 'start', '--state-dir', state, '--maximum-millis', '60000',
    '--todo-new-list', 'Groceries']);
  let startedFace;
  for (let attempt = 0; attempt < 100; attempt++) {
    startedFace = face();
    if (startedFace.presentation.actions.some(action => action.identity === 'todo.add'
      && action.availability === 'Available')) break;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  assert.ok(startedFace.presentation.actions.some(action => action.identity === 'todo.add'
    && action.availability === 'Available'), 'Todo Play did not admit the add action');
  assert.ok(!startedFace.presentation.subjects.some(subject => subject.name === itemText),
    'new Todo item must be absent before this browser action');
  await page.getByRole('button', { name: 'Refresh this Face' }).click();
  await page.waitForFunction(prior => {
    const current = globalThis.__conduitOwnerParticipation.face();
    return current?.face_revision !== prior && current?.actions.some(action => action.identity === 'todo.add');
  }, beforeStart.face_revision, { timeout: 12_000 });
  const prepared = await page.evaluate(() => ({
    face: globalThis.__conduitOwnerParticipation.face(),
    status: document.querySelector('[data-owner-face-status]')?.textContent,
    action: document.querySelector('[data-owner-action="todo.add"]')?.outerHTML,
  }));
  if (!prepared.face.interactions_admitted || prepared.face.show_state !== 'available') {
    await writeFile(path.join(output, 'diagnostic.json'), `${JSON.stringify({
      ...sourceRecord, ...prepared,
    }, null, 2)}\n`);
    await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-route-diagnostic.png') });
    throw new Error(`Todo browser Show has no admitted action return: ${prepared.status}`);
  }
  const before = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(before.body_id, bodyId);
  assert.equal(before.show_state, 'available');
  assert.equal(await page.locator('.owner-face-collection').count(), 1);
  assert.equal(await page.locator('.owner-face-collection-count').count(), 1);
  assert.equal(await page.locator('[data-owner-face-document] [data-face-role="Status"]').count(), 0,
    'matching progress wording must not appear twice');
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-before.png') });
  const texts = scenario.startsWith('cross-mask') ? [itemText, 'Prepare lunch', 'Water plants'] : [itemText];
  assert.equal(new Set(texts).size, texts.length, 'scenario item names must be distinct');
  const adds = [];
  let after;
  for (const text of texts) {
    const prior = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
    const priorOwner = face();
    assert.equal(priorOwner.presentation.identity, prior.face_id);
    const exactPrior = { ...prior, face_revision_decimal: priorOwner.presentation_revision_decimal };
    const add = page.locator('[data-owner-action="todo.add"]');
    await add.getByRole('textbox', { name: 'Item text' }).fill(text);
    await add.getByRole('button', { name: 'add an item' }).click();
    await page.waitForFunction(({ revision, name }) => {
      const current = globalThis.__conduitOwnerParticipation.face();
      return current?.face_revision !== revision && current?.subjects.some(subject => subject.name === name)
        && document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === current.show_id;
    }, { revision: prior.face_revision, name: text }, { timeout: 12_000 });
    after = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
    assert.equal(after.body_id, bodyId);
    assert.notEqual(after.show_id, prior.show_id);
    const ownerAfterAdd = face();
    assert.equal(ownerAfterAdd.presentation.identity, after.face_id);
    const mutation = await observedTodoMutation(state, output, `add-${adds.length + 1}`,
      bodyId, exactPrior, ownerAfterAdd, 'todo.add');
    const screenshot = `browser-add-${adds.length + 1}.png`;
    await page.locator('[data-owner-face]').screenshot({ path: path.join(output, screenshot) });
    adds.push({ action_id: 'todo.add', text, before: exactPrior, after, owner: ownerAfterAdd,
      mutation, screenshot });
    await writeFile(path.join(output, 'adds.json'), `${JSON.stringify(adds, null, 2)}\n`);
  }
  let crossMask = null;
  const speechBeforeCompletion = scenario === 'cross-mask-speech'
    ? await captureDirectTodoSpeech({ owner, state, output: path.join(output, 'spoken-before-complete'),
      bodyId, readRemaining: false }) : null;
  if (speechBeforeCompletion) {
    assert.match(speechBeforeCompletion.opening.direct_opening_wording, /3 things left/);
    assert.equal(speechBeforeCompletion.remaining, null);
  }
  if (scenario.startsWith('cross-mask')) {
    // Join this Body in a terminal without a Todo mutation before completing it.
    const joinInput = 'wardrobe wear\nwardrobe prefer\nshow\nevidence\nquit\n';
    const joinBefore = face();
    const join = spawnSync(binary, ['body', 'terminal', '--owner-show', '--state-dir', state],
      { cwd: ownerCwd, encoding: 'utf8', input: joinInput, timeout: 30_000, maxBuffer: 512 * 1024 });
    await writeFile(path.join(output, 'terminal-join.input'), joinInput);
    await writeFile(path.join(output, 'terminal-join.stdout'), join.stdout ?? '');
    await writeFile(path.join(output, 'terminal-join.stderr'), join.stderr ?? '');
    assert.equal(join.status, 0, join.stderr);
    assert.match(join.stdout, /Owner terminal Show show\/[a-f0-9]+/);
    const ownerBefore = face();
    const canonicalTodo = value => value.presentation.properties.filter(property => property.subject.startsWith('todo/'));
    assert.deepEqual(canonicalTodo(ownerBefore), canonicalTodo(joinBefore), 'terminal join must not mutate the list');
    const terminalEvidence = text => text.split(/\r?\n/).filter(line => line.startsWith('Owner terminal evidence '))
      .map(line => JSON.parse(line.slice('Owner terminal evidence '.length)));
    const joinEvidence = terminalEvidence(join.stdout).at(-1);
    assert.equal(joinEvidence?.schema, 'conduit.body/terminal-show-evidence@1');
    assert.equal(joinEvidence.body_id, bodyId);
    assert.equal(joinEvidence.show_state, 'available');
    assert.equal(joinEvidence.owner_selected_show_current, true);
    const joined = { observed_at_unix_ms: Date.now(), body_id: bodyId,
      evidence: joinEvidence,
      observed_owner_face_id: ownerBefore.presentation.identity,
      observed_owner_face_revision: ownerBefore.presentation_revision_decimal,
      acknowledged_show_id: [...join.stdout.matchAll(/Owner terminal Show (show\/[a-f0-9]+)/g)].at(-1)[1],
      transcript: 'terminal-join.stdout', todo_mutation: false };

    assert.equal(ownerBefore.presentation.subjects.filter(subject => subject.role === 'Item').length, 3);
    const item = ownerBefore.presentation.subjects.find(subject => subject.role === 'Item' && subject.name === itemText);
    assert.ok(item, 'browser-added item must exist in the canonical Face');
    const action = ownerBefore.presentation.actions.find(action => action.intent === 'todo/complete@1'
      && action.target === item.identity && action.availability === 'Available');
    assert.ok(action, 'canonical Face must offer the exact item completion');
    const input = `wardrobe wear\nwardrobe prefer\nshow\nevidence\nactions\naction ${action.identity}\nevidence\nquit\n`;
    await writeFile(path.join(output, 'terminal-complete.input'), input);
    const terminal = spawnSync(binary, ['body', 'terminal', '--owner-show', '--state-dir', state],
      { cwd: ownerCwd, encoding: 'utf8', input, timeout: 30_000, maxBuffer: 512 * 1024 });
    await writeFile(path.join(output, 'terminal-complete.stdout'), terminal.stdout ?? '');
    await writeFile(path.join(output, 'terminal-complete.stderr'), terminal.stderr ?? '');
    assert.equal(terminal.status, 0, terminal.stderr);
    assert.ok(!terminal.stdout.includes('Action refused:'), 'terminal completion must be accepted');
    const ownerAfter = face();
    assert.equal(ownerAfter.presentation.basis.body_id, bodyId);
    assert.ok(ownerAfter.presentation.properties.some(property => property.subject === item.identity
      && property.name === 'complete' && property.value.Flag === true), 'terminal must commit the item completion');
    const writeReceipt = JSON.parse(await readFile(path.join(state, 'body/owner-execution.json'))).last_execution.write;
    const initiatingEvidence = terminalEvidence(terminal.stdout).find(evidence => evidence.show_id === writeReceipt.initiating_action.show_id);
    assert.ok(initiatingEvidence, 'terminal must retain its exact initiating Show evidence');
    assert.equal(initiatingEvidence.owner_selected_show_current, true);
    const completion = await observedTodoMutation(state, output, 'terminal-complete',
      bodyId, initiatingEvidence, ownerAfter, action.identity);
    assert.ok(terminal.stdout.includes(completion.initiating_show_id),
      'checkpoint completion must name a Show actually acknowledged by this terminal');
    const staleView = after;
    const staleButton = page.getByRole('button', { name: `complete ${itemText}`, exact: true });
    assert.ok(await staleButton.isEnabled(), 'the previously displayed action must still be offered for this stale-Show test');
    await staleButton.click();
    await page.waitForFunction(() => document.querySelector('[data-owner-action-result]')?.dataset.refused === 'true',
      null, { timeout: 12_000 });
    const refusal = await page.locator('[data-owner-action-result]').textContent();
    const ownerAfterStale = face();
    const todoProperties = value => value.presentation.properties.filter(property => property.subject.startsWith('todo/'));
    assert.deepEqual(todoProperties(ownerAfterStale), todoProperties(ownerAfter), 'stale action must not mutate Todo');
    await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-stale.png') });
    const stale = { initiating_face_id: staleView.face_id, initiating_face_revision: staleView.face_revision,
      initiating_show_id: staleView.show_id, action_id: action.identity, refusal, owner_after: ownerAfterStale };
    await page.getByRole('button', { name: 'Inspect current wardrobe' }).click();
    await page.waitForFunction(prior => {
      try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent)
        .wardrobe_revision_decimal !== prior; } catch { return false; }
    }, currentWardrobe.wardrobe_revision_decimal, { timeout: 12_000 });
    await wardrobeIdle();
    currentWardrobe = await wardrobe();
    if (await page.getByRole('button', { name: `Wear ${description.mask_name}`, exact: true }).isEnabled()) {
      await changeWardrobe('Wear');
    }
    if (await page.getByRole('button', { name: `Prefer only ${description.mask_name}`, exact: true }).isEnabled()) {
      await changeWardrobe('Prefer only');
    }
    await page.getByRole('button', { name: 'Refresh this Face' }).click();
    await page.waitForFunction(target => {
      const view = globalThis.__conduitOwnerParticipation.face();
      return view?.show_state === 'available' && view.interactions_admitted
        && view.actions.some(action => action.intent === 'todo/reopen@1' && action.target === target);
    }, item.identity, { timeout: 12_000 });
    after = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
    assert.equal(after.body_id, bodyId);
    assert.equal(await page.locator('.owner-face-collection-count').textContent(), '2 things left · 1 completed');
    assert.equal(await page.locator('.owner-face-completed').getAttribute('open'), null,
      'completed items must remain subordinate in the checklist');
    const completedEvidence = terminalEvidence(terminal.stdout).at(-1);
    assert.equal(completedEvidence.body_id, bodyId);
    assert.equal(completedEvidence.owner_selected_show_current, true);
    crossMask = { joined, completion, completed_evidence: completedEvidence,
      inspected_at_unix_ms: Date.now(), stale, action_id: action.identity, target: item.identity, owner_before: ownerBefore,
      owner_after: ownerAfter, browser_after: after, terminal_input: 'terminal-complete.input',
      terminal_stdout: 'terminal-complete.stdout', terminal_stderr: 'terminal-complete.stderr' };
  }
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-after.png') });
  const afterOwner = face();
  assert.equal(afterOwner.presentation.identity, after.face_id);
  const inspectCapture = { face_id: after.face_id,
    face_revision: afterOwner.presentation_revision_decimal, show_id: after.show_id,
    observed_at_unix_ms: Date.now() };
  assert.equal(afterOwner.presentation.basis.body_id, bodyId);
  assert.ok(afterOwner.presentation.subjects.some(subject => subject.name === itemText));
  assert.deepEqual(errors, []);
  const directSpeech = scenario === 'cross-mask-speech'
    ? await captureDirectTodoSpeech({ owner, state, output: path.join(output, 'spoken'), bodyId })
    : null;
  await writeFile(path.join(output, 'receipt.json'), `${JSON.stringify({
    schema: 'conduit.proof/todo-owner-browser@1',
    ...sourceRecord,
    body_id: bodyId, browser_host_id: identity.hostId, browser_boot_id: identity.bootId,
    item_text: itemText, adds, cross_mask: crossMask, inspect_capture: inspectCapture,
    speech_before_completion: speechBeforeCompletion,
    direct_speech: directSpeech, before: { face_id: before.face_id, face_revision: before.face_revision,
      show_id: before.show_id }, after: { face_id: after.face_id,
      face_revision: after.face_revision, show_id: after.show_id },
    screenshots: scenario.startsWith('cross-mask') ? ['browser-before.png', 'browser-stale.png', 'browser-after.png']
      : ['browser-before.png', 'browser-after.png'],
  }, null, 2)}\n`);
} catch (error) {
  // Preserve the actual failed UI and owner state without retrying its action.
  const diagnostics = [writeFile(path.join(output, 'failure.json'), `${JSON.stringify({
    error: String(error), page_errors: errors, ...sourceRecord,
  }, null, 2)}\n`)];
  if (page) {
    diagnostics.push(page.screenshot({ path: path.join(output, 'browser-failed.png') }));
    diagnostics.push(page.content().then(html => writeFile(path.join(output, 'browser-failed.html'), html)));
    diagnostics.push(page.evaluate(() => globalThis.__conduitOwnerParticipation?.face())
      .then(view => writeFile(path.join(output, 'browser-failed-face.json'), `${JSON.stringify(view, null, 2)}\n`)));
  }
  await Promise.allSettled(diagnostics);
  throw error;
} finally {
  await browser?.close();
  server?.child.kill();
}
