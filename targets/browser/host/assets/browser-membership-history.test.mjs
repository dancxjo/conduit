import assert from 'node:assert/strict';
import test from 'node:test';
import {checkedHistoryMaskRequestFrame,checkedHistoryMaskResponse} from './browser-membership.js';
const enc = new TextEncoder();
const pending = {requestId:'browser-history:1'};
const request = {schema:'conduit.face/owner-face-request@1',body_id:'body/fixture'};
const showBytes = enc.encode('{"revision":18446744073709551615,"face_id":"fixture"}');
const lookBytes = Uint8Array.of(1,2,255);
const frame = extra => checkedHistoryMaskRequestFrame({requestId:pending.requestId,request,showBytes,lookBytes,...extra});
test('history request preserves original Show integers and detached canonical intent bytes', () => {
  const bytes = frame(); lookBytes[0] = 0;
  const text = new TextDecoder().decode(bytes);
  assert.match(text,/"revision":18446744073709551615/);
  assert.deepEqual(JSON.parse(text).look,[1,2,255]); lookBytes[0] = 1;
  assert.equal(JSON.parse(text).kind,'face-history-mask-request');
});
test('history transport bounds canonical bytes, carrier pressure and exact sequence syntax', () => {
  for (const lookBytes of [[],new Uint8Array(),new Uint8Array(65537)]) assert.throws(() => frame({lookBytes}));
  // JSON decimal byte arrays have a separate carrier limit from canonical bytes.
  assert.throws(() => frame({lookBytes:new Uint8Array(65536).fill(255)}),/carrier bound/);
  for (const requestId of ['browser-history:0','browser-history:01','browser-history:10000000000000000','browser-component:1']) assert.throws(() => frame({requestId}));
  assert.doesNotThrow(() => frame({requestId:'browser-history:9999999999999999'}));
  assert.throws(() => frame({showBytes:enc.encode('not-json')}));
});
test('history response correlates bounded success or explicit owner refusal', () => {
  const valid = {kind:'face-history-mask-response',protocol:1,request_id:pending.requestId,accepted:true,refusal:''};
  assert.doesNotThrow(() => checkedHistoryMaskResponse(valid,pending,512));
  for (const patch of [{request_id:'browser-history:2'},{accepted:true,refusal:'refused'},{accepted:false},{refusal:null},{protocol:2}]) assert.throws(() => checkedHistoryMaskResponse({...valid,...patch},pending,512));
  assert.throws(() => checkedHistoryMaskResponse(valid,pending,4097));
  assert.throws(() => checkedHistoryMaskResponse({...valid,accepted:false,refusal:'stale-face-or-show'},pending,512),error => error.code === 'stale-face-or-show');
});
