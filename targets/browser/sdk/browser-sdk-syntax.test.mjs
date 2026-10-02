import test from 'node:test';
import assert from 'node:assert/strict';
import { projectBrowserSyntax } from './browser-sdk-syntax.mjs';
const kinds = ['whitespace', 'comment', 'keyword', 'name', 'identity', 'string', 'number', 'literal', 'operator', 'delimiter'];
function fixture(change = value => value, status = 0) {
  const memory = { buffer: new ArrayBuffer(256 * 1024) };
  let outputLength = 0, calls = 0;
  return { memory, get calls() { return calls; }, conduit_syntax_input_capacity: () => 8192,
    conduit_syntax_input_ptr: () => 0, conduit_syntax_output_ptr: () => 8192,
    conduit_syntax_output_len: () => outputLength,
    conduit_syntax_project(length) {
      calls++;
      const projection = change({ protocol: 'conduit.syntax-highlight-projection@1', source_bytes: length, kinds: [...kinds], spans: length ? [[0, length, 3]] : [] });
      const output = new TextEncoder().encode(JSON.stringify(projection));
      new Uint8Array(memory.buffer, 8192, output.length).set(output);
      outputLength = output.length;
      return status;
    } };
}
test('adapter forwards incomplete and Unicode source unchanged to native ABI', () => {
  const runtime = fixture();
  for (const source of ['', 'plot demo {', 'plot café { "🎵']) {
    const result = projectBrowserSyntax(source, runtime);
    const bytes = new TextEncoder().encode(source);
    assert.deepEqual(new Uint8Array(runtime.memory.buffer, 0, bytes.length), bytes);
    assert.equal(result.source_bytes, bytes.length);
    assert.ok(Object.isFrozen(result.spans));
  }
  assert.equal(runtime.calls, 3);
});
test('byte overflow and malformed Unicode refuse before invoking native ABI', () => {
  const runtime = fixture();
  assert.throws(() => projectBrowserSyntax('🎵'.repeat(2049), runtime), /byte bound/);
  assert.throws(() => projectBrowserSyntax('\ud800', runtime), /Unicode/);
  assert.equal(runtime.calls, 0);
});
for (const [name, mutate] of [
  ['wrong protocol', p => ({ ...p, protocol: 'other' })],
  ['wrong length', p => ({ ...p, source_bytes: 1 })],
  ['unknown kind', p => ({ ...p, kinds: ['invented', ...p.kinds.slice(1)] })],
  ['gap', p => ({ ...p, spans: [[1, p.source_bytes, 0]] })],
  ['split UTF-8', p => ({ ...p, spans: [[0, 1, 0], [1, p.source_bytes, 0]] })],
]) test(`refuses malformed native projection: ${name}`, () => {
  assert.throws(() => projectBrowserSyntax('é', fixture(mutate)));
});
test('native refusal remains a refusal', () => {
  assert.throws(() => projectBrowserSyntax('plot', fixture(v => v, -409)), /refused/);
});
