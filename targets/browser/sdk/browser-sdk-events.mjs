const BODY_SNAPSHOT_SCHEMA = "conduit.workspace/body@1";
const EVENT_SCHEMA = "conduit.browser/body-event@1";
const MAXIMUM_EVENT_RECORDS = 64;
const MAXIMUM_EVENT_SUBSCRIPTIONS = 32;
const MINIMUM_POLL_MILLIS = 50;
const MAXIMUM_POLL_MILLIS = 5_000;

export function projectBodySnapshot(value) {
  if (value?.schema !== BODY_SNAPSHOT_SCHEMA || value.evidence?.schema !== "conduit.body/biography-evidence@2"
    || typeof value.evidence.body_id !== "string" || !Array.isArray(value.evidence.records)
    || value.evidence.records.length > MAXIMUM_EVENT_RECORDS
    || (value.evidence.wakes !== undefined && (!Array.isArray(value.evidence.wakes) || value.evidence.wakes.length > 8))
    || !Array.isArray(value.current_host_offers)) {
    throw new TypeError("Rust Workspace returned an invalid bounded Body snapshot");
  }
  return freezeJson(structuredClone(value));
}

export function projectBodyEvidenceEvents(snapshot, { afterSequence = 0 } = {}) {
  const evidence = snapshot?.evidence;
  if (snapshot?.schema !== BODY_SNAPSHOT_SCHEMA || !Array.isArray(evidence?.records)
    || evidence.records.length > MAXIMUM_EVENT_RECORDS || !Number.isSafeInteger(afterSequence) || afterSequence < 0) {
    throw new TypeError("Body event projection requires one validated Workspace snapshot and bounded cursor");
  }
  const wakes = new Map((evidence.wakes ?? []).map((wake) => [wake.wake_id, wake]));
  const events = [];
  for (const record of evidence.records) {
    if (!Number.isSafeInteger(record.sequence) || record.sequence <= afterSequence
      || typeof record.sign_id !== "string" || !record.kind || typeof record.kind !== "object") continue;
    const [recordKind] = Object.keys(record.kind);
    if (!recordKind) continue;
    const detail = record.kind[recordKind];
    const identity = Object.freeze({
      bodyId: evidence.body_id,
      signId: record.sign_id,
      recordSequence: record.sequence,
      membershipRevision: evidence.membership?.revision ?? 0,
      workloadRevision: evidence.body?.workload_revision ?? 0,
    });
    if (recordKind === "WakeEvent") {
      const wake = wakes.get(detail?.wake_id);
      const lifecycleEvent = wake?.events?.[detail?.event_index];
      const [eventKind] = Object.keys(lifecycleEvent ?? {});
      if (!wake || !eventKind || lifecycleEvent[eventKind]?.sign_id !== record.sign_id) continue;
      events.push(eventForWake(identity, wake, lifecycleEvent));
    } else {
      events.push(Object.freeze({
        schema: EVENT_SCHEMA,
        id: `${evidence.body_id}/record/${record.sequence}/${record.sign_id}`,
        type: bodyEventType(recordKind),
        identity,
        evidence: Object.freeze({ record }),
      }));
    }
  }
  return Object.freeze(events.sort((left, right) => left.identity.recordSequence - right.identity.recordSequence));
}

export function projectHostOffersChange(previous, current) {
  const before = previous?.current_host_offers ?? [];
  const after = current?.current_host_offers ?? [];
  if (JSON.stringify(before) === JSON.stringify(after)) return null;
  const bodyId = current?.evidence?.body_id;
  if (typeof bodyId !== "string") throw new TypeError("Host offer projection lacks its exact Body identity");
  const offers = after.map((offer) => Object.freeze({ ...offer }));
  const prior = new Map(before.map((offer) => [`${offer.host_id}/${offer.boot_id}`, offer]));
  const currentKeys = new Set(after.map((offer) => `${offer.host_id}/${offer.boot_id}`));
  const changed = after.filter((offer) => JSON.stringify(prior.get(`${offer.host_id}/${offer.boot_id}`)) !== JSON.stringify(offer));
  const removed = before.filter((offer) => !currentKeys.has(`${offer.host_id}/${offer.boot_id}`));
  const skippedGenerations = changed.reduce((count, offer) => {
    const priorGeneration = Number(prior.get(`${offer.host_id}/${offer.boot_id}`)?.offer_generation) || 0;
    const nextGeneration = Number(offer.offer_generation) || 0;
    return count + Math.max(0, nextGeneration - priorGeneration - 1);
  }, 0);
  const generation = Math.max(0, ...offers.map((offer) => Number(offer.offer_generation) || 0));
  const bootIds = offers.map((offer) => `${offer.host_id}/${offer.boot_id}`).filter((value) => value !== "undefined/undefined").sort();
  return Object.freeze({
    schema: EVENT_SCHEMA,
    id: `${bodyId}/host-offers/${generation}/${bootIds.join(",")}`,
    type: "host-offers-changed",
    identity: Object.freeze({ bodyId, offerGeneration: generation, bootIds: Object.freeze(bootIds), changedHostBoots: Object.freeze(changed.map((offer) => `${offer.host_id}/${offer.boot_id}`)), removedHostBoots: Object.freeze(removed.map((offer) => `${offer.host_id}/${offer.boot_id}`)) }),
    pressure: Object.freeze({ coalescedGenerationAdvances: skippedGenerations }),
    evidence: Object.freeze({ previousHostOffers: freezeJson(structuredClone(before)), currentHostOffers: Object.freeze(offers) }),
  });
}

