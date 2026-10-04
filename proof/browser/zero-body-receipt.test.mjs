import assert from 'node:assert/strict';
import { test } from 'node:test';
import { makeZeroBodyReceipt } from './zero-body-receipt.mjs';

test('pre-Birth receipt binds the installed Host and live zero-Body status', () => {
  const installation = { host_id: 'host/fresh', body_state: null, joined_body_state: null };
  const status = { host_id: 'host/fresh', body_id: null, presence: 'installed-live' };
  const receipt = makeZeroBodyReceipt(installation, status, false, false);
  assert.equal(receipt.live_service_status, status);
  assert.throws(() => makeZeroBodyReceipt(installation, { ...status, body_id: 'body/old' }, false, false));
  assert.throws(() => makeZeroBodyReceipt(installation, { ...status, host_id: 'host/other' }, false, false));
  assert.throws(() => makeZeroBodyReceipt(installation, status, true, false));
  assert.throws(() => makeZeroBodyReceipt(installation, status, false, true));
});
