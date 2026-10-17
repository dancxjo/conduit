import assert from 'node:assert/strict';
import { test } from 'node:test';
import { captureRunId } from './three-host-run-identity.mjs';

test('run identity binds the actual capture Host and Boot as well as Body', () => {
  const first = captureRunId('body/clock', 'host/one', 'boot/one');
  assert.equal(first, captureRunId('body/clock', 'host/one', 'boot/one'));
  assert.notEqual(first, captureRunId('body/clock', 'host/two', 'boot/one'));
  assert.notEqual(first, captureRunId('body/clock', 'host/one', 'boot/two'));
  assert.notEqual(first, captureRunId('body/other', 'host/one', 'boot/one'));
  assert.throws(() => captureRunId('body/clock', '', 'boot/one'));
});
