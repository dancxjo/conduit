import test from "node:test";
import assert from "node:assert/strict";
import { createBodyInputRouting } from "../../targets/browser/host/assets/browser-body-input.mjs";

function setup() {
  let selected = "notes", waiting;
  const forms = ["notes", "morse"].map(form => ({ form: { checked_form_id: form }, plan: { fragments: [{ placements: [{ placement_id: `${form}/keys`, kind_id: "input/keyboard" }] }] } }));
  const routing = createBodyInputRouting({ forms, foreground: () => selected, maximumPlacements: 32 });
  routing.attach({ nextKeyboard: () => new Promise((resolve, reject) => { waiting = { resolve, reject }; }) });
  const next = (form, signal = new AbortController().signal) => routing.next("keyboard", `${form}/keys`, signal);
  const capture = (usage, phase = 0) => {
    const canonical_bytes = Uint8Array.of(usage, phase, 0);
    return { canonical_bytes, delivery_form: routing.capture("keyboard", canonical_bytes) };
  };
  const send = async event => {
    assert.ok(waiting, "one acquired reader is armed");
    const pending = waiting; waiting = null; pending.resolve(event);
    await Promise.resolve();
  };
  const close = () => { routing.close(); waiting?.reject(new Error("closed")); };
  return { routing, next, capture, send, close, select(form) { selected = form; } };
}

test("foreground changes route future keys while preserving background pending requests", async () => {
  const f = setup();
  let notesDone = false;
  const notes = f.next("notes").then(value => { notesDone = true; return value; });
  const morse = f.next("morse");
  f.select("morse");
  await f.send(f.capture(4));
  assert.equal((await morse).delivery_form, "morse");
  assert.equal(notesDone, false);
  f.select("notes");
  await f.send(f.capture(5));
  assert.equal((await notes).delivery_form, "notes");
  f.close();
});

test("queued keys retain capture focus and held-key releases stay with their press", async () => {
  const f = setup();
  const first = f.next("notes");
  const down = f.capture(4);
  f.select("morse");
  await f.send(down);
  assert.equal((await first).delivery_form, "notes");
  await f.send(f.capture(4, 1));
  assert.equal((await f.next("notes")).canonical_bytes[1], 1);
  const next = f.next("morse");
  await f.send(f.capture(4));
  assert.equal((await next).delivery_form, "morse");
  f.close();
});

test("cancelling one input request does not cancel another Form's request", async () => {
  const f = setup();
  const controller = new AbortController();
  const cancelled = assert.rejects(f.next("notes", controller.signal), { code: "Cancelled" });
  const morse = f.next("morse");
  controller.abort();
  await cancelled;
  f.select("morse");
  await f.send(f.capture(4));
  assert.equal((await morse).delivery_form, "morse");
  f.close();
});

test("uninstalled identities and duplicate requests refuse without consuming input", async () => {
  const f = setup();
  const first = f.next("notes");
  await assert.rejects(f.next("notes"), /already has a pending/);
  await assert.rejects(f.next("unknown"), /outside the admitted/);
  f.select("unknown");
  assert.throws(() => f.capture(4), /outside the admitted/);
  f.select("notes");
  await f.send(f.capture(4));
  assert.equal((await first).delivery_form, "notes");
  f.close();
});

test("ordered pressure terminates only the affected Form stream", async () => {
  const f = setup();
  const notes = f.next("notes");
  f.select("morse");
  for (let usage = 4; usage < 13; usage++) await f.send(f.capture(usage));
  await assert.rejects(f.next("morse"), { code: "Pressure" });
  f.select("notes");
  await f.send(f.capture(20));
  assert.equal((await notes).canonical_bytes[0], 20);
  assert.deepEqual(f.routing.pressure().map(({ form, occupancy, terminal }) => ({ form, occupancy, terminal })), [
    { form: "morse", occupancy: 0, terminal: "Pressure" },
    { form: "notes", occupancy: 0, terminal: null },
  ]);
  f.close();
});

test('pointer delivery follows its captured Form and unrelated input cannot fill its queue', async () => {
  let selected = 'pointer', consume, stopped = false;
  const forms = ['pointer', 'notes'].map(form => ({ form: { checked_form_id: form }, plan: { fragments: [{ placements: [{ placement_id: `${form}/input`, kind_id: form === 'pointer' ? 'input/pointer-source' : 'input/keyboard' }] }] } }));
  const routing = createBodyInputRouting({ forms, foreground: () => selected, maximumPlacements: 32 });
  routing.attach({ observePointer(listener) { consume = listener; return () => { stopped = true; }; } });
  const signal = new AbortController().signal;
  const waiting = routing.next('pointer', 'pointer/input', signal);
  const event = { position_x: 250000, delivery_form: routing.capture('pointer', null) };
  selected = 'notes';
  for (let i = 0; i < 100; i++) assert.equal(routing.capture('pointer', null), null);
  assert.equal(routing.capture('button', { pressed: true }), null);
  consume(event);
  assert.equal((await waiting).position_x, 250000);
  await assert.rejects(routing.next('pointer', 'notes/input', signal), { code: 'StalePlacement' });
  routing.close();
  assert.equal(stopped, true);
});

test("pointer pressure coalesces 100,000 observations through one reusable slot", async () => {
  let consume;
  const forms = [{ form: { checked_form_id: "theremin" }, plan: { fragments: [{ placements: [{ placement_id: "theremin/pointer", kind_id: "input/pointer-source" }] }] } }];
  const routing = createBodyInputRouting({ forms, foreground: () => "theremin", maximumPlacements: 1 });
  routing.attach({ observePointer(listener) { consume = listener; return () => {}; } });
  const signal = new AbortController().signal;
  const first = routing.next("pointer", "theremin/pointer", signal);
  for (let sequence = 0; sequence < 100_000; sequence += 1) {
    consume(Object.freeze({
      schema: "input/pointer-event@1",
      delivery_form: routing.capture("pointer", null),
      position_x: sequence,
      position_y: sequence * 2,
      delta_x: 1,
      delta_y: -1,
      primary_pressed: sequence % 2 === 0,
      coalesced: 0,
      dropped: 0,
      queue_capacity: 1,
      sequence,
    }));
  }
  assert.equal((await first).sequence, 0);
  const latest = await routing.next("pointer", "theremin/pointer", signal);
  assert.equal(latest.sequence, 99_999);
  assert.equal(latest.position_x, 99_999);
  assert.equal(latest.primary_pressed, false);
  assert.equal(latest.delta_x, 99_999);
  assert.equal(latest.delta_y, -99_999);
  assert.equal(latest.coalesced, 99_998);
  assert.deepEqual(routing.pressure(), [{
    kind: "pointer", form: "theremin", capacity: 1, occupancy: 0,
    accepted: 100_000, delivered: 2, coalesced: 99_998, dropped: 0,
    refusals: 0, terminal: null,
  }]);
  routing.close();
});

test("ordered storage is reusable across repeated queue wraparound", async () => {
  const f = setup();
  for (let sequence = 0; sequence < 1_024; sequence += 1) {
    const pending = f.next("notes");
    await f.send(f.capture(4 + (sequence % 20), sequence % 2));
    const value = await pending;
    assert.equal(value.canonical_bytes[0], 4 + (sequence % 20));
    assert.equal(value.canonical_bytes[1], sequence % 2);
  }
  assert.deepEqual(f.routing.pressure(), [{
    kind: "keyboard", form: "notes", capacity: 8, occupancy: 0,
    accepted: 1_024, delivered: 1_024, coalesced: 0, dropped: 0,
    refusals: 0, terminal: null,
  }]);
  f.close();
});
