import assert from 'node:assert/strict';
import test from 'node:test';
import { configureWorkspaceInput, workspaceSurfacePlacements } from '../../targets/browser/workspace/workspace-surface.mjs';

const placement = (id, face = false) => ({ placement_id: id, kind_id: 'patchbay/accept-interaction',
  resources: face ? [{ class_id: 'conduit.resource/face-slot@1' }] : [] });
const chain = (...placements) => ({ plan: { fragments: [{ placements }] } });
const partition = (id, ...placements) => ({ plot: { checked_plot_id: id }, plan: chain(...placements).plan });
const topology = (id, ...chains) => ({ face: { plot: { checked_plot_id: id } }, chains });

test('selected Workspace presentation includes root and acquired Mask chain only', () => {
  const root = placement('root'), mask = placement('mask', true), standby = placement('standby', true);
  const plan = { plots: [partition('selected', root), partition('other', placement('other-root'))],
    mask_topologies: [topology('selected', chain(mask), chain(standby)), topology('other', chain(placement('other-mask', true)))] };
  assert.deepEqual(workspaceSurfacePlacements(plan, 'selected'), [root, mask]);
  assert.deepEqual(workspaceSurfacePlacements(plan, 'other').map(value => value.placement_id), ['other-root', 'other-mask']);
  assert.deepEqual(workspaceSurfacePlacements(plan, 'absent'), []);
  assert.deepEqual(workspaceSurfacePlacements(plan, null), []);
  assert.deepEqual(workspaceSurfacePlacements(undefined, 'selected'), []);
  assert.equal(plan.plots[0].plan.fragments[0].placements.length, 1, 'does not mutate admitted root');
});

test('an admitted selected Mask face slot keeps the output container visible without inventing input', () => {
  const input = { disabled: false, dataset: {}, attributes: {},
    toggleAttribute() {}, setAttribute(name, value) { this.attributes[name] = value; },
    removeAttribute(name) { delete this.attributes[name]; }, querySelector() { return this.prompt; }, prompt: {} };
  const root = placement('root'), mask = placement('mask', true);
  const plot = { title: 'Patchbay', required_kinds: [] };
  const current = partition('selected', root);
  configureWorkspaceInput(input, plot, current, '', [root, mask]);
  assert.equal(input.hidden, false);
  assert.equal(input.dataset.acceptsInput, 'false');
  assert.equal(input.attributes.role, 'group');
  assert.equal(input.tabIndex, -1);
  configureWorkspaceInput(input, plot, current, '', [root]);
  assert.equal(input.hidden, true, 'no ambient output without admitted face slot');
});
