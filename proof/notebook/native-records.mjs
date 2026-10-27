// A growing serial file may end partway through a record. Only its completed
// newline-delimited frames are observations; malformed completed frames refuse.
import assert from 'node:assert/strict';
import { parseCaptureJSON } from '../browser/exact-capture-json.mjs';

export function nativeRecords(text, prefix) {
  assert.ok(Buffer.byteLength(text) <= 2 * 1024 * 1024, 'finite native serial');
  return text.split('\n').slice(0, -1)
    .filter(line => line.startsWith(`${prefix} `))
    .map(line => parseCaptureJSON(line.slice(prefix.length + 1)));
}
