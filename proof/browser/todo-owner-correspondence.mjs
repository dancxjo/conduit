// #5372 preparation: integrate beside the existing helper after #5355 acceptance.
// Requires a caller-owned retained Todo owner; this module never creates/reset state.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { observedTodoMutation } from './todo-owner-events.mjs';
import { refuseInvalidTodoReturns } from './todo-owner-correspondence-refusals.mjs';

export async function proveTodoOwnerCorrespondence({ page, face, state, output, bodyId, texts }) {
  assert.equal(texts.length, 2);
  assert.equal(new Set(texts).size, 2);
  for (const text of texts) {
    assert.equal(typeof text, 'string');
    assert.ok(text.length > 0 && Buffer.byteLength(text) <= 64);
  }
  const items = owner => owner.presentation.subjects.filter(subject => subject.role === 'Item');
  const current = () => page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  const ready = () => page.waitForFunction(() => {
    const view = globalThis.__conduitOwnerParticipation.face();
    return view?.show_state === 'available' && view.interactions_admitted
      && document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === view.show_id;
  }, null, { timeout: 12_000 });
  const executionBytes = () => readFile(path.join(state, 'body/owner-execution.json'));
  const execution = async () => JSON.parse((await executionBytes()).toString('utf8')).last_execution;
  const snapshot = async () => {
    await ready();
    const view = await current();
    const owner = face();
    assert.equal(owner.presentation.basis.body_id, bodyId);
    assert.equal(view.body_id, bodyId);
    assert.equal(view.face_id, owner.presentation.identity);
    assert.equal(view.face_revision, owner.presentation_revision_decimal);
    for (const key of ['mask_plot_id', 'mask_plan_id', 'mask_play_id', 'show_id']) assert.ok(view[key]);
    assert.ok(view.route.mask_host_id && view.route.mask_boot_id);
    assert.equal(view.route.owner_host_id, owner.advertisement.host_id);
    assert.equal(view.route.owner_boot_id, owner.advertisement.boot_id);
    return { view, owner };
  };
  const original = await snapshot();
  assert.notEqual(original.view.component?.expression?.expression?.density, 'compact',
    'caller must supply an initial encounter distinct from the first selected Look');
  for (const text of texts) assert.ok(!items(original.owner).some(item => item.name === text));
  const records = [];
  for (const [index, look] of ['compact', 'spacious'].entries()) {
    const previous = await snapshot();
    await page.evaluate(value => globalThis.__conduitOwnerParticipation.selectPresentation(value, 'flat'), look);
    await page.waitForFunction(({ previousShow, look }) => {
      const view = globalThis.__conduitOwnerParticipation.face();
      const expression = view?.component?.expression?.expression;
      return view?.show_id !== previousShow && expression?.density === look
        && view.show_state === 'available' && view.interactions_admitted
        && document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === view.show_id;
    }, { previousShow: previous.view.show_id, look }, { timeout: 12_000 });
    const before = await snapshot();
    assert.deepEqual(before.owner.presentation, previous.owner.presentation);
    const rendered = await page.locator('conduit-owner-face').evaluate(element => ({
      expression: element.dataset.componentExpression, density: element.dataset.componentDensity,
      columns: element.dataset.componentColumns }));
    assert.equal(rendered.expression, before.view.component.expression.expression.identity);
    assert.equal(rendered.density, look);
    assert.equal(rendered.columns, String(before.view.component.maximum_columns));
    const action = before.view.actions.find(candidate => candidate.identity === 'todo.add');
    assert.ok(action && action.availability === 'available');
    assert.equal(action.arguments.length, 1);
    const argument = action.arguments[0];
    const invalidReturns = await refuseInvalidTodoReturns({ page, face, current, execution,
      before, action, text: texts[index] });
    const form = page.locator('[data-owner-action="todo.add"]');
    assert.equal(await form.count(), 1);
    const input = form.getByRole('textbox', { name: argument.value_name, exact: true });
    const button = form.getByRole('button', { name: action.name, exact: true });
    assert.equal(await input.count(), 1);
    assert.equal(await button.count(), 1);
    assert.ok(await input.isVisible() && await input.isEnabled());
    assert.ok(await button.isVisible() && await button.isEnabled());
    await input.fill(texts[index]);
    let event;
    if (index === 0) {
      await button.click();
      event = { type: 'pointer', operation: 'click' };
    } else {
      await page.keyboard.press('Tab');
      assert.ok(await button.evaluate(node => node.getRootNode().activeElement === node));
      await page.keyboard.press('Enter');
      event = { type: 'keyboard', key: 'Enter', traversal: ['Tab'] };
    }
    await page.waitForFunction(prior => {
      const view = globalThis.__conduitOwnerParticipation.face();
      return view?.show_id !== prior && view.show_state === 'available'
        && document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === view.show_id;
    }, before.view.show_id, { timeout: 12_000 });
    const after = await snapshot();
    assert.equal(items(after.owner).length, items(before.owner).length + 1);
    for (const item of items(before.owner)) {
      assert.deepEqual(items(after.owner).find(candidate => candidate.identity === item.identity), item);
      const properties = owner => owner.presentation.properties.filter(property => property.subject === item.identity);
      assert.deepEqual(properties(after.owner), properties(before.owner));
    }
    const added = items(after.owner).filter(item => item.name === texts[index]);
    assert.equal(added.length, 1);
    assert.ok(!items(before.owner).some(item => item.identity === added[0].identity));
    const originalReceiptBytes = await executionBytes();
    const committed = JSON.parse(originalReceiptBytes.toString('utf8')).last_execution;
    const originalReceiptFile = `correspondence-${event.type}-owner-execution.json`;
    await writeFile(path.join(output, originalReceiptFile), originalReceiptBytes, { mode: 0o600, flag: 'wx' });
    const originalReceipt = { file: originalReceiptFile,
      sha256: createHash('sha256').update(originalReceiptBytes).digest('hex') };
    const initiating = committed.write.initiating_action;
    const shown = committed.write.initiating_show;
    assert.equal(shown.show_id, before.view.show_id);
    assert.equal(shown.presentation_id, before.view.face_id);
    assert.equal(shown.mask_plot.checked_plot_id, before.view.mask_plot_id);
    assert.equal(shown.planned_mask.plan.plan_id, before.view.mask_plan_id);
    assert.equal(shown.show.plan_id, before.view.mask_plan_id);
    assert.equal(shown.show.active_play_id, before.view.mask_play_id);
    assert.equal(shown.show.host_id, before.view.route.mask_host_id);
    assert.equal(shown.show.boot_id, before.view.route.mask_boot_id);
    for (const terminal of [committed.write.terminal_sign, committed.read_terminal_sign]) {
      assert.equal(terminal.host_id, before.owner.advertisement.host_id);
      assert.equal(terminal.boot_id, before.owner.advertisement.boot_id);
    }
    assert.notEqual(committed.write.plan_id, before.view.mask_plan_id);
    assert.notEqual(committed.read_plan_id, before.view.mask_plan_id);
    assert.equal(initiating.action_id, action.identity);
    assert.equal(initiating.target, action.target);
    assert.deepEqual(initiating.arguments, [{ name: argument.name, value_kind: argument.value_kind,
      value: Array.from(Buffer.from(texts[index], 'utf8')) }]);
    const receipt = await observedTodoMutation(state, output, `correspondence-${event.type}`, bodyId,
      { ...before.view, face_revision_decimal: before.owner.presentation_revision_decimal }, after.owner, action.identity);
    const refusal = await page.evaluate(async ({ view, action, text }) => {
      try {
        await globalThis.__conduitOwnerParticipation.submitFaceInteraction({ view,
          actionId: action.identity, target: action.target,
          arguments: [{ name: action.arguments[0].name, value: text }] });
        return { refused: false };
      } catch (error) { return { refused: true, message: error.message }; }
    }, { view: before.view, action, text: texts[index] });
    assert.equal(refusal.refused, true);
    assert.match(refusal.message, /^owner Face interaction refused \(-5\)$/);
    assert.deepEqual(face().presentation, after.owner.presentation);
    assert.deepEqual(await execution(), committed);
    records.push({ look, event, action, before, after, invalid_returns: invalidReturns, added_subject: added[0], receipt,
      original_owner_receipt: originalReceipt, stale_refusal: refusal });
  }
  assert.equal(records[0].action.identity, records[1].action.identity);
  assert.equal(records[0].action.target, records[1].action.target);
  assert.deepEqual(records[0].action.arguments, records[1].action.arguments);
  for (const key of ['mask_plot_id', 'mask_plan_id', 'mask_play_id', 'show_id']) {
    assert.notEqual(records[0].before.view[key], records[1].before.view[key]);
  }
  assert.equal(records[1].before.view.route.mask_host_id, records[0].before.view.route.mask_host_id);
  assert.equal(records[1].before.view.route.mask_boot_id, records[0].before.view.route.mask_boot_id);
  const record = { schema: 'conduit.proof/todo-owner-correspondence@1', body_id: bodyId,
    proof_class: 'actual browser events, owner-issued Mask routes and verified committed Todo effects', records };
  await writeFile(path.join(output, 'todo-owner-correspondence.json'), `${JSON.stringify(record, null, 2)}\n`,
    { mode: 0o600, flag: 'wx' });
  return record;
}
