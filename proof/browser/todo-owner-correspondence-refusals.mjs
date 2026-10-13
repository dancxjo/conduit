// Negative probes only: positive agency must come from actual DOM input events.
import assert from 'node:assert/strict';

export async function refuseInvalidTodoReturns({ page, face, current, execution, before, action, text }) {
  const declaration = action.arguments[0];
  assert.ok(Number.isSafeInteger(declaration.maximum_bytes));
  assert.ok(declaration.maximum_bytes >= 0 && declaration.maximum_bytes < 4096);
  const argument = { name: declaration.name, value: text };
  const unavailable = 'Choose one currently available Face action and its declared inputs.';
  const wrongBound = 'Face action input differs from its declared bound.';
  const stale = 'owner Face interaction refused (-5)';
  const cases = [
    { label: 'invented-action', actionId: 'correspondence/invented-action', expected: unavailable },
    { label: 'wrong-target', target: 'correspondence/wrong-target', expected: unavailable },
    { label: 'missing-argument', arguments: [], expected: unavailable },
    { label: 'undeclared-argument', arguments: [{ name: 'correspondence/undeclared', value: text }], expected: wrongBound },
    { label: 'over-bound-argument', arguments: [{ name: declaration.name,
      value: 'x'.repeat(declaration.maximum_bytes + 1) }], expected: wrongBound },
    { label: 'stale-face', view: { ...before.view, face_id: 'correspondence/stale-face' }, expected: stale },
    { label: 'stale-revision', view: { ...before.view,
      face_revision: before.view.face_revision === '0' ? '1' : '0' }, expected: stale },
    { label: 'stale-show', view: { ...before.view, show_id: 'show/correspondence-stale' }, expected: stale },
  ];
  const alteredActionView = changed => ({ ...before.view,
    actions: before.view.actions.map(candidate => candidate.identity === action.identity ? changed : candidate) });
  cases.push(
    { label: 'mask-rejects-invented-view-action', actionId: 'correspondence/invented-action',
      view: { ...before.view, actions: [...before.view.actions,
        { ...action, identity: 'correspondence/invented-action' }] }, expected: stale },
    { label: 'mask-rejects-redirected-view-target', target: 'correspondence/wrong-target',
      view: alteredActionView({ ...action, target: 'correspondence/wrong-target' }), expected: stale },
    { label: 'mask-rejects-inflated-view-bound',
      view: alteredActionView({ ...action, arguments: [{ ...declaration,
        maximum_bytes: declaration.maximum_bytes + 1 }] }),
      arguments: [{ name: declaration.name, value: 'x'.repeat(declaration.maximum_bytes + 1) }], expected: stale },
  );
  const disabled = before.view.actions.find(candidate => candidate.availability !== 'available'
    && candidate.arguments.length === 0);
  assert.ok(disabled, 'fixture must supply an actual unavailable source action');
  cases.push(
    { label: 'unavailable-source-action', actionId: disabled.identity, target: disabled.target,
      arguments: [], expected: unavailable },
    { label: 'mask-rejects-forged-availability', actionId: disabled.identity, target: disabled.target,
      arguments: [], view: { ...before.view, actions: before.view.actions.map(candidate =>
        candidate.identity === disabled.identity ? { ...disabled, availability: 'available' } : candidate) },
      expected: stale },
  );
  const priorExecution = await execution();
  const refusals = [];
  for (const candidate of cases) {
    const proposal = { view: candidate.view ?? before.view,
      actionId: candidate.actionId ?? action.identity, target: candidate.target ?? action.target,
      arguments: candidate.arguments ?? [argument] };
    const result = await page.evaluate(async proposal => {
      try {
        const outcome = await globalThis.__conduitOwnerParticipation.submitFaceInteraction(proposal);
        return { refused: outcome?.accepted === false, message: outcome?.message ?? null };
      } catch (error) { return { refused: true, message: error.message }; }
    }, proposal);
    assert.equal(result.refused, true, candidate.label);
    assert.equal(result.message, candidate.expected, candidate.label);
    assert.deepEqual(face().presentation, before.owner.presentation, `${candidate.label}: semantic state`);
    assert.deepEqual(await execution(), priorExecution, `${candidate.label}: committed execution`);
    const after = await current();
    for (const key of ['face_id', 'face_revision', 'show_id', 'mask_plan_id', 'mask_play_id']) {
      assert.equal(after[key], before.view[key], `${candidate.label}: ${key}`);
    }
    assert.equal(after.show_state, 'available');
    assert.equal(after.interactions_admitted, true);
    refusals.push({ label: candidate.label, action_id: proposal.actionId, target: proposal.target,
      face_id: proposal.view.face_id, face_revision_decimal: proposal.view.face_revision,
      show_id: proposal.view.show_id, forged_view_metadata: candidate.view !== undefined,
      expected_refusal: candidate.expected, refusal: result });
  }
  return refusals;
}
