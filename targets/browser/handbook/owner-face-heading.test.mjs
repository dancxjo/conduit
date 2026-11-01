import assert from 'node:assert/strict';
import { test } from 'node:test';
import { presentedBodySubject } from './owner-face-component.mjs';

test('one presented Body supplies exact wording and identity without mutation', () => {
  for (const disclosure of [undefined, 'Primary', 'Context']) {
    const body = Object.freeze({ identity: 'body/exact', role: 'Body',
      name: '  Malo Lefèvre Laurent 🪴  ', disclosure });
    const subjects = Object.freeze([body,
      Object.freeze({ identity: 'host/exact', role: 'Host', name: body.name })]);
    assert.equal(presentedBodySubject(subjects), body);
  }
});

test('private, blank, absent and ambiguous Body names never become a heading', () => {
  const body = { identity: 'body/one', role: 'Body', name: 'Body name' };
  for (const disclosure of ['CurrentAction', 'SelectedDetail', 'ExactProvenance', 'future-level', null]) {
    assert.equal(presentedBodySubject([{ ...body, disclosure }]), undefined);
  }
  for (const name of ['', ' \n\t', undefined, null, 1]) {
    assert.equal(presentedBodySubject([{ ...body, name }]), undefined);
  }
  assert.equal(presentedBodySubject([]), undefined);
  assert.equal(presentedBodySubject([{ ...body, role: 'Host' }]), undefined);
  assert.equal(presentedBodySubject([body, { ...body, identity: 'body/two',
    disclosure: 'ExactProvenance' }]), undefined);
});
