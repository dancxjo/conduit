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
  const items = initial.presentation.subjects.filter(subject => subject.role === 'Item');
  assert.equal(items.length, 3, 'retained proof requires three actual items');
  assert.equal(new Set(items.map(item => item.name)).size, 3, 'DOM names must identify items unambiguously');
  const complete = (owner, item) => {
    const property = owner.presentation.properties.find(value => value.subject === item.identity && value.name === 'complete');
    assert.equal(typeof property?.value?.Flag, 'boolean');
    return property.value.Flag;
  };
  const observe = async () => {
    await ready();
    const view = await current();
    const owner = face();
    assert.equal(view.body_id, bodyId);
    assert.equal(owner.presentation.basis.body_id, bodyId);
    assert.equal(view.face_id, owner.presentation.identity);
    assert.equal(view.face_revision, owner.presentation_revision_decimal);
    const identity = item => ({ identity: item.identity, name: item.name, role: item.role });
    assert.deepEqual(owner.presentation.subjects.filter(subject => subject.role === 'Item').map(identity), items.map(identity));
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
    const status = owner.presentation.text.filter(text => text.subject === 'todo/status');
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
      { ...before.view, face_revision_decimal: before.owner.presentation_revision_decimal }, after.owner, action.identity);
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
  assert.deepEqual(face().presentation, completion.after.owner.presentation,
    'stale return must have no domain effect');
  const record = { initial, mutations, before, after: completion.after, stale_return: refusal };
  await writeFile(path.join(output, 'retained-dom-completion.json'), `${JSON.stringify(record, null, 2)}\n`);
  return record;
}
