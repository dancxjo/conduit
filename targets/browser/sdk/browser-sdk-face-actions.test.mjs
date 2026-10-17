import assert from 'node:assert/strict';
import test from 'node:test';
import { checkedOwnerFaceArguments } from './browser-sdk-face-actions.mjs';

const action = {
  identity: 'body/action/name-and-interval', target: 'body/current', availability: 'available',
  arguments: [
    { name: 'body/name', value_kind: 'value/text', maximum_bytes: 8, choices: [] },
    { name: 'clock/interval-ms', value_kind: 'value/text', maximum_bytes: 4, choices: ['250', '500'] },
  ],
};
const view = { actions: [action] };
const submit = arguments_ => checkedOwnerFaceArguments(view, action.identity, action.target, arguments_);

test('browser Face controls pass only named, bounded values from the current action', () => {
  assert.deepEqual(submit([
    { name: 'body/name', value: 'Clock' },
    { name: 'clock/interval-ms', value: '500' },
  ]), [
    { name: 'body/name', value: 'Clock' },
    { name: 'clock/interval-ms', value: '500' },
  ]);
  assert.throws(() => submit([{ name: 'body/name', value: 'Clock' }]), /declared inputs/);
  assert.throws(() => submit([
    { name: 'body/name', value: 'Clock' }, { name: 'body/name', value: 'Other' },
  ]), /duplicated/);
  assert.throws(() => submit([
    { name: 'body/name', value: 'Clock' }, { name: 'clock/interval-ms', value: '750' },
  ]), /supported browser text form/);
  assert.throws(() => submit([
    { name: 'body/name', value: 'ééééé' }, { name: 'clock/interval-ms', value: '250' },
  ]), /declared bound/);
  assert.throws(() => checkedOwnerFaceArguments(view, action.identity, 'body/other', []), /currently available/);
  assert.throws(() => checkedOwnerFaceArguments({ actions: [{ ...action, availability: 'refused' }] },
    action.identity, action.target, []), /currently available/);
});

test('actions with no arguments are ordinary buttons; nontext without choices is unavailable', () => {
  assert.deepEqual(checkedOwnerFaceArguments({ actions: [{ ...action, arguments: [] }] },
    action.identity, action.target, []), []);
  assert.throws(() => checkedOwnerFaceArguments({ actions: [{ ...action, arguments: [
    { name: 'opaque', value_kind: 'value/binary', maximum_bytes: 4, choices: [] },
  ] }] }, action.identity, action.target, [{ name: 'opaque', value: 'text' }]),
  /no supported browser text form/);
});

test('browser Face argument admission refuses malformed declaration names and whole choice tables', () => {
  const variants = [
    { name: '', choices: ['yes'] },
    { name: '   ', choices: ['yes'] },
    { name: 'value', choices: ['yes', 42] },
    { name: 'value', choices: ['yes', 'ééé'] },
    { name: 'value', choices: ['yes', ''] },
  ];
  const accepted = variants.map(declaration => {
    const current = { ...action, arguments: [{ value_kind: 'value/text', maximum_bytes: 4, ...declaration }] };
    try {
      checkedOwnerFaceArguments({ actions: [current] }, current.identity, current.target,
        [{ name: declaration.name, value: 'yes' }]);
      return true;
    } catch { return false; }
  });
  assert.deepEqual(accepted, variants.map(() => false));
  const current = { ...action, arguments: [{ name: 'value', value_kind: 'value/text', maximum_bytes: 4, choices: ['yes', 'éé'] }] };
  assert.deepEqual(checkedOwnerFaceArguments({ actions: [current] }, current.identity, current.target,
    [{ name: 'value', value: 'éé' }]), [{ name: 'value', value: 'éé' }]);
});

test('owner interaction arguments bind to the last acknowledged Show and its declarations', async () => {
  const { checkedAcknowledgedOwnerFaceArguments } = await import('./browser-sdk-face-actions.mjs');
  const acknowledged = { ...view, body_id: 'body/one', show_id: 'show/one', face_id: 'face/one', face_revision: 7,
    show_state: 'available', interactions_admitted: true };
  const supplied = [{ name: 'body/name', value: 'Clock' }, { name: 'clock/interval-ms', value: '500' }];
  assert.deepEqual(checkedAcknowledgedOwnerFaceArguments(acknowledged, structuredClone(acknowledged),
    action.identity, action.target, supplied), supplied);
  for (const field of ['body_id', 'show_id', 'face_id', 'face_revision']) {
    const stale = { ...acknowledged, [field]: field === 'face_revision' ? 8 : 'other' };
    assert.throws(() => checkedAcknowledgedOwnerFaceArguments(acknowledged, stale,
      action.identity, action.target, supplied), /acknowledged owner Show/);
  }
  assert.throws(() => checkedAcknowledgedOwnerFaceArguments(null, acknowledged,
    action.identity, action.target, supplied), /acknowledged owner Show/);
  for (const state of [{ show_state: 'prepared' }, { interactions_admitted: false }]) {
    assert.throws(() => checkedAcknowledgedOwnerFaceArguments({ ...acknowledged, ...state }, acknowledged,
      action.identity, action.target, supplied), /acknowledged owner Show/);
  }
  const forged = structuredClone(acknowledged);
  forged.actions[0].arguments[1].choices.push('750');
  assert.throws(() => checkedAcknowledgedOwnerFaceArguments(acknowledged, forged,
    action.identity, action.target, [supplied[0], { name: 'clock/interval-ms', value: '750' }]), /supported browser text form/);
});

test('Face text admission rejects unpaired surrogates without changing valid Unicode', () => {
  const invalid = ['\ud800', '\udfff', '\udfff\ud800'];
  const accepted = invalid.map(value => {
    try { submit([{ name: 'body/name', value }, { name: 'clock/interval-ms', value: '250' }]); return true; }
    catch { return false; }
  });
  const malformedChoices = { ...action, arguments: [{ name: 'value', value_kind: 'value/text', maximum_bytes: 8, choices: ['yes', '\ud800'] }] };
  try { checkedOwnerFaceArguments({ actions: [malformedChoices] }, action.identity, action.target, [{ name: 'value', value: 'yes' }]); accepted.push(true); }
  catch { accepted.push(false); }
  assert.deepEqual(accepted, [false, false, false, false]);
  assert.deepEqual(submit([{ name: 'body/name', value: 'é 😀' }, { name: 'clock/interval-ms', value: '250' }])[0],
    { name: 'body/name', value: 'é 😀' });
});
