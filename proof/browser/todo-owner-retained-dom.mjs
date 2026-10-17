// Real DOM actions against the caller's sole retained owner; never creates a list.
import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { observedTodoMutation } from './todo-owner-events.mjs';

export async function retainedTodoDomCompletion({ page, face, state, output, bodyId }) {
  const current = () => page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  const ready = () => page.waitForFunction(() => {
    const view = globalThis.__conduitOwnerParticipation.face();
    return view?.show_state === 'available' && view.interactions_admitted
      && document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === view.show_id;
  }, null, { timeout: 12_000 });
  const initial = face();
  const items = initial.face.subjects.filter(subject => subject.role === 'Item');
  assert.equal(items.length, 3, 'retained proof requires three actual items');
  assert.equal(new Set(items.map(item => item.name)).size, 3, 'DOM names must identify items unambiguously');
  const complete = (owner, item) => {
    const property = owner.face.properties.find(value => value.subject === item.identity && value.name === 'complete');
    assert.equal(typeof property?.value?.Flag, 'boolean');
    return property.value.Flag;
  };
  const observe = async () => {
    await ready();
    const view = await current();
    const owner = face();
    assert.equal(view.body_id, bodyId);
    assert.equal(owner.face.basis.body_id, bodyId);
    assert.equal(view.face_id, owner.face.identity);
    assert.equal(view.face_revision, owner.face_revision_decimal);
    const identity = item => ({ identity: item.identity, name: item.name, role: item.role });
    assert.deepEqual(owner.face.subjects.filter(subject => subject.role === 'Item').map(identity), items.map(identity));
    const dom = await page.locator('conduit-owner-face').evaluate(element => {
      const root = element.shadowRoot;
      return {
        count: root.querySelector('.owner-face-collection-count')?.textContent,
        lists: [...root.querySelectorAll('.owner-face-items')].map(list => ({
          label: list.getAttribute('aria-label'),
          names: [...list.querySelectorAll('.owner-face-item-name')].map(node => node.textContent),
        })),
        look: { expression: element.dataset.componentExpression,
          density: element.dataset.componentDensity, columns: element.dataset.componentColumns },
      };
    });
    const pending = items.filter(item => !complete(owner, item)).map(item => item.name);
    const completed = items.filter(item => complete(owner, item)).map(item => item.name);
    const status = owner.face.text.filter(text => text.subject === 'todo/status');
    assert.equal(status.length, 1, 'source must supply exact progress wording');
    assert.equal(status[0].text, `${pending.length} ${pending.length === 1 ? 'thing' : 'things'} left · ${completed.length} completed`);
    assert.equal(dom.count, status[0].text);
    assert.ok(dom.look.expression, 'shown Look must have an expression identity');
    assert.ok(dom.look.density, 'shown Look must have a density');
    assert.deepEqual(dom.lists.find(list => list.label?.startsWith('Things left in '))?.names ?? [], pending);
    assert.deepEqual(dom.lists.find(list => list.label?.startsWith('Completed items in '))?.names ?? [], completed);
    return { owner, view, dom };
  };
  const mutations = [];
  const perform = async (item, intent, label) => {
    const before = await observe();
    const action = before.view.actions.find(value => value.target === item.identity
      && value.intent === intent && value.availability === 'available');
    assert.ok(action, 'only an available supplied current action may be performed');
    assert.equal(action.arguments.length, 0);
    if (intent === 'todo/reopen@1') {
      const details = page.locator('.owner-face-completed');
      if (!await details.evaluate(element => element.open)) {
        await details.locator('summary').click();
      }
    }
    const button = page.getByRole('button', { name: `${action.name} ${item.name}`, exact: true });
    assert.equal(await button.count(), 1);
    assert.ok(await button.isEnabled());
    await button.click();
    await page.waitForFunction(prior => {
      const view = globalThis.__conduitOwnerParticipation.face();
      return view?.show_id !== prior && view?.show_state === 'available'
        && document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === view.show_id;
    }, before.view.show_id, { timeout: 12_000 });
    const after = await observe();
    assert.equal(after.dom.look.expression, before.dom.look.expression, 'domain action must preserve Look expression');
    assert.equal(after.dom.look.density, before.dom.look.density, 'domain action must preserve Look density');
    assert.equal(complete(after.owner, item), intent === 'todo/complete@1');
    for (const other of items.filter(value => value.identity !== item.identity)) {
      assert.equal(complete(after.owner, other), complete(before.owner, other));
    }
    const receipt = await observedTodoMutation(state, output, label, bodyId,
      { ...before.view, face_revision_decimal: before.owner.face_revision_decimal }, after.owner, action.identity);
    mutations.push({ label, action, before, after, receipt });
    return { before, after, action };
  };
  await observe();
  for (const item of items) {
    if (complete(face(), item)) await perform(item, 'todo/reopen@1', `reopen-${mutations.length + 1}`);
  }
  const before = await observe();
  assert.ok(items.every(item => !complete(before.owner, item)));
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-three-pending.png') });
  const completion = await perform(items[0], 'todo/complete@1', 'dom-complete');
  assert.equal(items.filter(item => !complete(completion.after.owner, item)).length, 2);
  const refusal = await page.evaluate(async ({ view, action }) => {
    try {
      const result = await globalThis.__conduitOwnerParticipation.submitFaceInteraction({
        view, actionId: action.identity, target: action.target, arguments: [],
      });
      return { refused: result.accepted === false };
    } catch (error) { return { refused: true, message: error.message }; }
  }, { view: completion.before.view, action: completion.action });
  assert.equal(refusal.refused, true, 'replaced initiating Show must refuse');
  assert.match(refusal.message ?? '', /^owner Face interaction refused \(-5\)$/,
    'stale return must reach the exact Show gate');
  const afterStaleOwner = face();
  assert.deepEqual(afterStaleOwner.face, completion.after.owner.face,
    'stale return must have no domain effect');
  const record = { initial, mutations, before, after: completion.after, stale_return: refusal,
    after_stale_owner: afterStaleOwner };
  await writeFile(path.join(output, 'retained-dom-completion.json'), `${JSON.stringify(record, null, 2)}\n`);
  return record;
}

