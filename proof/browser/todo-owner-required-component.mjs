// Installed-owner observations only. The caller supplies its existing owner
// reader after its first actual Add; this helper creates no Body or Face state.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { observedTodoMutation } from './todo-owner-events.mjs';
import { parseCaptureJSON, captureDecimal } from './exact-capture-json.mjs';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const itemId = 'todo/item/task-1';
const surfaceSelector = '[data-owner-face-surface]';
const exactFace = owner => ({ ...owner.face, revision: owner.face_revision_decimal });
const completed = owner => {
  const values = owner.face.properties.filter(property => property.subject === itemId && property.name === 'complete');
  assert.equal(values.length, 1);
  assert.equal(typeof values[0].value.Flag, 'boolean');
  return values[0].value.Flag;
};

export async function captureRequiredTodoComponent({ page, face, output, state }) {
  const directory = path.join(output, 'required-component');
  await mkdir(directory); // Refuse to overwrite evidence from another run.
  const observations = [], actions = [];
  const source = await readFile(new URL('../../semantics/face/components/todo-required-item.conduit', import.meta.url));
  const evidence = { schema: 'conduit.proof/todo-required-component@1', evidence_directory: 'required-component',
    scope: 'actual installed-owner Source, acknowledged browser Shows and verified write/read Plan/Play terminal receipts',
    binding_proof: 'source-pinned canonical recipe/profile, checked subject_regions and SlotAction fallback; no JavaScript recipe decoder',
    required_recipe_source_sha256: hash(source), observations, actions };
  const retain = () => writeFile(path.join(directory, 'observations.json'), `${JSON.stringify(evidence, null, 2)}\n`);
  const ready = prior => page.waitForFunction(prior => {
    const view = globalThis.__conduitOwnerParticipation.face();
    return view?.show_state === 'available' && view.interactions_admitted === true
      && (!prior || view.show_id !== prior)
      && document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === view.show_id;
  }, prior ?? null, { timeout: 12_000 });
  const observe = async label => {
    await ready();
    const browser = await page.evaluate(() => {
      const api = globalThis.__conduitOwnerParticipation;
      const element = document.querySelector('[data-owner-face-surface]');
      return { view: api.face(), canonical_json: api.canonicalFaceJSON(), back: api.componentBack(),
        acknowledged_show: document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged,
        dom: { tag: element?.localName, dataset: { ...element?.dataset },
          reading: element?.shadowRoot?.textContent } };
    });
    const owner = face();
    const occurrence = { label, owner, ...browser, screenshot: `${label}.png` };
    observations.push(occurrence); await retain();
    const canonical = parseCaptureJSON(browser.canonical_json);
    occurrence.canonical_face_json = JSON.stringify(canonical.face);
    assert.equal(captureDecimal(canonical.face.revision), owner.face_revision_decimal);
    assert.deepEqual({ ...canonical.face, revision: owner.face_revision_decimal }, exactFace(owner),
      'canonical browser Face must equal the independently observed current owner Face');
    assert.equal(browser.view.face_id, owner.face.identity);
    assert.equal(browser.view.face_revision, owner.face_revision_decimal);
    assert.equal(browser.view.body_id, owner.face.basis.body_id);
    assert.equal(browser.acknowledged_show, browser.view.show_id);
    await page.locator('[data-owner-face]').screenshot({ path: path.join(directory, occurrence.screenshot) });
    return occurrence;
  };
  const assertSelected = occurrence => {
    assert.equal(occurrence.back.selected, true);
    assert.ok(occurrence.back.bundle_sha256 && occurrence.back.artifact_id);
    assert.equal(occurrence.dom.tag, 'conduit-owner-face');
    assert.equal(occurrence.view.mask_renderer_artifact_id, occurrence.back.artifact_id);
    assert.equal(occurrence.view.mask_renderer_artifact_id,
      `artifact/browser-owner-face-component@1/sha256/${occurrence.back.bundle_sha256}`);
    const configuration = occurrence.view.component?.configuration;
    assert.ok(configuration?.component.source_document_id && configuration.component.checked_plot_id && configuration.component.fore_id);
    assert.ok(configuration.placement_id && configuration.implementation_id && configuration.host_id && configuration.boot_id);
    assert.ok(configuration.recipe.profile && Array.isArray(configuration.recipe.canonical_value));
    assert.ok(configuration.recipe.canonical_value.length > 0 && configuration.recipe.canonical_value.length <= 65536);
    return configuration;
  };
  const assertBound = occurrence => {
    const configuration = assertSelected(occurrence), component = occurrence.view.component;
    assert.equal(component.fallback, null);
    assert.equal(occurrence.dom.dataset.componentFallback, '');
    const index = occurrence.view.subjects.findIndex(subject => subject.identity === itemId);
    assert.ok(index >= 0);
    assert.equal(component.subject_regions[index], 0, 'checked required item must occupy the admitted region');
    assert.equal(component.groups[0], 0);
    assert.equal(completed(occurrence.owner), false);
    const action = occurrence.view.actions.find(action => action.identity === 'todo.complete.task-1');
    assert.equal(action?.target, itemId); assert.equal(action?.intent, 'todo/complete@1');
    assert.equal(action?.availability, 'available'); assert.deepEqual(action.arguments, []);
    return configuration;
  };
  const assertFallback = occurrence => {
    assertSelected(occurrence);
    assert.equal(occurrence.view.component.fallback, 'SlotAction');
    assert.equal(occurrence.dom.dataset.componentFallback, 'SlotAction');
    assert.ok(occurrence.view.component.subject_regions.every(region => region === null));
    assert.equal(completed(occurrence.owner), true);
    assert.ok(!occurrence.view.actions.some(action => action.identity === 'todo.complete.task-1' && action.availability === 'available'));
    // Recipe refusal preserves the remaining source-backed generic reading/actions.
    const owner = page.locator(surfaceSelector);
    assert.ok(occurrence.dom.reading.includes(occurrence.owner.face.subjects.find(subject => subject.identity === itemId).name));
    for (const text of occurrence.owner.face.text.filter(text => text.subject === 'todo/status')) {
      assert.ok(occurrence.dom.reading.includes(text.text), 'fallback preserves actual source status wording');
    }
    const add = owner.locator('[data-owner-action="todo.add"]').getByRole('button');
    return add.isEnabled().then(enabled => assert.equal(enabled, true));
  };
  const act = async (before, identity, method, label) => {
    const action = before.view.actions.find(action => action.identity === identity && action.target === itemId);
    assert.equal(action?.availability, 'available'); assert.deepEqual(action.arguments, []);
    const form = page.locator(surfaceSelector).locator(`[data-owner-action="${identity}"]`);
    assert.equal(await form.count(), 1);
    // Completed items may be in real, source-backed local disclosure.
    const ancestors = await form.locator('xpath=ancestor::details').all();
    for (const details of ancestors.reverse()) {
      if (!await details.evaluate(node => node.open)) await details.locator(':scope > summary').click();
    }
    await form.evaluate(element => {
      globalThis.__todoRequiredActivationEvents = [];
      for (const type of ['click', 'submit']) element.addEventListener(type, event => {
        globalThis.__todoRequiredActivationEvents.push({ type, trusted: event.isTrusted,
          action_id: element.dataset.ownerAction });
      }, { capture: true });
    });
    const button = form.getByRole('button');
    assert.equal(await button.count(), 1); assert.equal(await button.isEnabled(), true);
    if (method === 'keyboard') { await button.focus(); await page.keyboard.press('Enter'); }
    else await button.click();
    await ready(before.view.show_id);
    const activationEvents = await page.evaluate(() => globalThis.__todoRequiredActivationEvents);
    assert.equal(activationEvents.filter(event => event.type === 'submit').length, 1,
      'one trusted activation produces one native form submission');
    assert.ok(activationEvents.every(event => event.trusted && event.action_id === identity));
    const after = await observe(label);
    assert.equal(after.owner.face.basis.body_id, before.owner.face.basis.body_id);
    assert.notEqual(after.owner.face.identity, before.owner.face.identity);
    assert.notEqual(after.owner.face_revision_decimal, before.owner.face_revision_decimal);
    assert.notEqual(after.view.show_id, before.view.show_id);
    assert.equal(completed(after.owner), action.intent === 'todo/complete@1');
    const identities = owner => owner.face.subjects.filter(subject => subject.role === 'Item')
      .map(({ identity, name, role }) => ({ identity, name, role }));
    assert.deepEqual(identities(after.owner), identities(before.owner), 'actions retain source item identity/name');
    const mutation = await observedTodoMutation(state, directory, label, after.owner.face.basis.body_id,
      { ...before.view, face_revision_decimal: before.owner.face_revision_decimal }, after.owner, identity);
    actions.push({ method, activation_events: activationEvents, action, mutation, initiating_show: before.view.show_id,
      initiating_face: before.owner.face.identity, resulting_show: after.view.show_id,
      resulting_face: after.owner.face.identity, label });
    await retain(); return after;
  };
  try {
    const generic = await observe('ordinary-generic');
    assert.equal(generic.dom.tag, 'div');
    assert.equal(generic.view.mask_renderer_artifact_id, 'artifact/browser-runtime');
    assert.equal(generic.view.component ?? null, null);
    const items = generic.owner.face.subjects.filter(subject => subject.role === 'Item');
    assert.equal(items.length, 1, 'required specimen starts immediately after the sole first Add');
    assert.equal(items[0].identity, itemId); assert.equal(completed(generic.owner), false);
    const selected = [];
    for (const look of ['compact', 'spacious']) {
      const before = observations.at(-1);
      await page.evaluate(look => globalThis.__conduitOwnerParticipation.selectFace(look, 'required-item'), look);
      await ready(before.view.show_id);
      const occurrence = await observe(`required-${look}`);
      const configuration = assertBound(occurrence);
      assert.equal(configuration.invocation_path[0], `todo-mask-required-item-${look}`);
      assert.equal(occurrence.view.component.expression.expression.density, look);
      assert.equal(occurrence.dom.dataset.componentDensity, look);
      assert.deepEqual(exactFace(occurrence.owner), exactFace(generic.owner), 'Look/recipe selection must preserve the entire canonical Source Face');
      assert.equal(occurrence.canonical_face_json, generic.canonical_face_json, 'selected Back must retain byte-identical canonical Face JSON');
      occurrence.checked_binding = { subject: itemId, region: 0, action: 'todo.complete.task-1',
        recipe_profile: configuration.recipe.profile,
        canonical_recipe_sha256: hash(Uint8Array.from(configuration.recipe.canonical_value)) };
      selected.push(occurrence);
    }
    assert.deepEqual(selected[0].view.component.configuration.recipe, selected[1].view.component.configuration.recipe);
    assert.deepEqual(selected[0].view.component.configuration.component, selected[1].view.component.configuration.component);
    assert.notEqual(selected[0].view.mask_plan_id, selected[1].view.mask_plan_id);
    const first = await act(selected[1], 'todo.complete.task-1', 'keyboard', 'keyboard-complete-slot-fallback');
    await assertFallback(first);
    const reopened = await act(first, 'todo.reopen.task-1', 'pointer', 'pointer-reopen-slot-bound');
    assertBound(reopened);
    const last = await act(reopened, 'todo.complete.task-1', 'pointer', 'pointer-complete-slot-fallback');
    await assertFallback(last);
    evidence.result = 'passed'; await retain(); return evidence;
  } catch (error) {
    evidence.result = 'failed'; evidence.error = error.stack ?? String(error); await retain();
    await page.locator('[data-owner-face]').screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
    throw error;
  }
}
