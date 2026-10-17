import test from 'node:test';
import assert from 'node:assert/strict';
import { projectConduitSyntax } from '../../targets/browser/host/assets/application-syntax-face.mjs';
import { syntaxProjectionFixture } from './syntax-projection-fixture.mjs';

test('syntax adapter preserves leading BOM and exact scalar source bytes', () => {
  const { runtime, calls } = syntaxProjectionFixture();
  const source = '\ufeffplot café { # 😀';
  const result = projectConduitSyntax(source, runtime);
  assert.equal(result.source_bytes, Buffer.byteLength(source));
  assert.equal(new TextDecoder('utf-8', { ignoreBOM: true }).decode(new Uint8Array(runtime.memory.buffer, 0, result.source_bytes)), source);
  assert.equal(calls(), 1);
});

test('syntax adapter refuses overcapacity strings before UTF-8 allocation', () => {
  const { runtime, calls } = syntaxProjectionFixture();
  const original = TextEncoder.prototype.encode;
  TextEncoder.prototype.encode = function(value) {
    if (value.length > 8192) throw new Error('oversized encoding attempted');
    return original.call(this, value);
  };
  try { assert.throws(() => projectConduitSyntax('x'.repeat(8193), runtime), RangeError); }
  finally { TextEncoder.prototype.encode = original; }
  assert.equal(calls(), 0);
});

test('syntax adapter retains exact byte ceiling and refuses non-scalar or multibyte overflow', () => {
  const fixture = syntaxProjectionFixture();
  assert.equal(projectConduitSyntax('x'.repeat(8192), fixture.runtime).source_bytes, 8192);
  assert.throws(() => projectConduitSyntax('é'.repeat(4097), fixture.runtime), RangeError);
  assert.throws(() => projectConduitSyntax('\ud800', fixture.runtime), TypeError);
  assert.equal(fixture.calls(), 1);
});
