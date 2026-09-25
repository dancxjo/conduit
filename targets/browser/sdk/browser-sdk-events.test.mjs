import test from "node:test";
import assert from "node:assert/strict";
import { createBodyEventStream, projectBodyEvidenceEvents, projectBodySnapshot, projectHostOffersChange } from "./browser-sdk-events.mjs";

const snapshot = (records = [], extra = {}) => ({
  schema: "conduit.workspace/body@1",
  evidence: {
    schema: "conduit.body/biography-evidence@2", body_id: "body/1",
    body: { workload_revision: 3 }, membership: { revision: 2 }, records,
    wakes: [], compaction: null,
  },
  current_host_offers: [], ...extra,
});

test("snapshot projection is the immutable exact Workspace response", () => {
  const raw = snapshot([{ sequence: 1, sign_id: "sign/1", kind: { Born: { workload_revision: 0 } } }]);
  const projected = projectBodySnapshot(raw);
  assert.deepEqual(projected, raw);
  assert(Object.isFrozen(projected));
  assert(Object.isFrozen(projected.evidence.records[0].kind.Born));
});

test("lifecycle events correlate the exact retained biography record and sign", () => {
  const evidence = snapshot([
    { sequence: 1, sign_id: "sign/1", kind: { Born: { workload_revision: 0 } } },
    { sequence: 2, sign_id: "sign/2", kind: { FormAdmitted: { checked_form_id: "form/1", workload_revision: 1 } } },
    { sequence: 3, sign_id: "sign/3", kind: { WakeEvent: { wake_id: "wake/1", event_index: 0 } } },
  ]);
  evidence.evidence.wakes.push({ wake_id: "wake/1", body_id: "body/1", wake_sequence: 1, events: [{ Woke: { sign_id: "sign/3" } }] });
  const events = projectBodyEvidenceEvents(projectBodySnapshot(evidence));
  assert.deepEqual(events.map(({ type }) => type), ["body-born", "workset-changed", "wake-proposed"]);
  assert.equal(events[1].identity.signId, "sign/2");
  assert.equal(events[2].identity.wakeId, "wake/1");
  assert.equal(events[2].evidence.lifecycleEvent.Woke.sign_id, "sign/3");
  assert(Object.isFrozen(events[2].evidence.lifecycleEvent));
});

test("host offer transitions report observed changes and coalesced generation advances", () => {
  const before = projectBodySnapshot(snapshot([], { current_host_offers: [{ host_id: "host/1", boot_id: "boot/1", offer_generation: 1 }] }));
  const after = projectBodySnapshot(snapshot([], { current_host_offers: [{ host_id: "host/1", boot_id: "boot/1", offer_generation: 4 }] }));
  const event = projectHostOffersChange(before, after);
  assert.equal(event.type, "host-offers-changed");
  assert.deepEqual(event.identity.changedHostBoots, ["host/1/boot/1"]);
  assert.equal(event.pressure.coalescedGenerationAdvances, 2);
  assert.equal(projectHostOffersChange(after, after), null);
});

test("event streams replay only when requested and release their finite subscription", async () => {
  const current = projectBodySnapshot(snapshot([{ sequence: 4, sign_id: "sign/4", kind: { Born: { workload_revision: 0 } } }]));
  let reserved = 0, released = 0;
  const stream = createBodyEventStream({ readSnapshot: async () => current, replay: true, pollIntervalMillis: 50,
    reserve: () => { reserved++; }, release: () => { released++; } });
  const iterator = stream[Symbol.asyncIterator]();
  const event = await iterator.next();
  assert.equal(event.value.identity.recordSequence, 4);
  await iterator.return();
  assert.equal(reserved, 1);
  assert.equal(released, 1);
});

test("unrequested replay starts from current evidence and polling bounds are enforced", async () => {
  const current = projectBodySnapshot(snapshot([{ sequence: 4, sign_id: "sign/4", kind: { Born: { workload_revision: 0 } } }]));
  const abort = new AbortController();
  const stream = createBodyEventStream({ readSnapshot: async () => current, signal: abort.signal, pollIntervalMillis: 50 });
  const iterator = stream[Symbol.asyncIterator]();
  const pending = iterator.next();
  abort.abort();
  assert.deepEqual(await pending, { done: true, value: undefined });
  await assert.rejects(async () => createBodyEventStream({ readSnapshot: async () => current, pollIntervalMillis: 1 })[Symbol.asyncIterator]().next(), /polling interval/);
});

test("compaction is reported from its retained boundary before later biography records", async () => {
  const current = projectBodySnapshot(snapshot([
    { sequence: 9, sign_id: "sign/9", kind: { FormAdmitted: {} } },
  ], { evidence: {
    schema: "conduit.body/biography-evidence@2", body_id: "body/1", body: { workload_revision: 9 },
    membership: { revision: 2 }, records: [{ sequence: 9, sign_id: "sign/9", kind: { FormAdmitted: {} } }],
    wakes: [], compaction: { through_sequence: 8, through_sign_id: "sign/8" },
  } }));
  const iterator = createBodyEventStream({ readSnapshot: async () => current, replay: true, pollIntervalMillis: 50 })[Symbol.asyncIterator]();
  const gap = await iterator.next();
  assert.equal(gap.value.type, "history-gap");
  assert.equal(gap.value.identity.throughSignId, "sign/8");
  const record = await iterator.next();
  assert.equal(record.value.identity.recordSequence, 9);
  await iterator.return();
});
