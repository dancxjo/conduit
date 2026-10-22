import test from 'node:test';
import assert from 'node:assert/strict';
import { configureWorkspaceInput } from '../../targets/browser/workspace/workspace-surface.mjs';

test('a selected Mask Face keeps its surface visible without granting ordinary input', () => {
  const attributes = new Map();
  const prompt = {};
  const input = { disabled: false, dataset: {},
    toggleAttribute(name, value) { if (value) attributes.set(name, ''); else attributes.delete(name); },
    setAttribute(name, value) { attributes.set(name, value); },
    removeAttribute(name) { attributes.delete(name); },
    querySelector() { return prompt; } };
  const plot = { title: 'Patchbay' };
  const partition = { plan: { fragments: [{ placements: [] }] } };
  configureWorkspaceInput(input, plot, partition, '');
  assert.equal(input.hidden, true);
  configureWorkspaceInput(input, plot, partition, '', [{ kind_id: 'input/pointer-source',
    resources: [{ class_id: 'conduit.resource/face-slot@1' }] }]);
  assert.equal(input.hidden, false);
  assert.equal(input.dataset.acceptsInput, 'false');
  assert.equal(input.tabIndex, -1);
  assert.equal(prompt.hidden, true);
  assert.equal(attributes.get('role'), 'group');
  assert.equal(attributes.get('aria-label'), 'Output of Patchbay');
});
