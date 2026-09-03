const DEFAULTS = Object.freeze({
  maximumPeers: 4,
  maximumPulseHistory: 16,
  maximumQueuedObservations: 8,
  maximumEvidenceItems: 32,
  maximumTimerBursts: 8,
  minimumPeriodMs: 160,
  defaultPeriodMs: 240,
  maximumPeriodMs: 960,
  synchronizationWindowMs: 320,
  maximumPhaseAdjustMs: 64,
  maximumPeriodAdjustMs: 16,
  lightPulseMs: 80,
  tonePulseMs: 60,
  toneFrequencyHz: 880,
  observationBytes: 6,
});

export const FireflyChoirFailure = Object.freeze({
  InvalidBinding: "CND-CHOIR-001",
  InvalidTime: "CND-CHOIR-002",
  InvalidPeer: "CND-CHOIR-003",
  InvalidObservation: "CND-CHOIR-004",
});

export function encodePulseObservation({ sequence, periodMs }) {
  if (!Number.isSafeInteger(sequence) || sequence < 0
    || !Number.isSafeInteger(periodMs)
    || periodMs < DEFAULTS.minimumPeriodMs
    || periodMs > DEFAULTS.maximumPeriodMs) {
    refuse(FireflyChoirFailure.InvalidObservation, "pulse observation is outside the admitted bounds");
  }
  const bytes = new Uint8Array(DEFAULTS.observationBytes);
  const view = new DataView(bytes.buffer);
  view.setUint32(0, sequence, true);
  view.setUint16(4, periodMs, true);
  return bytes;
}

export function decodePulseObservation(bytes) {
  const value = exactBytes(bytes, DEFAULTS.observationBytes);
  if (value === null) return null;
  const view = new DataView(value.buffer, value.byteOffset, value.byteLength);
  const periodMs = view.getUint16(4, true);
  if (periodMs < DEFAULTS.minimumPeriodMs || periodMs > DEFAULTS.maximumPeriodMs) return null;
  return Object.freeze({
    sequence: view.getUint32(0, true),
    periodMs,
  });
}

