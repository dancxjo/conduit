import assert from 'node:assert/strict';
import test from 'node:test';
import { parseCaptureJSON, captureDecimal } from '../../proof/browser/exact-capture-json.mjs';

test('retained native u64 revisions and values round trip without JavaScript rounding', () => {
  const source = '{"revision":18446744073709551615,"nested":[9007199254740993,21.5,true,null,"literal"]}';
  const value = parseCaptureJSON(source);
  assert.equal(captureDecimal(value.revision), '18446744073709551615');
  assert.equal(captureDecimal(value.nested[0]), '9007199254740993');
  assert.equal(JSON.stringify(value), source);
  assert.equal(value.nested[1], 21.5);
});

test('revision comparisons refuse rounded numbers and unsuitable decimal claims', () => {
  assert.throws(() => captureDecimal(Number('18446744073709551615')));
  for (const value of [-1, 1.5, '1e20', 'not-a-revision', null]) assert.throws(() => captureDecimal(value));
  assert.equal(captureDecimal(0), '0');
  assert.equal(captureDecimal('9007199254740993'), '9007199254740993');
});
