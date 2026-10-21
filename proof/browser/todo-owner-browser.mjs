// Live browser Todo action against one already running installed Body owner.
// The owner and Handbook package are supplied by the caller; this driver never
// creates a second Body or constructs Face state for the browser.
import assert from 'node:assert/strict';
import { captureTodoAccessibility, withTodoStylesDisabled } from './todo-owner-accessibility.mjs';
import { captureArbitraryTodoRefusals } from './todo-owner-arbitrary-action.mjs';
import { inspectTodoRealization, activateTodoAdd } from './todo-owner-realization.mjs';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { startStaticProduct } from './static-product-server.mjs';
import { observedTodoMutation } from './todo-owner-events.mjs';
import { captureDirectTodoSpeech } from './todo-owner-spoken.mjs';
import { retainedTodoDomCompletion, compareRetainedTodoLooks } from './todo-owner-retained-dom.mjs';
import { assertLookReplacement } from './look-replacement-evidence.mjs';

const [ownerCwdArg, binaryArg, stateArg, handbookArg, outputArg, playwrightArg,
  itemText = 'Pick up prescription', scenario = 'first-add'] = process.argv.slice(2);
assert.ok(['first-add', 'count-matrix', 'cross-mask', 'cross-mask-speech', 'cross-mask-speech-resume', 'look-replacement', 'retained-dom-completion'].includes(scenario), 'unknown Todo capture scenario');
const retainedDom = scenario === 'retained-dom-completion';
const resumeExisting = scenario === 'cross-mask-speech-resume' || retainedDom;
const captureSpeech = scenario.startsWith('cross-mask-speech');
if (!ownerCwdArg || !binaryArg || !stateArg || !handbookArg || !outputArg || !playwrightArg) {
  throw new Error('usage: todo-owner-browser.mjs OWNER-CWD INSTALLED-OWNER OWNER-STATE HANDBOOK-PACKAGE NEW-EVIDENCE-DIR PINNED-PLAYWRIGHT [NEW-ITEM-TEXT] [first-add|cross-mask|cross-mask-speech|cross-mask-speech-resume|look-replacement|retained-dom-completion]');
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
if (captureSpeech) assert.equal(installed.selected_speech?.artifact_only, true,
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
if (retainedDom) assert.equal(sourceRecord.source_relation, 'exact-source',
  'retained DOM mutation requires identical clean owner/runtime/UI source');
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
  return { schema: value.schema, face: value.face,
    face_revision_decimal: value.face_revision_decimal,
    advertisement: { host_id: value.advertisement.host_id, boot_id: value.advertisement.boot_id } };
};
const beforeOwner = face();
const bodyId = beforeOwner.face.basis.body_id;
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
  // An explicit preference change retires the prior Show. Request this Mask's
  // new Show before using it as the startup observation basis.
  await page.getByRole('button', { name: 'Refresh this Face' }).click();
  await page.waitForFunction(() => {
    const current = globalThis.__conduitOwnerParticipation.face();
    return current?.show_state === 'available'
      && document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === current.show_id;
  }, null, { timeout: 12_000 });
  const beforeStart = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  if (!resumeExisting) {
    owner(['body', 'start', '--state-dir', state, '--maximum-millis', '60000',
      '--todo-new-list', 'Groceries']);
  }
  let startedFace;
  for (let attempt = 0; attempt < 100; attempt++) {
    startedFace = face();
    if (startedFace.face.actions.some(action => action.identity === 'todo.add'
      && action.availability === 'Available')) break;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  assert.ok(startedFace.face.actions.some(action => action.identity === 'todo.add'
    && action.availability === 'Available'), 'Todo Play did not admit the add action');
  if (!resumeExisting) assert.ok(!startedFace.face.subjects.some(subject => subject.name === itemText),
    'new Todo item must be absent before this browser action');
  await page.getByRole('button', { name: 'Refresh this Face' }).click();
  await page.waitForFunction(({ prior, resumed }) => {
    const current = globalThis.__conduitOwnerParticipation.face();
    return current?.show_state === 'available' && (resumed || current.face_revision !== prior)
      && current?.actions.some(action => action.identity === 'todo.add')
      && document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === current.show_id;
  }, { prior: beforeStart.face_revision, resumed: resumeExisting }, { timeout: 12_000 });
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
  let retainedLookSelection = null;
  if (retainedDom) {
    retainedLookSelection = await compareRetainedTodoLooks({ page, face, bodyId,
      hostId: identity.hostId, bootId: identity.bootId, output });
  }
  const before = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(before.body_id, bodyId);
  assert.equal(before.show_state, 'available');
  const realization = await inspectTodoRealization({page,view:before,face,state});
  const accessibleBefore = scenario === 'first-add'
    ? await captureTodoAccessibility({ page, face, state, output, sourceRecord, stage: 'before-add' }) : null;
  if (scenario === 'look-replacement') {
    const observations = [];
    for (const [look, recipe] of [['compact', 'flat'], ['spacious', 'flat'], ['spacious', 'grouped']]) {
      await page.evaluate(({ look, recipe }) =>
        globalThis.__conduitOwnerParticipation.selectFace(look, recipe), { look, recipe });
      await page.waitForFunction(() => {
        const element = document.querySelector('conduit-owner-face');
        const view = globalThis.__conduitOwnerParticipation.face();
        return view?.component && element?.dataset.componentColumns ===
          String(view.component.maximum_columns);
      }, null, { timeout: 12_000 });
      const occurrence = await page.evaluate(() => ({
        view: globalThis.__conduitOwnerParticipation.face(),
        canonical_json: globalThis.__conduitOwnerParticipation.canonicalFaceJSON(),
        geometry: (() => {
          const element = document.querySelector('conduit-owner-face');
          const grid = element.shadowRoot.querySelector('.owner-face-document');
          return { width: element.getBoundingClientRect().width,
            admitted_columns: Number(element.dataset.componentColumns),
            computed_columns: getComputedStyle(grid).gridTemplateColumns.split(/\s+/).filter(Boolean),
            grid_width: grid.getBoundingClientRect().width,
            button_font_px: Number.parseFloat(getComputedStyle(grid.querySelector('button')).fontSize) };
        })(),
        acknowledged_show_id: document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged,
      }));
      assert.ok(occurrence.view?.component, 'production renderer must supply checked component evidence');
      assert.ok(occurrence.geometry.width - 2 * occurrence.view.component.gutter >=
        occurrence.view.component.reflow_width, 'wide recipe proof must meet actual content reflow width');
      assert.equal(occurrence.geometry.admitted_columns, recipe === 'grouped' ? 2 : 1);
      assert.equal(occurrence.geometry.computed_columns.length, occurrence.geometry.admitted_columns,
        'checked recipe must change actual computed grid columns');
      assert.equal(occurrence.acknowledged_show_id, occurrence.view.show_id);
      const currentOwner = face();
      assert.deepEqual(currentOwner.face, startedFace.face,
        'face selection must preserve the original whole Todo Face');
      assert.equal(currentOwner.face_revision_decimal, startedFace.face_revision_decimal);
      assert.deepEqual(currentOwner.advertisement, startedFace.advertisement);
      const screenshot = `look-${recipe}-${look}.png`;
      await page.locator('[data-owner-face]').screenshot({ path: path.join(output, screenshot) });
      let retirement = null;
      if (observations.length > 0) {
        const retired = observations.at(-1).view;
        const action = retired.actions.find(action => action.identity === 'todo.add');
        assert.ok(action, 'retired Show must retain its actual typed add action');
        assert.equal(action.arguments.length, 1, 'Todo add must expose its one declared text argument');
        const refusal = await page.evaluate(async ({ view, action }) => {
          try {
            await globalThis.__conduitOwnerParticipation.submitFaceInteraction({
              view, actionId: action.identity, target: action.target,
              arguments: [{ name: action.arguments[0].name, value: 'Retired Look must not add this item' }],
            });
            return { refused: false };
          } catch (error) {
            return { refused: true, message: error.message, code: error.code ?? null };
          }
        }, { view: retired, action });
        assert.equal(refusal.refused, true, 'old Show interaction must be refused');
        assert.equal(refusal.code, 'StaleShow',
          'the production SDK must reject the retired acknowledged Show before WASM dispatch');
        const ownerAfterRefusal = face();
        assert.deepEqual(ownerAfterRefusal.face, currentOwner.face,
          'retired Show refusal must preserve the entire canonical owner Face');
        assert.equal(await page.evaluate(() => globalThis.__conduitOwnerParticipation.face().show_id),
          occurrence.view.show_id, 'refusal must preserve the replacement Show');
        retirement = { retired_show_id: retired.show_id,
          current_show_id: occurrence.view.show_id, refusal, refusalLayer: 'sdk-before-wasm',
          owner_before: currentOwner, owner_after: ownerAfterRefusal };
      }
      observations.push({ look, recipe, ...occurrence, owner: currentOwner, screenshot, retirement });
      await writeFile(path.join(output, 'look-observations.json'), `${JSON.stringify({
        scope: 'actual acknowledged browser occurrences and old-Show refusal; execution required',
        observations,
      }, null, 2)}\n`);
    }
    assert.notEqual(observations[0].view.show_id, observations[1].view.show_id);
    assert.notDeepEqual(observations[0].view.component.expression, observations[1].view.component.expression);
    assert.deepEqual(observations[0].view.component.configuration.component,
      observations[1].view.component.configuration.component);
    assert.deepEqual(observations[0].view.component.configuration.recipe,
      observations[1].view.component.configuration.recipe);
    assert.notDeepEqual(observations[1].view.component.configuration.recipe,
      observations[2].view.component.configuration.recipe);
    const ownerEvidence = owner => ({
      sourceDocumentId: owner.face.basis.source_document_id,
      bodyId: owner.face.basis.body_id,
      hostId: owner.advertisement.host_id, bootId: owner.advertisement.boot_id,
      planId: owner.face.basis.plan_id, playId: owner.face.basis.active_play_id,
    });
    const exactFace = owner => ({ ...owner.face, revision: owner.face_revision_decimal });
    const occurrenceEvidence = occurrence => {
      const canonical = JSON.parse(occurrence.canonical_json);
      assert.equal(canonical.face_revision_decimal, occurrence.owner.face_revision_decimal);
      const canonicalFace = { ...canonical.face, revision: canonical.face_revision_decimal };
      assert.deepEqual(canonicalFace, exactFace(occurrence.owner),
        'renderer retained canonical Face must match the independently observed owner');
      const configuration = occurrence.view.component.configuration;
      return { canonicalFace, owner: ownerEvidence(occurrence.owner),
        component: { sourceDocumentId: configuration.component.source_document_id,
          checkedPlotId: configuration.component.checked_plot_id, foreId: configuration.component.fore_id,
          recipe: configuration.recipe },
        realization: { expandedPlotId: occurrence.view.mask_expanded_plot_id,
          planId: occurrence.view.mask_plan_id, playId: occurrence.view.mask_play_id,
          showId: occurrence.view.show_id, acknowledgedShowId: occurrence.acknowledged_show_id,
          expression: occurrence.view.component.expression } };
    };
    const retired = observations[1].retirement;
    const replacementEvidence = { before: occurrenceEvidence(observations[0]),
      after: occurrenceEvidence(observations[1]), retirement: {
        attemptedShowId: retired.retired_show_id, currentShowId: retired.current_show_id,
        refusedBeforeSourceAdmission: retired.refusal.refused && retired.refusal.code === 'StaleShow',
        refusalCode: retired.refusal.code, refusalLayer: retired.refusalLayer,
        ownerBefore: ownerEvidence(retired.owner_before), ownerAfter: ownerEvidence(retired.owner_after),
        faceBefore: exactFace(retired.owner_before), faceAfter: exactFace(retired.owner_after),
      } };
    assertLookReplacement(replacementEvidence);
    await writeFile(path.join(output, 'look-replacement-evidence.json'),
      `${JSON.stringify(replacementEvidence, null, 2)}\n`);
    await page.getByLabel(/^Look\b/).selectOption('spacious');
    await page.getByLabel(/^Layout\b/).selectOption('grouped');
    await page.getByLabel(/^Minimum text size\b/).selectOption('200');
    await page.getByRole('button', { name: 'Apply face', exact: true }).click();
    await page.waitForFunction(() => {
      const view = globalThis.__conduitOwnerParticipation.face();
      return view?.component?.expression?.expression?.text_scale_percent === 200 &&
        document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === view.show_id;
    }, null, { timeout: 12_000 });
    const largeText = await page.evaluate(() => {
      const element = document.querySelector('conduit-owner-face');
      const button = element.shadowRoot.querySelector('.owner-face-document button');
      return { view: globalThis.__conduitOwnerParticipation.face(),
        canonical_json: globalThis.__conduitOwnerParticipation.canonicalFaceJSON(),
        button_font_px: Number.parseFloat(getComputedStyle(button).fontSize) };
    });
    assert.deepEqual(face().face, startedFace.face,
      'user text-size override must preserve the whole canonical Face');
    const priorScale = observations[2].view.component.expression.expression.text_scale_percent;
    const expectedFont = observations[2].geometry.button_font_px * 200 / priorScale;
    assert.ok(Math.abs(largeText.button_font_px - expectedFont) < 0.1,
      'user minimum must change actual rendered control text proportionally');
    await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'look-large-text.png') });
    await writeFile(path.join(output, 'look-observations.json'), `${JSON.stringify({
      scope: 'actual acknowledged browser occurrences, computed geometry, old-Show refusal and user large-text override',
      observations, large_text: { ...largeText, screenshot: 'look-large-text.png' },
    }, null, 2)}\n`);
  }
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-before.png') });
  const specimenNames = [itemText, 'Prepare lunch', 'Water plants'];
  let resumedItems = null;
  const domCompletion = retainedDom
    ? await retainedTodoDomCompletion({ page, face, state, output, bodyId }) : null;
  if (resumeExisting && !retainedDom) {
    assert.equal(new Set(specimenNames).size, specimenNames.length, 'resume specimen names must be distinct');
    const retained = face();
    assert.deepEqual(retained.face.subjects.filter(subject => subject.role === 'Item').map(subject => subject.name), specimenNames,
      'resume requires the exact previously added three-item specimen');
    assert(retained.face.text.some(text => text.subject === 'todo/status' && text.text === '3 things left · 0 completed'),
      'resume refuses an already completed or differently mutated specimen');
    for (const item of retained.face.subjects.filter(subject => subject.role === 'Item')) {
      assert(retained.face.properties.some(property => property.subject === item.identity
        && property.name === 'complete' && property.value.Flag === false), 'resume requires every retained item to be pending');
    }
    resumedItems = retained;
    await writeFile(path.join(output, 'resumed-owner-face.json'), `${JSON.stringify(retained, null, 2)}\n`);
  }
  const matrix = [];
  const recordMatrix = async (ownerFace, expected) => {
    const properties = ownerFace.face.properties.filter(property => property.subject === 'todo/list');
    const count = name => properties.find(property => property.name === name)?.value?.Count;
    assert.equal(count('remaining-count'), expected);
    assert.equal(count('completed-count'), 0);
    assert.equal(ownerFace.face.subjects.filter(subject => subject.role === 'Item').length, expected);
    const filename = `owner-count-${expected}.json`;
    const raw = spawnSync(binary, ['body', 'face', '--state-dir', state, '--json'],
      { cwd: ownerCwd, encoding: 'utf8', timeout: 10_000 });
    assert.equal(raw.status, 0, raw.stderr);
    const exact = JSON.parse(raw.stdout);
    assert.equal(exact.face.identity, ownerFace.face.identity);
    assert.equal(exact.face_revision_decimal, ownerFace.face_revision_decimal);
    // Preserve native canonical integer bytes; JSON.stringify rounds u64 revisions.
    await writeFile(path.join(output, filename), raw.stdout);
    matrix.push({ remaining: expected, completed: 0, face_id: ownerFace.face.identity,
      revision: ownerFace.face_revision_decimal, file: filename });
    await writeFile(path.join(output, 'count-matrix.json'), `${JSON.stringify({
      ...sourceRecord, body_id: bodyId, proof_class: 'live native owner with actual browser Adds', observations: matrix,
    }, null, 2)}\n`);
  };
  if (scenario === 'count-matrix') await recordMatrix(startedFace, 0);
  const texts = scenario === 'count-matrix' ? [itemText, ...Array.from({ length: 19 }, (_, i) => `Task ${i + 2}`)]
    : resumeExisting ? [] : scenario.startsWith('cross-mask') ? specimenNames : [itemText];
  assert.equal(new Set(texts).size, texts.length, 'scenario item names must be distinct');
  const adds = [];
  let after = retainedDom
    ? await page.evaluate(() => globalThis.__conduitOwnerParticipation.face()) : before;
  const performAdd = async (text, unstyled = false) => {
    const prior = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
    const priorOwner = face();
    assert.equal(priorOwner.face.identity, prior.face_id);
    const exactPrior = { ...prior, face_revision_decimal: priorOwner.face_revision_decimal };
    const add = page.locator('[data-owner-action="todo.add"]');
    const activate = () => activateTodoAdd({page,form:add,text,realization,scenario});
    const activation = unstyled ? await withTodoStylesDisabled(page, activate) : await activate();
    await page.waitForFunction(({ revision, name }) => {
      const current = globalThis.__conduitOwnerParticipation.face();
      return current?.face_revision !== revision && current?.subjects.some(subject => subject.name === name)
        && document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === current.show_id;
    }, { revision: prior.face_revision, name: text }, { timeout: 12_000 });
    after = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
    assert.equal(after.body_id, bodyId);
    assert.notEqual(after.show_id, prior.show_id);
    const ownerAfterAdd = face();
    if (scenario === 'count-matrix' && [1, 3, 20].includes(adds.length + 1)) await recordMatrix(ownerAfterAdd, adds.length + 1);
    assert.equal(ownerAfterAdd.face.identity, after.face_id);
    const mutation = await observedTodoMutation(state, output, `add-${adds.length + 1}`,
      bodyId, exactPrior, ownerAfterAdd, 'todo.add');
    const screenshot = `browser-add-${adds.length + 1}.png`;
    await page.locator('[data-owner-face]').screenshot({ path: path.join(output, screenshot) });
    adds.push({ action_id: 'todo.add', text, activation, stylesheet_disabled_during_activation: unstyled, before: exactPrior, after, owner: ownerAfterAdd,
      mutation, screenshot });
    await writeFile(path.join(output, 'adds.json'), `${JSON.stringify(adds, null, 2)}\n`);
    return adds.at(-1);
  };
  for (const text of texts) await performAdd(text, Boolean(accessibleBefore));
  if (accessibleBefore) {
    const accessibleAfter = await captureTodoAccessibility({ page, face, state, output, sourceRecord, stage: 'after-add' });
    assert.equal(accessibleAfter.body_id, accessibleBefore.body_id);
    assert.notEqual(accessibleAfter.face_revision, accessibleBefore.face_revision);
    assert.notEqual(accessibleAfter.show_id, accessibleBefore.show_id);
  }
  let crossMask = null;
  const speechBeforeCompletion = captureSpeech
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
    const canonicalTodo = value => value.face.properties.filter(property => property.subject.startsWith('todo/'));
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
      observed_owner_face_id: ownerBefore.face.identity,
      observed_owner_face_revision: ownerBefore.face_revision_decimal,
      acknowledged_show_id: [...join.stdout.matchAll(/Owner terminal Show (show\/[a-f0-9]+)/g)].at(-1)[1],
      transcript: 'terminal-join.stdout', todo_mutation: false };

    assert.equal(ownerBefore.face.subjects.filter(subject => subject.role === 'Item').length, 3);
    const item = ownerBefore.face.subjects.find(subject => subject.role === 'Item' && subject.name === itemText);
    assert.ok(item, 'browser-added item must exist in the canonical Face');
    const action = ownerBefore.face.actions.find(action => action.intent === 'todo/complete@1'
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
    assert.equal(ownerAfter.face.basis.body_id, bodyId);
    assert.ok(ownerAfter.face.properties.some(property => property.subject === item.identity
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
    const todoProperties = value => value.face.properties.filter(property => property.subject.startsWith('todo/'));
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
  if (scenario === 'first-add') await captureArbitraryTodoRefusals({
    page, face, output, sourceRecord, bodyId, identity, retiredView: before, state,
    acceptCurrent: () => performAdd('Verify recovered current Show'),
  });
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-after.png') });
  const afterOwner = face();
  assert.equal(afterOwner.face.identity, after.face_id);
  const inspectCapture = { face_id: after.face_id,
    face_revision: afterOwner.face_revision_decimal, show_id: after.show_id,
    observed_at_unix_ms: Date.now() };
  assert.equal(afterOwner.face.basis.body_id, bodyId);
  if (!retainedDom) assert.ok(afterOwner.face.subjects.some(subject => subject.name === itemText));
  assert.deepEqual(errors, []);
  const directSpeech = captureSpeech
    ? await captureDirectTodoSpeech({ owner, state, output: path.join(output, 'spoken'), bodyId })
    : null;
  await writeFile(path.join(output, 'receipt.json'), `${JSON.stringify({
    schema: 'conduit.proof/todo-owner-browser@1',
    ...sourceRecord,
    body_id: bodyId, browser_host_id: identity.hostId, browser_boot_id: identity.bootId,
    item_text: itemText, realization, adds, resumed_items: resumedItems, cross_mask: crossMask, inspect_capture: inspectCapture,
    speech_before_completion: speechBeforeCompletion,
    direct_speech: directSpeech, retained_look_selection: retainedLookSelection, retained_dom_completion: domCompletion, before: { face_id: before.face_id, face_revision: before.face_revision,
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