export function createFireflySynchronizer(options = {}) {
  const limits = configure(options);
  const peerStates = Array.from({ length: limits.maximumPeers }, () => ({
    accepted: 0,
    expectedSequence: 0,
    lastReceivedAtMs: null,
  }));
  const history = [];
  const evidence = [];
  const outbound = [];
  let lineState = "ready";
  let started = false;
  let pressureCount = 0;
  let periodMs = limits.defaultPeriodMs;
  let nextSequence = 0;
  let nextPulseAtMs = 0;
  let lastPulseAtMs = null;

  function pushHistory(entry) {
    if (history.length === limits.maximumPulseHistory) history.shift();
    history.push(Object.freeze(entry));
  }

  function record(entry) {
    if (evidence.length === limits.maximumEvidenceItems) evidence.shift();
    evidence.push(Object.freeze(entry));
  }

  function queueObservation(observation, atMs, source) {
    if (outbound.length === limits.maximumQueuedObservations) {
      pressureCount += 1;
      record({
        kind: "pressure",
        atMs,
        source,
        reason: "outbound-observation-bound",
        sequence: observation.sequence,
      });
      return false;
    }
    outbound.push(encodePulseObservation(observation));
    return true;
  }

  function emitPulse(atMs, source) {
    const observation = Object.freeze({
      sequence: nextSequence,
      periodMs,
    });
    nextSequence += 1;
    lastPulseAtMs = atMs;
    const queued = queueObservation(observation, atMs, source);
    pushHistory({
      kind: "local",
      atMs,
      source,
      sequence: observation.sequence,
      queued,
    });
    const pulse = Object.freeze({
      kind: "pulse",
      atMs,
      source,
      sequence: observation.sequence,
      periodMs,
      queued,
      lightUntilMs: atMs + limits.lightPulseMs,
      tone: limits.enableTone
        ? Object.freeze({
            durationMs: limits.tonePulseMs,
            frequencyHz: limits.toneFrequencyHz,
          })
        : null,
    });
    record(pulse);
    return pulse;
  }

  function retimeTowards(targetPulseAtMs) {
    const error = targetPulseAtMs - nextPulseAtMs;
    if (Math.abs(error) > limits.synchronizationWindowMs) return 0;
    const adjustment = clamp(
      Math.round(error / 2),
      -limits.maximumPhaseAdjustMs,
      limits.maximumPhaseAdjustMs,
    );
    nextPulseAtMs += adjustment;
    return adjustment;
  }

  function adjustPeriod(targetPeriodMs) {
    const adjustment = clamp(
      Math.round((targetPeriodMs - periodMs) / 4),
      -limits.maximumPeriodAdjustMs,
      limits.maximumPeriodAdjustMs,
    );
    periodMs = clamp(
      periodMs + adjustment,
      limits.minimumPeriodMs,
      limits.maximumPeriodMs,
    );
    return adjustment;
  }

  return Object.freeze({
    tap(atMs) {
      const admittedAtMs = exactTime(atMs);
      const wasStarted = started;
      started = true;
      const pulse = emitPulse(admittedAtMs, "tap");
      nextPulseAtMs = wasStarted
        ? Math.min(nextPulseAtMs, admittedAtMs + periodMs)
        : admittedAtMs + periodMs;
      record({ kind: "schedule", atMs: admittedAtMs, nextPulseAtMs, periodMs });
      return Object.freeze({ pulse, state: snapshot() });
    },
    advance(atMs) {
      const admittedAtMs = exactTime(atMs);
      const pulses = [];
      while (started && admittedAtMs >= nextPulseAtMs && pulses.length < limits.maximumTimerBursts) {
        const emittedAtMs = nextPulseAtMs;
        pulses.push(emitPulse(emittedAtMs, "timer"));
        nextPulseAtMs += periodMs;
      }
      if (pulses.length > 0) {
        record({
          kind: "advance",
          atMs: admittedAtMs,
          emitted: pulses.length,
          nextPulseAtMs,
          periodMs,
        });
      }
      return Object.freeze({ pulses: Object.freeze(pulses), state: snapshot() });
    },
    ingest(peerIndex, bytes, atMs) {
      const admittedPeer = exactPeer(peerIndex, limits.maximumPeers);
      const admittedAtMs = exactTime(atMs);
      const observation = decodePulseObservation(bytes);
      if (observation === null) {
        record({
          kind: "peer-refused",
          atMs: admittedAtMs,
          peerIndex: admittedPeer,
          reason: "malformed-observation",
        });
        return Object.freeze({ accepted: false, reason: "malformed-observation", state: snapshot() });
      }
      const peer = peerStates[admittedPeer];
      if (observation.sequence !== peer.expectedSequence) {
        record({
          kind: "peer-refused",
          atMs: admittedAtMs,
          peerIndex: admittedPeer,
          reason: "unexpected-sequence",
          expectedSequence: peer.expectedSequence,
          observedSequence: observation.sequence,
        });
        return Object.freeze({ accepted: false, reason: "unexpected-sequence", state: snapshot() });
      }
      const arrivalPeriodMs = peer.lastReceivedAtMs === null
        ? null
        : admittedAtMs - peer.lastReceivedAtMs;
      const targetPeriodMs = arrivalPeriodMs !== null
        && arrivalPeriodMs >= limits.minimumPeriodMs
        && arrivalPeriodMs <= limits.maximumPeriodMs * 2
        ? clamp(
            Math.round((arrivalPeriodMs + observation.periodMs) / 2),
            limits.minimumPeriodMs,
            limits.maximumPeriodMs,
          )
        : observation.periodMs;
      const periodAdjustmentMs = adjustPeriod(targetPeriodMs);
      if (!started) {
        started = true;
        nextPulseAtMs = admittedAtMs + Math.round(periodMs / 2);
      } else {
        retimeTowards(admittedAtMs);
      }
      peer.expectedSequence += 1;
      peer.accepted += 1;
      peer.lastReceivedAtMs = admittedAtMs;
      pushHistory({
        kind: "peer",
        atMs: admittedAtMs,
        peerIndex: admittedPeer,
        sequence: observation.sequence,
        periodMs: observation.periodMs,
      });
      record({
        kind: "peer",
        atMs: admittedAtMs,
        peerIndex: admittedPeer,
        sequence: observation.sequence,
        observedPeriodMs: observation.periodMs,
        localPeriodMs: periodMs,
        nextPulseAtMs,
        periodAdjustmentMs,
      });
      return Object.freeze({ accepted: true, state: snapshot() });
    },
    takeOutbound() {
      return Object.freeze(outbound.splice(0).map((bytes) => Object.freeze([...bytes])));
    },
    markLineState(state, detail = null) {
      lineState = ["ready", "linked", "lost"].includes(state) ? state : "lost";
      record({ kind: "line", state: lineState, detail });
      return snapshot();
    },
    lightActive(atMs) {
      const admittedAtMs = exactTime(atMs);
      return lastPulseAtMs !== null && admittedAtMs < lastPulseAtMs + limits.lightPulseMs;
    },
    snapshot,
    drainEvidence() {
      return Object.freeze(evidence.splice(0));
    },
  });

  function snapshot() {
    return Object.freeze({
      started,
      periodMs,
      nextPulseAtMs,
      nextSequence,
      lastPulseAtMs,
      lineState,
      enableTone: limits.enableTone,
      pressureCount,
      pendingObservations: outbound.length,
      peerStates: Object.freeze(peerStates.map((peer) => Object.freeze({ ...peer }))),
      history: Object.freeze([...history]),
    });
  }
}

