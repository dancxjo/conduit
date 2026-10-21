import test from 'node:test';
import assert from 'node:assert/strict';
import { resolvePatchbayEditorTarget } from './patchbay-editor.mjs';

function fixture() {
  const plot = Object.freeze({ name: 'lesson', checkedPlotId: 'checked/lesson', sourceDocumentId: 'source/lesson', source: 'plot lesson {}' });
  const property = (subject, name, value) => ({ subject, name, value: { Identity: value } });
  const observation = { pending_owner_request: { request: { kind: 'EditCurrent', expanded_plot_id: 'expanded/lesson', subject_identity: 'raw/port' } }, face: {
    subjects: [{ identity: 'plot/checked/lesson', name: 'Untrusted caption', role: 'Plot' },
      { identity: 'gear/lesson', name: 'value Gear', role: 'Gear' }, { identity: 'port/lesson', name: 'value output', role: 'Port' }],
    properties: [property('plot/checked/lesson', 'expanded-plot-id', 'expanded/lesson'),
      property('plot/checked/lesson', 'source-document-id', 'source/lesson'), property('port/lesson', 'semantic-id', 'raw/port'),
      property('gear/lesson', 'source-plot', 'lesson')],
    relationships: [{ source: 'plot/checked/lesson', target: 'gear/lesson', kind: 'Contains' }, { source: 'gear/lesson', target: 'port/lesson', kind: 'Contains' }],
  } };
  return { plot, observation };
}

test('Patchbay Edit resolves exact checked Source and subject independently of captions', () => {
  const { plot, observation } = fixture();
  const target = resolvePatchbayEditorTarget(observation, [plot]);
  assert.equal(target.plot, plot);
  assert.equal(target.subject.identity, 'port/lesson');
  assert.equal(target.plot.source, 'plot lesson {}');
  assert(Object.isFrozen(target));
});

test('Patchbay Edit refuses foreign, ambiguous, nested and cyclic Source ownership', () => {
  for (const alter of [
    value => { value.pending_owner_request.request.expanded_plot_id = 'expanded/foreign'; },
    value => { value.pending_owner_request.request.subject_identity = 'raw/foreign'; },
    value => { value.face.properties[1].value.Identity = 'source/foreign'; },
    value => { value.face.properties.push(structuredClone(value.face.properties[2])); },
    value => { value.face.properties.push({ subject: 'gear/lesson', name: 'recursive-parent', value: { Identity: 'other' } }); },
    value => { value.face.properties[3].value.Identity = 'nested-implementation'; },
    value => { value.face.relationships[0].source = 'port/lesson'; },
    value => { value.face.properties[0].value = { Text: 'expanded/lesson' }; },
  ]) {
    const { plot, observation } = fixture(); alter(observation);
    assert.throws(() => resolvePatchbayEditorTarget(observation, [plot]), error => error.code === 'PatchbayEditRefused');
  }
});
