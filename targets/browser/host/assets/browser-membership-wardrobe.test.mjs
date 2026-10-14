import assert from 'node:assert/strict';
import test from 'node:test';
import { checkedOwnerWardrobeResponse, checkedOwnerWardrobeRequestFrame } from './browser-membership.js';

const pending = { requestId: 'browser-wardrobe:1' };
const response = { kind: 'face-wardrobe-response', protocol: 1, request_id: pending.requestId,
  accepted: true, code: '', report: { schema: 'conduit.body/owner-mask-wardrobe@1' } };

test('wardrobe response binds to one request and bounded owner report', () => {
  assert.equal(checkedOwnerWardrobeResponse(response, pending, 512), response.report);
  assert.throws(() => checkedOwnerWardrobeResponse({ ...response, request_id: 'browser-wardrobe:2' }, pending, 512));
  assert.throws(() => checkedOwnerWardrobeResponse(response, pending, 64 * 1024 + 1025));
  assert.throws(() => checkedOwnerWardrobeResponse({ ...response, report: null }, pending, 512));
});

test('wardrobe frame preserves exact u64 revision and one controlled numeric slot', () => {
  const request = { schema: 'conduit.face/owner-face-request@1', body_id: 'body/clock' };
  const bytes = checkedOwnerWardrobeRequestFrame({ requestId: pending.requestId, request,
    ownerPlanId: 'plan/owner', basisRevision: '18446744073709551615', action: { Wear: {
      source_document_id: 'source', checked_plot_id: 'checked', expanded_plot_id: 'expanded',
    } } });
  const text = new TextDecoder().decode(bytes);
  assert.match(text, /"basis_revision":18446744073709551615/);
  assert.doesNotMatch(text, /"basis_revision":"18446744073709551615"/);
  assert.equal((text.match(/"basis_revision":/g) ?? []).length, 1);
  assert.throws(() => checkedOwnerWardrobeRequestFrame({ requestId: pending.requestId,
    request: { basis_revision: null }, ownerPlanId: null, basisRevision: '0', action: null }), /unique revision slot/);
  assert.throws(() => checkedOwnerWardrobeRequestFrame({ requestId: pending.requestId,
    request, ownerPlanId: null, basisRevision: '18446744073709551616', action: null }), /exact u64/);
});

test('owner refusal remains a refusal with its machine-readable code', () => {
  assert.throws(() => checkedOwnerWardrobeResponse({ ...response, accepted: false,
    code: 'owner-wardrobe-unavailable', report: null }, pending, 512),
  error => error.code === 'owner-wardrobe-unavailable');
});


test('component selection keeps exact Show integers and refuses open Look fields', async () => {
  const { checkedComponentMaskRequestFrame } = await import('./browser-membership.js');
  const input = { requestId: 'browser-component:1', request: { body_id: 'body' },
    showBytes: new TextEncoder().encode('{"revision":18446744073709551615}'),
    look: 'compact', recipe: 'flat' };
  const text = new TextDecoder().decode(checkedComponentMaskRequestFrame(input));
  assert.match(text, /18446744073709551615/);
  assert.match(text, /"kind":"face-component-mask-request"/);
  for (const mutation of [{ look: 'arbitrary-css' }, { recipe: 'change-actions' },
    { requestId: 'unbounded' }, { showBytes: new Uint8Array(65537) }]) {
    assert.throws(() => checkedComponentMaskRequestFrame({ ...input, ...mutation }));
  }
});


test('component response correlates one bounded request and truthful refusal', async () => {
  const { checkedComponentMaskResponse } = await import('./browser-membership.js');
  const pending = { requestId: 'browser-component:1' };
  const frame = { kind: 'face-component-mask-response', protocol: 1,
    request_id: pending.requestId, accepted: true, code: '' };
  checkedComponentMaskResponse(frame, pending, 128);
  for (const change of [{ request_id: 'browser-component:2' }, { accepted: 'true' },
    { accepted: false }, { code: 'refused' }, { protocol: 2 }]) {
    assert.throws(() => checkedComponentMaskResponse({ ...frame, ...change }, pending, 128));
  }
  assert.throws(() => checkedComponentMaskResponse(frame, pending, 4097));
  assert.throws(() => checkedComponentMaskResponse({ ...frame, accepted: false, code: 'stale-show' },
    pending, 128), error => error.code === 'stale-show');
});


test('component request sequence is canonical and bounded at both limits', async () => {
  const { checkedComponentMaskRequestFrame } = await import('./browser-membership.js');
  const input = { request: { body_id: 'body' },
    showBytes: new TextEncoder().encode('{"show":{}}'), look: 'spacious', recipe: 'grouped' };
  for (const requestId of ['browser-component:1', 'browser-component:9999999999999999']) {
    assert.ok(checkedComponentMaskRequestFrame({ ...input, requestId }).length > 0);
  }
  for (const requestId of ['browser-component:0', 'browser-component:01',
    'browser-component:', 'browser-component:10000000000000000',
    'browser-component:1e2', 'browser-component:+1', 'browser-component:1\n']) {
    assert.throws(() => checkedComponentMaskRequestFrame({ ...input, requestId }));
  }
});