export function createFireflyChoirBrowserHost({
  button,
  light,
  status,
  log = null,
  clock = globalThis.performance ?? { now: () => Date.now() },
  enableTone = true,
  toneSink = null,
} = {}) {
  if (!(button instanceof Element) || !(light instanceof Element) || !(status instanceof Element)) {
    refuse(FireflyChoirFailure.InvalidBinding, "browser host requires button, light, and status Elements");
  }
  const synchronizer = createFireflySynchronizer({ enableTone });
  const tones = [];
  const sink = toneSink ?? defaultToneSink(tones);

  function handle(result) {
    for (const pulse of "pulse" in result ? [result.pulse] : result.pulses) {
      if (pulse?.tone) sink(pulse.tone);
    }
    render();
    appendLog(result);
    return state();
  }

  function render(nowMs = Math.round(clock.now())) {
    const snapshot = synchronizer.snapshot();
    const active = synchronizer.lightActive(nowMs);
    light.dataset.state = active ? "on" : "off";
    light.textContent = active ? "●" : "○";
    status.textContent =
      `line=${snapshot.lineState} pulses=${snapshot.nextSequence} ` +
      `period=${snapshot.periodMs}ms audio=${snapshot.enableTone ? "enabled" : "omitted"}`;
  }

  function appendLog(result) {
    if (!(log instanceof Element)) return;
    const item = document.createElement("li");
    const snapshot = synchronizer.snapshot();
    item.textContent =
      `pulses=${snapshot.nextSequence} line=${snapshot.lineState} ` +
      `pending=${snapshot.pendingObservations} pressure=${snapshot.pressureCount}`;
    log.append(item);
  }

  button.addEventListener("click", () => {
    handle(synchronizer.tap(Math.round(clock.now())));
  });
  button.addEventListener("keydown", (event) => {
    if (!["Space", "Enter"].includes(event.code) || event.repeat) return;
    event.preventDefault();
    handle(synchronizer.tap(Math.round(clock.now())));
  });

  render(0);

  function state() {
    return Object.freeze({
      ...synchronizer.snapshot(),
      tones: Object.freeze([...tones]),
      lightState: light.dataset.state,
      status: status.textContent,
    });
  }

  return Object.freeze({
    advance(atMs = Math.round(clock.now())) {
      return handle(synchronizer.advance(atMs));
    },
    receive(bytes, atMs = Math.round(clock.now())) {
      const result = synchronizer.ingest(0, bytes, atMs);
      render(atMs);
      appendLog(result);
      return Object.freeze({
        accepted: result.accepted,
        reason: result.reason ?? null,
        state: state(),
      });
    },
    takeOutbound() {
      return synchronizer.takeOutbound();
    },
    markLinked() {
      synchronizer.markLineState("linked");
      render();
      return state();
    },
    markLineLost(reason) {
      synchronizer.markLineState("lost", reason ?? null);
      render();
      return state();
    },
    state,
    render,
  });
}

function configure(options) {
  const configured = {
    ...DEFAULTS,
    ...options,
    enableTone: options.enableTone !== false,
  };
  for (const key of [
    "maximumPeers",
    "maximumPulseHistory",
    "maximumQueuedObservations",
    "maximumEvidenceItems",
    "maximumTimerBursts",
    "minimumPeriodMs",
    "defaultPeriodMs",
    "maximumPeriodMs",
    "synchronizationWindowMs",
    "maximumPhaseAdjustMs",
    "maximumPeriodAdjustMs",
    "lightPulseMs",
    "tonePulseMs",
    "toneFrequencyHz",
  ]) {
    if (!Number.isSafeInteger(configured[key]) || configured[key] < 1) {
      refuse(FireflyChoirFailure.InvalidBinding, `invalid firefly choir limit ${key}`);
    }
  }
  if (configured.defaultPeriodMs < configured.minimumPeriodMs
    || configured.defaultPeriodMs > configured.maximumPeriodMs) {
    refuse(FireflyChoirFailure.InvalidBinding, "default period is outside the admitted range");
  }
  return Object.freeze(configured);
}

function defaultToneSink(tones) {
  return (tone) => {
    tones.push(Object.freeze({ ...tone }));
    return Object.freeze({ ok: true });
  };
}

function exactBytes(bytes, expectedLength) {
  const value = bytes instanceof Uint8Array
    ? bytes
    : Array.isArray(bytes)
      ? Uint8Array.from(bytes)
      : null;
  return value?.length === expectedLength ? value : null;
}

function exactPeer(peerIndex, maximumPeers) {
  if (!Number.isSafeInteger(peerIndex) || peerIndex < 0 || peerIndex >= maximumPeers) {
    refuse(FireflyChoirFailure.InvalidPeer, "peer index is outside the admitted bound");
  }
  return peerIndex;
}

function exactTime(value) {
  if (!Number.isSafeInteger(value) || value < 0) {
    refuse(FireflyChoirFailure.InvalidTime, "time is missing or outside the admitted bound");
  }
  return value;
}

function clamp(value, minimum, maximum) {
  return Math.min(maximum, Math.max(minimum, value));
}

function refuse(code, message) {
  const error = new Error(message);
  error.code = code;
  throw error;
}
