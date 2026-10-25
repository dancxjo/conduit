import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { checkEvidenceEntries, ROOTS } from '../../tools/ci/pipeline/evidence-retention.mjs';
const path = 'docs/audits/evidence/run/raw.log';
const bytes = Buffer.from('historical capture');
const entry = { path, bytes };
function policy() {
  return { budgets: Object.fromEntries(ROOTS.map(root => [root, 100000])), retained: {
    [path]: { bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex'),
      consumer: 'canonical regression fixture' },
  } };
}
test('small receipts need no raw-output exception', () => {
  const p = policy(); p.retained = {};
  checkEvidenceEntries([{ path: 'docs/audits/evidence/result.json', bytes }], p);
});
test('exact retained fixture passes; new raw output refuses', () => {
  checkEvidenceEntries([entry], policy());
  assert.throws(() => checkEvidenceEntries([entry, { ...entry, path: `${path}.gz` }], policy()), /named consumer/);
});
test('changed bytes, absent consumer, stale entry and exceeded budget refuse', () => {
  assert.throws(() => checkEvidenceEntries([{ ...entry, bytes: Buffer.from('changed') }], policy()), /named consumer/);
  const p = policy(); p.retained[path].consumer = ' ';
  assert.throws(() => checkEvidenceEntries([entry], p), /named consumer/);
  assert.throws(() => checkEvidenceEntries([], policy()), /stale/);
  const bounded = policy(); bounded.budgets[ROOTS[0]] = 1;
  assert.throws(() => checkEvidenceEntries([entry], bounded), /budget/);
});
test('large JSON is raw output despite its suffix', () => {
  assert.throws(() => checkEvidenceEntries([{ path: 'site/evidence/capture.json', bytes: Buffer.alloc(65537) }], policy()), /named consumer/);
});

test('missing or nonfinite budgets refuse rather than disabling the bound', () => {
  const p = policy(); delete p.budgets[ROOTS[0]];
  assert.throws(() => checkEvidenceEntries([entry], p), /finite evidence budget/);
});
