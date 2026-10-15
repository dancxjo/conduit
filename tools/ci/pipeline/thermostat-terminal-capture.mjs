// Verify the installed peer's retained foreground receipts, not the old owner-local flag.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { parseCaptureJSON } from '../../../proof/browser/exact-capture-json.mjs';
const digest = bytes => [...createHash('sha256').update(bytes).digest()];
export function validateTerminalCapture(root, receipt) {
  const bytes = readFileSync(path.join(root, 'capture/terminal.txt'));
  assert.ok(bytes.length > 0 && bytes.length <= 1024 * 1024, 'terminal transcript exceeds bound');
  const prefix = Buffer.from('Peer terminal evidence ');
  const frameStart = Buffer.from('\x1b[2J\x1b[H');
  const reports = [];
  let offset = 0;
  while ((offset = bytes.indexOf(prefix, offset)) >= 0) {
    const end = bytes.indexOf(10, offset);
    assert.ok(end >= 0 && reports.length < 3, 'terminal evidence is incomplete or duplicated');
    const report = parseCaptureJSON(bytes.subarray(offset + prefix.length, end).toString('utf8'));
    assert.equal(report.schema, 'conduit.body/terminal-show-evidence@1');
    assert.equal(report.body_id, receipt.body_id);
    assert.equal(report.host_id, receipt.presentation_peer_host_id);
    assert.equal(report.boot_id, receipt.journey.speech.evidence.execution.source_show.show.boot_id);
    assert.equal(report.show_state, 'available');
    for (const key of ['show_id', 'route_plan_id', 'mask_plan_id', 'mask_play_id']) {
      assert.ok(typeof report[key] === 'string' && report[key].length > 0 && report[key].length <= 256, `terminal ${key} is absent`);
    }
    assert.deepEqual(report.show_sha256, digest(Buffer.from(report.show_id)), 'terminal Show digest differs');
    assert.ok(Number.isSafeInteger(report.bytes_written) && report.bytes_written > 0 && report.bytes_written <= 16384, 'terminal frame extent differs');
    const start = bytes.lastIndexOf(frameStart, offset);
    assert.ok(start >= 0 && start + report.bytes_written < offset, 'terminal frame is absent');
    assert.deepEqual(report.frame_sha256, digest(bytes.subarray(start, start + report.bytes_written)), 'terminal frame digest differs');
    const target = ['21', '22.5', '18'][reports.length];
    assert.ok(bytes.subarray(start, start + report.bytes_written).includes(Buffer.from(`Target temperature: ${target} °C`)), 'terminal typed target readback is absent');
    // The peer emits Ready only after its foreground write/flush acknowledgement
    // and the owner's accepted Show acknowledgement have both returned.
    assert.ok(bytes.subarray(start + report.bytes_written, offset).includes(Buffer.from('\nTerminal ready.')), 'terminal owner acknowledgement is absent');
    reports.push(report);
    offset = end + 1;
  }
  assert.equal(reports.length, 3, 'terminal Shows are absent');
  assert.deepEqual(reports, receipt.journey.terminal_evidence, 'terminal receipts differ from retained transcript');
  return bytes.toString('utf8');
}
