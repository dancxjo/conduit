import assert from 'node:assert/strict';
import { test } from 'node:test';
import { assertExactFaceRevision } from './three-host-llm-chapter.mjs';

test('browser decimal Face revision matches exact numeric receipt', () => {
  assert.doesNotThrow(() => assertExactFaceRevision('13', 13));
  assert.throws(() => assertExactFaceRevision('13', 14));
  assert.throws(() => assertExactFaceRevision('013', 13));
  assert.throws(() => assertExactFaceRevision(13, 13));
});

test('unsafe numeric Face revision refuses instead of rounding', () => {
  assert.throws(() => assertExactFaceRevision('9007199254740992', 9007199254740992),
    /exact safe integer/);
});
