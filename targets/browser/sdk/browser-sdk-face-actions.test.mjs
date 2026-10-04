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
