import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";
import { parseNativeStartup, recordNativeStartup } from "./native-startup-trace.mjs";

const phases = ["service-lock", "body-lock", "retained-recovery", "ownership-selection", "retained-scan",
  "installation", "boot-identity", "host-construction", "equipment-selection", "runtime-identity",
  "image-digest", "runtime-state", "catalogs", "birth-encounter", "runtime-publication"];
function rows() {
  const result = [];
  const add = (event, phase = null, duration_us = null) => result.push({
    schema: "conduit.proof/host-startup@1", pid: 42, sequence: result.length,
    event, phase, elapsed_us: 10 + result.length * 10, duration_us,
  });
  add("start");
  for (const phase of phases) { add("begin", phase); add("end", phase, 5); }
  add("prepared"); return result;
}
const encode = values => Buffer.from(values.map(row => `CONDUIT_HOST_STARTUP_PHASE ${JSON.stringify(row)}\n`).join(""));

test("complete native preparation retains source phases, PID and timings without endpoint readiness", () => {
  const packet = parseNativeStartup(encode(rows()), 42);
  assert.equal(packet.outcome, "prepared");
  assert.equal(packet.records.length, 32);
  assert.equal(packet.active_phase, null);
  assert.equal(packet.records.find(row => row.phase === "catalogs" && row.event === "end").duration_us, 5);
  assert.ok(encode(rows()).length < 16 * 1024);
  assert.equal(parseNativeStartup(Buffer.from("durable host is running\n"), 42).outcome, "absent");
});

test("refusal and an interrupted complete-line prefix identify the actual open phase", () => {
  const prefix = rows().slice(0, 2);
  const incomplete = parseNativeStartup(encode(prefix), 42);
  assert.equal(incomplete.outcome, "incomplete");
  assert.equal(incomplete.active_phase, "service-lock");
  prefix.push({ ...rows()[2], event: "refused" });
  prefix.push({ ...rows()[3], phase: null, event: "aborted", duration_us: null });
  const refused = parseNativeStartup(encode(prefix), 42);
  assert.equal(refused.outcome, "aborted");
  assert.equal(refused.failed_phase, "service-lock");
});

test("wrong process, missing phases, repeated sequence and backwards clocks cannot certify a trace", () => {
  for (const mutate of [
    value => { value[1].pid = 43; },
    value => { value[1].sequence = 0; },
    value => { value[2].elapsed_us = 0; },
    value => { value[2].duration_us = 1000; },
    value => { value[1].phase = "catalogs"; },
    value => { value[1].phase = "private/unreviewed"; },
  ]) {
    const value = rows(); mutate(value);
    assert.equal(parseNativeStartup(encode(value), 42).outcome, "refused");
  }
  assert.equal(parseNativeStartup(encode([rows()[0], { ...rows().at(-1), sequence: 1 }]), 42).outcome, "refused");
  assert.equal(parseNativeStartup(encode(rows()), null).outcome, "refused");
});

test("byte, line and schema refusals retain only a finite validated prefix", () => {
  const value = rows(); value[1].secret = "private-value";
  const bad = parseNativeStartup(encode(value), 42);
  assert.equal(bad.outcome, "refused");
  assert.ok(!JSON.stringify(bad).includes("private-value"));
  value[1].secret = "x".repeat(600);
  assert.equal(parseNativeStartup(encode(value), 42).error, "phase-line-bound");
  const oversized = parseNativeStartup(Buffer.concat([encode(rows()), Buffer.alloc(20_000, 120)]), 42);
  assert.equal(oversized.outcome, "incomplete");
  assert.equal(oversized.output_truncated, true);
  assert.ok(oversized.records.length <= 40);
  const cut = encode(rows()).subarray(0, -1);
  assert.equal(parseNativeStartup(cut, 42).incomplete_line, true);
});

test("separate stderr capture survives readiness arriving before the final diagnostic chunk", () => {
  const child = { pid: 42, stderr: new EventEmitter(), stdout: new EventEmitter() };
  const finish = recordNativeStartup(child), bytes = encode(rows());
  child.stderr.emit("data", bytes.subarray(0, 13));
  child.stdout.emit("data", Buffer.from("durable host is running\n"));
  child.stderr.emit("data", bytes.subarray(13));
  const record = finish();
  assert.equal(record.outcome, "prepared");
  assert.strictEqual(finish(), record);
  assert.equal(child.stderr.listenerCount("data"), 0);
});
