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
  const request = { schema: 'conduit.presentation/owner-face-request@1', body_id: 'body/clock' };
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
