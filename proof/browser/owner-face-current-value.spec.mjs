import { expect, test } from '@playwright/test';

// Renderer fixture only: the callback is not a Body/action receipt.
const view = {
  body_id: 'fixture/body', face_id: 'fixture/face', face_revision: 1, show_id: 'fixture/show',
  show_state: 'available', interactions_admitted: true,
  subjects: [{ identity: 'fixture/task', name: 'Read the handbook', role: 'Info',
    disclosure: 'Primary', text: [], properties: [], unmodeled_property_count: 0,
    values: [{ name: 'Completion', text: 'Open' }] }], relationships: [],
  actions: [{ identity: 'fixture/complete', target: 'fixture/task', name: 'Complete task',
    availability: 'available', disclosure: 'CurrentAction', arguments: [] }],
};

test('typed current text opens a multiline editor and submits only an explicit replacement', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  const document = { ...view,
    subjects: [{ ...view.subjects[0], role: 'TextEntry', name: 'Thoughts' }],
    actions: [{ ...view.actions[0], identity: 'notebook.replace', name: 'Replace text',
      arguments: [{ name: 'text', value_name: 'Document text', value_kind: 'value/text',
        minimum_bytes: 0, maximum_bytes: 256, choices: [], current_value: 'I was here.\nAgain.' }] }],
  };
  await page.evaluate(async view => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    const element = document.createElement('conduit-owner-face');
    globalThis.calls = [];
    element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
    document.body.replaceChildren(element);
    const { assertOwnerFaceReachability } = await import('/targets/browser/handbook/owner-face-reachability.mjs');
    assertOwnerFaceReachability(element, view);
    element.disabled = true;
    assertOwnerFaceReachability(element, { ...view, interactions_admitted: false });
    if (!element.shadowRoot.querySelector('textarea').disabled) throw new Error('Unavailable editor remains enabled');
    element.disabled = false;
  }, document);
  const face = page.locator('conduit-owner-face');
  const field = face.getByLabel('Document text');
  await expect(field).toHaveValue('I was here.\nAgain.');
  expect(await field.evaluate(node => node.tagName)).toBe('TEXTAREA');
  await field.fill('Still here.\nOn another line.');
  expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
  await face.getByRole('button', { name: 'Replace text', exact: true }).click();
  await expect(field).toBeDisabled();
  expect(await page.evaluate(() => globalThis.calls[0].arguments)).toEqual([
    { name: 'text', value: 'Still here.\nOn another line.' },
  ]);
});