export function createBodyEventStream({ readSnapshot, signal, replay = false, pollIntervalMillis = 250, reserve, release }) {
  if (typeof readSnapshot !== "function") throw new TypeError("Body event stream requires an authoritative snapshot reader");
  if (!Number.isSafeInteger(pollIntervalMillis) || pollIntervalMillis < MINIMUM_POLL_MILLIS || pollIntervalMillis > MAXIMUM_POLL_MILLIS) {
    throw new RangeError(`Body event polling interval must be ${MINIMUM_POLL_MILLIS}..${MAXIMUM_POLL_MILLIS} milliseconds`);
  }
  return {
    async *[Symbol.asyncIterator]() {
      reserve?.();
      try {
        let snapshot = await readSnapshot();
        let afterSequence = replay ? 0 : latestSequence(snapshot);
        let previous = snapshot;
        let first = true;
        while (!signal?.aborted) {
          const next = first ? snapshot : await readSnapshot();
          first = false;
          const compaction = next.evidence.compaction;
          if (compaction && compaction.through_sequence > afterSequence) {
            yield Object.freeze({
              schema: EVENT_SCHEMA,
              id: `${next.evidence.body_id}/history-gap/${compaction.through_sequence}/${compaction.through_sign_id}`,
              type: "history-gap",
              identity: Object.freeze({ bodyId: next.evidence.body_id, throughSequence: compaction.through_sequence, throughSignId: compaction.through_sign_id }),
              evidence: Object.freeze({ compaction: freezeJson(structuredClone(compaction)) }),
            });
            afterSequence = compaction.through_sequence;
          }
          const events = projectBodyEvidenceEvents(next, { afterSequence });
          for (const event of events) {
            afterSequence = Math.max(afterSequence, event.identity.recordSequence);
            yield event;
          }
          const offersChanged = projectHostOffersChange(previous, next);
          if (offersChanged) yield offersChanged;
          previous = next;
          snapshot = next;
          await wait(pollIntervalMillis, signal);
        }
      } finally {
        release?.();
      }
    },
  };
}

export const maximumBodyEventSubscriptions = MAXIMUM_EVENT_SUBSCRIPTIONS;

function eventForWake(identity, wake, lifecycleEvent) {
  const [kind] = Object.keys(lifecycleEvent);
  return Object.freeze({
    schema: EVENT_SCHEMA,
    id: `${identity.bodyId}/wake/${wake.wake_id}/${lifecycleEvent[kind].sign_id}`,
    type: wakeEventType(kind),
    identity: Object.freeze({ ...identity, wakeId: wake.wake_id, ...(lifecycleEvent[kind]?.active_play_id ? { playId: lifecycleEvent[kind].active_play_id } : {}), ...(lifecycleEvent[kind]?.plan_id ? { planId: lifecycleEvent[kind].plan_id } : {}) }),
    evidence: Object.freeze({ lifecycleEvent: freezeJson(structuredClone(lifecycleEvent)) }),
  });
}

function bodyEventType(kind) {
  return ({ Born: "body-born", PartAdmitted: "part-admitted", HostJoined: "host-attached", HostLeft: "host-lost", PartRevoked: "part-revoked", FormAdmitted: "workset-changed", FormRemoved: "workset-changed", Fulfilled: "body-fulfilled", Graduated: "body-graduated", EmergencyConfigured: "emergency-configured", LullRetained: "lull" })[kind] ?? "body-evidence";
}

function wakeEventType(kind) {
  return ({ Woke: "wake-proposed", PlanReady: "plan-ready", PlanHeld: "plan-held", HeldPlanReleased: "plan-resumed", HeldPlanInvalidated: "plan-invalidated", PlayStarted: "play-started", BecameUnsatisfied: "play-unsatisfied", WorkloadChanged: "workset-changed", Replanned: "plan-replaced", SamePlanObserved: "plan-observed", Lulled: "lull", Failed: "wake-failed" })[kind] ?? "wake-evidence";
}

function latestSequence(snapshot) {
  const records = snapshot.evidence.records;
  return records.reduce((maximum, record) => Math.max(maximum, Number(record.sequence) || 0), 0);
}

function freezeJson(value) {
  if (!value || typeof value !== "object" || Object.isFrozen(value)) return value;
  for (const child of Object.values(value)) freezeJson(child);
  return Object.freeze(value);
}

function wait(milliseconds, signal) {
  if (signal?.aborted) return Promise.resolve();
  return new Promise((resolve) => {
    const timer = setTimeout(finish, milliseconds);
    function finish() {
      clearTimeout(timer);
      signal?.removeEventListener("abort", finish);
      resolve();
    }
    signal?.addEventListener("abort", finish, { once: true });
  });
}
