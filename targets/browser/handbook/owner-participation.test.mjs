import test from 'node:test';
import assert from 'node:assert/strict';
import { checkedExpectedBodyId, checkedLoopbackOwnerWindow } from './owner-participation.mjs';

test('Handbook owner window accepts only the local admitted carrier', () => {
  assert.equal(checkedLoopbackOwnerWindow('ws://127.0.0.1:41000/conduit'), 'ws://127.0.0.1:41000/conduit');
  for (const url of [
    'ws://localhost:41000/conduit', 'ws://192.168.1.2:41000/conduit',
    'wss://127.0.0.1:41000/conduit', 'ws://127.0.0.1:41000/other',
    'ws://127.0.0.1:41000/?token=secret', 'ws://user@127.0.0.1:41000/',
  ]) assert.throws(() => checkedLoopbackOwnerWindow(url), /exact ws:\/\/127\.0\.0\.1/);
});

test('Handbook requires an exact bounded owner Body identity', () => {
  assert.equal(checkedExpectedBodyId(' body/owner '), 'body/owner');
  assert.throws(() => checkedExpectedBodyId(''), /exact Body ID/);
  assert.throws(() => checkedExpectedBodyId('x'.repeat(129)), /exact Body ID/);
});
