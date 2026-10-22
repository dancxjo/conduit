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

test('Patchbay Edit refuses foreign, ambiguous, unproven and cyclic Source ownership', () => {
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

function nestedFixture() {
  const value = fixture();
  const back = Object.freeze({ name: 'back', checkedPlotId: 'checked/back', sourceDocumentId: 'source/back', source: 'plot back {}' });
  value.observation.face.subjects.push({ identity: 'gear/composition', role: 'Gear', name: 'A caption is not a source address' });
  value.observation.face.properties[3].value.Identity = 'back';
  value.observation.face.properties.push(
    { subject: 'gear/composition', name: 'checked-back-id', value: { Identity: back.checkedPlotId } },
    { subject: 'gear/lesson', name: 'recursive-parent', value: { Identity: 'gear/composition' } });
  value.observation.face.relationships[0].source = 'gear/composition';
  value.observation.face.relationships.push({ source: 'plot/checked/lesson', target: 'gear/composition', kind: 'Contains' });
  return { ...value, back };
}

test('nested Patchbay Edit follows the exact containing checked Back declaration', () => {
  const { plot, observation, back } = nestedFixture();
  const target = resolvePatchbayEditorTarget(observation, [plot, back]);
  assert.equal(target.plot, back);
  assert.equal(target.subject.identity, 'port/lesson');
  // The composition invocation itself is authored in the outer document.
  observation.pending_owner_request.request.subject_identity = 'raw/composition';
  observation.face.properties.push({ subject: 'gear/composition', name: 'semantic-id', value: { Identity: 'raw/composition' } });
  assert.equal(resolvePatchbayEditorTarget(observation, [plot, back]).plot, plot);
});

test('nested Edit refuses unavailable or ambiguous Backs and inconsistent provenance', () => {
  const missing = nestedFixture();
  assert.throws(() => resolvePatchbayEditorTarget(missing.observation, [missing.plot]), /absent/);
  assert.throws(() => resolvePatchbayEditorTarget(missing.observation, [missing.plot, missing.back, missing.back]), /absent/);
  const wrong = nestedFixture(); wrong.observation.face.properties[3].value.Identity = 'different-source';
  assert.throws(() => resolvePatchbayEditorTarget(wrong.observation, [wrong.plot, wrong.back]), /differs/);
  const cord = nestedFixture();
  cord.observation.face.subjects[2].role = 'Cord';
  assert.throws(() => resolvePatchbayEditorTarget(cord.observation, [cord.plot, cord.back]), /Cord.*provenance/);
});

function cordFixture() {
  const value = fixture();
  value.observation.face.subjects[2].role = 'Cord';
  const subject = 'port/lesson';
  const facts = [
    ['source-origin', { Identity: 'cord-stages' }],
    ['source-plot', { Identity: 'lesson' }],
    ['checked-source-plot-id', { Identity: value.plot.checkedPlotId }],
    ['source-document-id', { Identity: value.plot.sourceDocumentId }],
    ['source-byte-start', { Count: 5 }], ['source-byte-end', { Count: 11 }],
  ];
  value.observation.face.properties.push(...facts.map(([name, value]) => ({ subject, name, value })));
  return value;
}

test('Cord Edit requires exact declaration facts and selects its Source extent', () => {
  const { plot, observation } = cordFixture();
  const target = resolvePatchbayEditorTarget(observation, [plot]);
  assert.equal(target.plot, plot);
  assert.deepEqual(target.sourceRange, { start: 5, end: 11 });
  for (const name of ['source-origin', 'checked-source-plot-id', 'source-document-id', 'source-byte-start']) {
    const copy = structuredClone(observation);
    copy.face.properties = copy.face.properties.filter(property => property.subject !== 'port/lesson' || property.name !== name);
    assert.throws(() => resolvePatchbayEditorTarget(copy, [plot]), /provenance/);
  }
});

test('Cord Edit refuses generated, stale, duplicate and invalid Source facts', () => {
  for (const [name, value] of [
    ['source-origin', { Identity: 'semantic-expression' }],
    ['source-origin', { Identity: 'relational-operand' }],
    ['source-document-id', { Identity: 'stale/document' }],
    ['checked-source-plot-id', { Identity: 'stale/plot' }],
    ['source-byte-end', { Count: 999 }],
    ['source-byte-start', { Count: -1 }],
  ]) {
    const { plot, observation } = cordFixture();
    observation.face.properties.find(property => property.subject === 'port/lesson' && property.name === name).value = value;
    assert.throws(() => resolvePatchbayEditorTarget(observation, [plot]), error => error.code === 'PatchbayEditRefused');
  }
  const { plot, observation } = cordFixture();
  observation.face.properties.push(structuredClone(observation.face.properties.at(-1)));
  assert.throws(() => resolvePatchbayEditorTarget(observation, [plot]), /provenance/);
});

test('Cord Source byte extents preserve Unicode and reject split code points', () => {
  const { plot, observation } = cordFixture();
  const unicode = { ...plot, source: '// 🧭\nplot lesson {}' };
  const start = new TextEncoder().encode('// 🧭\nplot ').length;
  const property = name => observation.face.properties.find(property => property.subject === 'port/lesson' && property.name === name);
  property('source-byte-start').value.Count = start;
  property('source-byte-end').value.Count = start + 6;
  const target = resolvePatchbayEditorTarget(observation, [unicode]);
  assert.equal(unicode.source.slice(target.sourceRange.start, target.sourceRange.end), 'lesson');
  property('source-byte-start').value.Count = 4;
  assert.throws(() => resolvePatchbayEditorTarget(observation, [unicode]), /Unicode/);
});
