import assert from 'node:assert/strict';
import test from 'node:test';
import { captureDecimal } from '../browser/exact-capture-json.mjs';
import { nativeRecords } from './native-records.mjs';

const prefix = 'CONDUIT_NATIVE_OWNER_ACTION';
test('a partial acceptance tail remains pending without dropping completed records', () => {
  const complete = `${prefix} {"status":"accepted","revision":6752589735566018214}\n`;
  const tail = `${prefix} {"status":"accepted","prior_show_id":"show/`;
  const records = nativeRecords(`unrelated boot observation\n${complete}${tail}`, prefix);
  assert.equal(records.length, 1);
  assert.equal(records[0].status, 'accepted');
  assert.equal(captureDecimal(records[0].revision), '6752589735566018214');
  assert.equal(nativeRecords(tail, prefix).length, 0);
  assert.equal(nativeRecords(`${tail}next"}\n`, prefix).length, 1);
});
test('a malformed completed frame and excessive serial refuse', () => {
  assert.throws(() => nativeRecords(`${prefix} {"status":\n`, prefix), SyntaxError);
  assert.throws(() => nativeRecords('x'.repeat(2 * 1024 * 1024 + 1), prefix), /finite native serial/);
});
