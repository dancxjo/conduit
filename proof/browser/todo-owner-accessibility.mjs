// Browser semantic proof over the actual owner's acknowledged generic Todo Face.
// Reading/focus are Mask-local; the existing producer owns trusted Add and refusals.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

export async function captureTodoAccessibility({ page, face, state, output, sourceRecord, stage }) {
  const owner = face();
  const view = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(view.face_id, owner.face.identity);
  assert.equal(view.face_revision, owner.face_revision_decimal);
  assert.equal(view.body_id, owner.face.basis.body_id);
  assert.equal(view.show_state, 'available');
  assert.equal(view.interactions_admitted, true);
  assert.equal(await page.locator('[data-owner-show-acknowledged]').getAttribute('data-owner-show-acknowledged'), view.show_id);
  const element = page.locator('conduit-owner-face');
  assert.deepEqual(await element.evaluate(node => [node.dataset.faceId, node.dataset.faceRevision, node.dataset.showId]),
    [view.face_id, view.face_revision, view.show_id]);
  const canonical = new Map(owner.face.subjects.map(subject => [subject.identity, subject]));
  for (const subject of view.subjects) {
    assert.equal(subject.name, canonical.get(subject.identity)?.name);
    assert.equal(subject.role, canonical.get(subject.identity)?.role);
  }
  const executionPath = path.join(state, 'body/owner-execution.json');
  const execution = await readFile(executionPath);
  const inspect = async () => {
    // This receipt deliberately requires the generic source-facts route, rather
    // than treating a specialized checklist or fixture as fallback evidence.
    assert.equal(await element.locator('.owner-face-collection').count(), 0);
    const subjects = [];
    for (const subject of view.subjects) {
      const heading = element.getByRole('heading', { name: subject.name, exact: true });
      if (subject.disclosure !== 'Primary') continue;
      assert.equal(await heading.count(), 1);
      assert.equal(await heading.isVisible(), true);
      const section = heading.locator('..');
      const values = await section.locator('dl').evaluateAll(nodes => nodes.flatMap(node =>
        [...node.querySelectorAll('dt')].map(term => ({ name: term.textContent, text: term.nextElementSibling?.textContent }))));
      assert.deepEqual(values, (subject.values ?? []).map(({ name, text }) => ({ name, text })));
      subjects.push({ identity: subject.identity, role: subject.role, name: subject.name,
        values, text: subject.text, properties: subject.properties });
    }
    const controls = [];
    for (const form of await element.locator('form[data-owner-action]').all()) {
      if (!await form.isVisible()) continue;
      const actionId = await form.getAttribute('data-owner-action');
      const action = view.actions.find(candidate => candidate.identity === actionId);
      const source = owner.face.actions.find(candidate => candidate.identity === actionId);
      assert.ok(source && action);
      assert.deepEqual([action.name, action.target, action.intent], [source.name, source.target, source.intent]);
      const button = form.getByRole('button', { name: source.name, exact: true });
      assert.equal(await button.count(), 1);
      assert.equal(await button.isDisabled(), action.availability !== 'available');
      assert.deepEqual(await button.evaluate(node => JSON.parse(node.dataset.ownerFocus)),
        [view.body_id, source.identity, source.target, 'submit']);
      const argumentsSeen = [];
      for (const argument of action.arguments) {
        const sourceArgument = source.arguments.find(candidate => candidate.name === argument.name);
        assert.ok(sourceArgument);
        assert.equal(argument.value_name, sourceArgument.value_name);
        const minimum = Math.max(0, ...sourceArgument.contract.constraints.flatMap(constraint =>
          constraint.ByteLength ? [constraint.ByteLength.minimum] : []));
        assert.equal(argument.minimum_bytes, minimum);
        assert.equal(argument.maximum_bytes, sourceArgument.contract.maximum_bytes);
        const input = form.getByRole(argument.choices.length ? 'combobox' : 'textbox',
          { name: sourceArgument.value_name, exact: true });
        assert.equal(await input.count(), 1);
        assert.equal(await input.isDisabled(), action.availability !== 'available');
        const required = await input.evaluate(node => node.required);
        assert.equal(required, argument.choices.length > 0 || minimum > 0);
        argumentsSeen.push({ name: argument.name, accessible_name: argument.value_name,
          minimum_bytes: minimum, maximum_bytes: argument.maximum_bytes, native_required: required });
      }
      controls.push({ action_id: source.identity, target: source.target, name: source.name,
        disabled: await button.isDisabled(), arguments: argumentsSeen });
    }
    assert.ok(controls.some(control => control.action_id === 'todo.add'));
    return { subjects, controls, accessibility_tree: await element.ariaSnapshot() };
  };
  const styled = await inspect();
  let unstyled;
  await withTodoStylesDisabled(page, async () => {
    unstyled = await inspect();
    assert.deepEqual(unstyled.subjects, styled.subjects);
    assert.deepEqual(unstyled.controls, styled.controls);
    const add = view.actions.find(action => action.identity === 'todo.add');
    const form = element.locator('form[data-owner-action="todo.add"]');
    await form.getByRole('textbox', { name: add.arguments[0].value_name, exact: true }).focus();
    await page.keyboard.press('Tab');
    assert.equal(await form.getByRole('button', { name: add.name, exact: true })
      .evaluate(node => node.getRootNode().activeElement === node), true);
  });
  assert.deepEqual(face(), owner, 'semantic reading and focus cannot mutate the actual owner');
  assert.deepEqual(await readFile(executionPath), execution);
  const receipt = { schema: 'conduit.proof/browser-todo-accessibility@1', ...sourceRecord,
    stage, body_id: view.body_id, face_id: view.face_id, face_revision: view.face_revision,
    show_id: view.show_id, route: view.route, canonical_face: owner.face, styled, unstyled,
    proof_class: 'actual owner browser accessible semantics and unstyled native focus; no screen-reader enactment',
    owner_unchanged_by_inspection: true };
  await writeFile(path.join(output, `accessibility-${stage}.json`), `${JSON.stringify(receipt, null, 2)}\n`);
  return receipt;
}

// The caller keeps its normal action/receipt path, including when Add changes
// the Face. Only existing presentation sheets are temporarily disabled.
export async function withTodoStylesDisabled(page, action) {
  await page.locator('conduit-owner-face').evaluate(node => {
    globalThis.__todoAccessibilityStyles = [...document.styleSheets, ...node.shadowRoot.styleSheets]
      .map(sheet => ({ sheet, disabled: sheet.disabled }));
    for (const { sheet } of globalThis.__todoAccessibilityStyles) sheet.disabled = true;
  });
  try {
    return await action();
  } finally {
    await page.evaluate(() => {
      for (const { sheet, disabled } of globalThis.__todoAccessibilityStyles) sheet.disabled = disabled;
      delete globalThis.__todoAccessibilityStyles;
    });
  }
}