// Three actual Shows of one retained Face. Selections/disclosure/focus are Mask-local.
export async function compareRetainedTodoLooks({ page, face, bodyId, hostId, bootId, output }) {
  const source = face();
  const observations = [];
  const semantics = view => ({ subjects: view.subjects, relationships: view.relationships,
    actions: view.actions, face_id: view.face_id, face_revision: view.face_revision, body_id: view.body_id });
  for (const look of [null, 'compact', 'spacious']) {
    const prior = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
    if (look) {
      await page.evaluate(value => globalThis.__conduitOwnerParticipation.selectFace(value, 'flat'), look);
    }
    await page.waitForFunction(({ previous, selected }) => {
      const view = globalThis.__conduitOwnerParticipation.face();
      const element = document.querySelector('conduit-owner-face');
      const expression = view?.component?.expression?.expression;
      return view?.show_state === 'available' && view.interactions_admitted
        && (!selected || (view.show_id !== previous && expression?.identity
          && expression.density === selected && element?.dataset.componentExpression === expression.identity
          && element?.dataset.componentDensity === selected
          && element?.dataset.componentColumns === String(view.component.maximum_columns)))
        && document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === view.show_id;
    }, { previous: prior.show_id, selected: look }, { timeout: 12_000 });
    const view = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
    if (!look) {
      assert.equal(Object.hasOwn(view, 'component'), false, 'generic serialized view must omit component evidence');
      const genericDataset = await page.locator('conduit-owner-face').evaluate(element => ({
        expression: Object.hasOwn(element.dataset, 'componentExpression'),
        density: Object.hasOwn(element.dataset, 'componentDensity'),
        columns: Object.hasOwn(element.dataset, 'componentColumns'),
      }));
      assert.deepEqual(genericDataset, { expression: false, density: false, columns: false },
        'actual generic renderer must clear checked Look datasets');
    }
    assert.equal(view.body_id, bodyId);
    assert.equal(view.route.mask_host_id, hostId);
    assert.equal(view.route.mask_boot_id, bootId);
    assert.equal(view.route.owner_host_id, source.advertisement.host_id);
    assert.equal(view.route.owner_boot_id, source.advertisement.boot_id);
    for (const key of ['mask_plot_id', 'mask_plan_id', 'mask_play_id', 'show_id']) assert.ok(view[key]);
    assert.deepEqual(face().face, source.face, 'Look must preserve whole canonical Face');
    assert.deepEqual(view.subjects.map(subject => [subject.identity, subject.name, subject.role]),
      source.face.subjects.map(subject => [subject.identity, subject.name, subject.role]));
    assert.deepEqual(view.actions.map(action => [action.identity, action.name, action.target, action.intent]),
      source.face.actions.map(action => [action.identity, action.name, action.target, action.intent]));
    for (const action of source.face.actions) {
      assert.equal(view.actions.find(current => current.identity === action.identity).availability,
        action.availability === 'Available' ? 'available'
          : Object.hasOwn(action.availability, 'Refused') ? 'refused' : 'unavailable');
    }
    assert.deepEqual(view.relationships, source.face.relationships.map(relation => ({
      source: relation.source, target: relation.target, kind: relation.kind })));
    if (observations.length) assert.deepEqual(semantics(view), semantics(observations[0].view),
      'all projected subject/action/relationship identities, order, disclosure and availability must survive');
    // Open disclosure once where necessary, without submitting any domain action.
    const summaries = page.locator('conduit-owner-face details > summary');
    for (let index = 0; index < await summaries.count(); index++) {
      const summary = summaries.nth(index);
      if (!await summary.evaluate(node => node.parentElement.open)) await summary.click();
    }
    const dom = await page.locator('conduit-owner-face').evaluate(element => {
      const root = element.shadowRoot;
      const collection = root.querySelector('.owner-face-collection');
      const style = getComputedStyle(collection);
      return { text: root.textContent,
        lists: [...root.querySelectorAll('.owner-face-items')].map(list => ({ label: list.getAttribute('aria-label'),
          names: [...list.querySelectorAll('.owner-face-item-name')].map(node => node.textContent) })),
        status: root.querySelector('.owner-face-collection-count')?.textContent,
        forms: [...root.querySelectorAll('[data-owner-action]')].map(form => ({ action_id: form.dataset.ownerAction,
          name: form.querySelector('button')?.textContent, disabled: form.querySelector('button')?.disabled })),
        component_dataset: { expression: element.dataset.componentExpression ?? null,
          density: element.dataset.componentDensity ?? null, columns: element.dataset.componentColumns ?? null },
        geometry: { collection_padding_px: Number.parseFloat(style.paddingTop),
          collection_radius_px: Number.parseFloat(style.borderTopLeftRadius) } };
    });
    for (const subject of view.subjects) {
      // Progress may be folded into collection wording; its essential source text must survive.
      if (subject.role === 'Status') {
        assert.ok(subject.text.length > 0);
        for (const text of subject.text) assert.ok(dom.text.includes(text), `DOM omitted status ${subject.identity}`);
      } else assert.ok(dom.text.includes(subject.name), `DOM omitted supplied subject name ${subject.identity}`);
    }
    const names = new Map(view.subjects.map(subject => [subject.identity, subject.name]));
    for (const relation of view.relationships) {
      assert.ok(dom.text.includes(`${names.get(relation.source) ?? relation.source} ${relation.kind.toLowerCase()} ${names.get(relation.target) ?? relation.target}`),
        'actual disclosed relationship endpoints and meaning must survive');
    }
    for (const control of dom.forms) {
      const action = view.actions.find(candidate => candidate.identity === control.action_id);
      assert.ok(action, 'DOM cannot invent action identity');
      assert.ok(control.name?.includes(action.name), 'actual accessible control names must retain supplied action wording');
      if (action.availability !== 'available') assert.equal(control.disabled, true);
      const button = page.locator(`[data-owner-action="${control.action_id}"]`).getByRole('button');
      control.visible = await button.isVisible();
      assert.ok(control.visible, 'disclosed action must be visible');
    }
    for (const action of view.actions.filter(action => action.availability === 'available')) {
      assert.ok(dom.forms.some(form => form.action_id === action.identity), 'available supplied action must remain discoverable');
    }
    const add = view.actions.find(action => action.identity === 'todo.add');
    assert.ok(add?.availability === 'available' && add.arguments.length === 1);
    const addForm = page.locator('[data-owner-action="todo.add"]');
    const input = addForm.getByRole('textbox', { name: add.arguments[0].value_name });
    assert.ok(await input.isVisible());
    await input.focus();
    await page.keyboard.press('Tab');
    const addButton = addForm.getByRole('button', { name: add.name, exact: true });
    dom.keyboard_focus = { action_id: add.identity,
      input: await input.evaluate(node => ({ type: node.type, maximum_length: node.maxLength,
        labels: [...node.labels].map(label => label.textContent) })),
      accessible_name: await addButton.textContent(),
      focused: await addButton.evaluate(node => node.getRootNode().activeElement === node),
      enabled: await addButton.isEnabled(), visible: await addButton.isVisible() };
    assert.ok(dom.keyboard_focus.focused, 'keyboard traversal must reach the supplied named action');
    assert.ok(dom.keyboard_focus.enabled && dom.keyboard_focus.visible);
    if (observations.length) {
      assert.deepEqual(dom.lists, observations[0].dom.lists);
      assert.equal(dom.status, observations[0].dom.status);
    }
    let retirement = null;
    if (look) {
      const staleAction = prior.actions.find(action => action.identity === 'todo.add');
      const refusal = await page.evaluate(async ({ stale, action }) => {
        try {
          await globalThis.__conduitOwnerParticipation.submitFaceInteraction({ view: stale,
            actionId: action.identity, target: action.target,
            arguments: [{ name: action.arguments[0].name, value: 'Retired Show must refuse' }] });
          return { refused: false };
        } catch (error) { return { refused: true, message: error.message }; }
      }, { stale: prior, action: staleAction });
      assert.equal(refusal.refused, true);
      assert.match(refusal.message, /^owner Face interaction refused \(-5\)$/);
      const afterRefusal = face();
      assert.deepEqual(afterRefusal.face, source.face, 'retired Look must have no owner effect');
      assert.equal(await page.evaluate(() => globalThis.__conduitOwnerParticipation.face().show_id), view.show_id);
      retirement = { retired_show_id: prior.show_id, current_show_id: view.show_id, refusal,
        after_refusal_owner_face: afterRefusal.face };
    }
    observations.push({ look: look ?? 'generic', recipe: look ? 'flat' : null, view, dom, retirement });
  }
  assert.ok(observations[2].dom.geometry.collection_padding_px > observations[1].dom.geometry.collection_padding_px,
    'two checked density Looks must change actual computed CSS');
  const record = { source, observations, proof_class: 'actual browser DOM, Mask-local disclosure/focus, no domain mutation' };
  await writeFile(path.join(output, 'retained-look-comparison.json'), `${JSON.stringify(record, null, 2)}\n`);
  return record;
}
