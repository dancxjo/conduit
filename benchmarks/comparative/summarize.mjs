import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const input = process.argv[2];
const output = process.argv[3];
const reportOutput = process.argv[4];
if (!input || !output) throw new Error("usage: node summarize.mjs RAW.ndjson SUMMARY.json [REPORT.md]");
const samples = fs.readFileSync(input, "utf8").trim().split("\n").filter(Boolean).map(JSON.parse);
if (samples.length === 0) throw new Error("raw sample file is empty");

function expectedUseful(sample) {
  if (sample.workload.id !== "map-filter" || sample.workload.operators < 2) return sample.workload.input_values;
  return Math.ceil(sample.workload.input_values / 2);
}

for (const sample of samples) {
  if (sample.schema !== "conduit.comparative-raw-sample" || sample.schema_version !== 0 || sample.fixture_revision !== 0) {
    throw new Error("sample schema or fixture revision does not match the current manifest");
  }
  if (!["warmup", "measured"].includes(sample.sample_kind) || !["cold", "warming", "warmed"].includes(sample.thermal_state)) {
    throw new Error("sample warm-up identity is invalid");
  }
  if (!["reactive-runtime", "language-lower-bound"].includes(sample.runtime.comparison_role)) {
    throw new Error("sample comparison role is invalid");
  }
  const identityParts = sample.exact_identity.logical_fixture.split("/");
  const overload = sample.workload.id === "overload";
  const fanout = sample.workload.id === "fanout";
  const sharedPayload = sample.workload.id === "shared-payload-fanout";
  const persistentWake = sample.workload.id === "persistent-wake";
  const persistentTimer = sample.workload.id === "persistent-timer";
  const persistentResidency = persistentWake || persistentTimer;
  if (!sharedPayload && (sample.workload.payload_bytes !== 0
      || !["handle-backed-u64", "native-u64", "native-number", "native-long"].includes(sample.workload.payload_representation)
      || sample.workload.watch_slots !== 0
      || sample.workload.watch_preview_bytes !== 0
      || sample.workload.watch_retention !== "none"
      || [sample.memory.watch_admitted_slots, sample.memory.watch_attached_slots,
        sample.memory.watch_retained_observations, sample.memory.watch_retained_preview_bytes,
        sample.memory.watch_dropped_observations, sample.memory.watch_maximum_observations,
        sample.memory.watch_maximum_preview_bytes].some((value) => value !== null))) {
    throw new Error("non-payload fixture carries shared-payload identity");
  }
  if (overload) {
    const pressureId = sample.workload.pressure.split("/")[0];
    if (identityParts.length !== 12
        || identityParts[0] !== "comparative-overload"
        || identityParts[1] !== pressureId
        || identityParts[2] !== sample.workload.termination_request
        || identityParts[3] !== sample.workload.session_mode
        || Number(identityParts[4]) !== sample.workload.session_pump_quantum
        || identityParts[5] !== sample.workload.consumer_pattern
        || Number(identityParts[6]) !== sample.workload.consumer_burst_items
        || Number(identityParts[7]) !== sample.workload.input_values
        || Number(identityParts[8]) !== sample.workload.queue_capacity_items
        || Number(identityParts[9]) !== sample.workload.slow_consumer_yields
        || Number(identityParts[10]) !== sample.workload.cancel_after_offers
        || Number(identityParts[11]) !== sample.latency.sample_stride) {
      throw new Error("overload fixture identity does not match the raw sample");
    }
    if (sample.runtime.id !== "conduit-reference-scheduler") {
      throw new Error("the current overload slice has no cross-runtime substitute");
    }
  } else if (fanout) {
    if (identityParts.length !== 10
        || identityParts[0] !== "comparative-fanout"
        || identityParts[1] !== sample.workload.fanout_mode
        || identityParts[2] !== sample.workload.slow_branches
        || identityParts[3] !== sample.workload.consumer_pattern
        || Number(identityParts[4]) !== sample.workload.consumer_burst_items
        || Number(identityParts[5]) !== sample.workload.fanout_branches
        || Number(identityParts[6]) !== sample.workload.input_values
        || Number(identityParts[7]) !== sample.workload.queue_capacity_items
        || Number(identityParts[8]) !== sample.workload.slow_consumer_yields
        || Number(identityParts[9]) !== sample.latency.sample_stride) {
      throw new Error("fan-out fixture identity does not match the raw sample");
    }
    if (sample.runtime.id !== "conduit-reference-scheduler") {
      throw new Error("the current fan-out slice has no cross-runtime substitute");
    }
  } else if (sharedPayload) {
    if (identityParts.length !== 7
        || identityParts[0] !== "comparative-shared-payload-fanout"
        || Number(identityParts[1]) !== sample.workload.fanout_branches
        || Number(identityParts[2]) !== sample.workload.payload_bytes
        || Number(identityParts[3]) !== sample.workload.queue_capacity_items
        || identityParts[4] !== sample.workload.termination_request
        || Number(identityParts[5]) !== sample.workload.watch_slots
        || Number(identityParts[6]) !== sample.workload.watch_preview_bytes) {
      throw new Error("shared-payload fixture identity does not match the raw sample");
    }
    if (sample.runtime.id !== "conduit-hosted-value-arena") {
      throw new Error("the current shared-payload slice has no cross-runtime substitute");
    }
  } else if (persistentWake) {
    if (identityParts.length !== 6
        || identityParts[0] !== "comparative-persistent-wake"
        || Number(identityParts[1]) !== sample.workload.input_values
        || Number(identityParts[2]) !== sample.workload.residency_plateau_after_wakes
        || Number(identityParts[3]) !== sample.workload.queue_capacity_items
        || Number(identityParts[4]) !== sample.workload.session_pump_quantum
        || Number(identityParts[5]) !== sample.latency.sample_stride) {
      throw new Error("persistent host-wake fixture identity does not match the raw sample");
    }
    if (sample.runtime.id !== "conduit-reference-scheduler") {
      throw new Error("the current persistent host-wake slice has no cross-runtime substitute");
    }
  } else if (persistentTimer) {
    if (identityParts.length !== 7
        || identityParts[0] !== "comparative-persistent-timer"
        || Number(identityParts[1]) !== sample.workload.input_values
        || Number(identityParts[2]) !== sample.workload.residency_plateau_after_wakes
        || Number(identityParts[3]) !== sample.workload.queue_capacity_items
        || Number(identityParts[4]) !== sample.workload.session_pump_quantum
        || Number(identityParts[5]) !== sample.workload.timer_advance_ticks
        || Number(identityParts[6]) !== sample.latency.sample_stride) {
      throw new Error("persistent timer fixture identity does not match the raw sample");
    }
    if (sample.runtime.id !== "conduit-reference-scheduler") {
      throw new Error("the current persistent timer slice has no cross-runtime substitute");
    }
  } else if (identityParts.length !== 6
      || identityParts[0] !== "comparative-local-depth"
      || identityParts[1] !== sample.workload.id
      || Number(identityParts[2]) !== sample.workload.operators
      || Number(identityParts[3]) !== sample.workload.input_values
      || Number(identityParts[5]) !== sample.latency.sample_stride) {
    throw new Error("local-depth fixture identity does not match the raw sample");
  }
  if (["conduit-reference-scheduler", "conduit-hosted-value-arena"].includes(sample.runtime.id)
      && [sample.exact_identity.plan_identity, sample.exact_identity.source_semantic_hash, sample.exact_identity.artifact_digest].some((value) => !value)) {
    throw new Error("Conduit sample omitted exact plan, source, or artifact identity");
  }
  if (["conduit-reference-scheduler", "conduit-hosted-value-arena"].includes(sample.runtime.id)
      && (sample.execution.scheduler_decisions === null || sample.execution.producer_stall_ns === null)) {
    throw new Error("Conduit sample omitted scheduler decisions or producer stall time");
  }
  const persistent = sample.workload.session_mode === "persistent-exact-run-session";
  if (persistentTimer) {
    if (sample.workload.timer_advance_ticks <= sample.workload.session_pump_quantum) {
      throw new Error("persistent timer advance does not stay ahead of a bounded pump");
    }
  } else if (sample.workload.timer_advance_ticks !== 0) {
    throw new Error("non-timer fixture carries an unused timer advance");
  }
  if (persistent) {
    if (sample.workload.session_pump_quantum <= 0
        || sample.execution.session_pumps <= 1
        || sample.execution.session_reserved_bytes <= 0) {
      throw new Error("persistent exact-run session ownership accounting changed");
    }
    if (persistentResidency) {
      const wakeCount = persistentWake
        ? sample.execution.session_host_wakes
        : sample.execution.session_timer_wakes;
      const unusedWakeCount = persistentWake
        ? sample.execution.session_timer_wakes
        : sample.execution.session_host_wakes;
      const expectedPressure = persistentWake
        ? "exact host wake to bounded FIFO"
        : "exact timer wake to bounded FIFO";
      if (sample.workload.pressure !== expectedPressure
          || sample.workload.residency_plateau_after_wakes <= 0
          || sample.workload.residency_plateau_after_wakes >= sample.workload.input_values
          || wakeCount !== sample.workload.input_values
          || unusedWakeCount !== null
          || sample.execution.residency_plateau_verified !== true
          || sample.execution.pressured_items_at_stop !== null
          || sample.execution.residency_checkpoint_queue_items_high_water !== sample.memory.queue_items_high_water
          || sample.execution.residency_checkpoint_queue_payload_bytes_high_water !== sample.memory.queue_payload_bytes_high_water
          || sample.execution.residency_checkpoint_ready_slots_high_water !== sample.memory.ready_slots_high_water
          || sample.execution.residency_checkpoint_evidence_slots_high_water !== sample.memory.evidence_slots_high_water) {
        throw new Error("persistent wake residency accounting changed");
      }
    } else if (!overload || !["block", "reject", "coalesce/latest-wins", "sample/every-2-offset-0", "drop-disposable"].includes(sample.workload.pressure)
        || sample.workload.residency_plateau_after_wakes !== 0
        || sample.workload.cancel_after_offers !== sample.workload.input_values
        || sample.execution.pressured_items_at_stop <= 0
        || sample.execution.session_host_wakes !== null
        || sample.execution.session_timer_wakes !== null
        || sample.execution.residency_plateau_verified !== null
        || [sample.execution.residency_checkpoint_queue_items_high_water,
          sample.execution.residency_checkpoint_queue_payload_bytes_high_water,
          sample.execution.residency_checkpoint_ready_slots_high_water,
          sample.execution.residency_checkpoint_evidence_slots_high_water].some((value) => value !== null)) {
      throw new Error("persistent pressure session identity or admission accounting changed");
    }
  } else if (sharedPayload) {
    if (sample.workload.session_mode !== "finite-exact-run-session"
        || sample.workload.session_pump_quantum !== 512
        || sample.execution.session_pumps < 1
        || sample.execution.session_reserved_bytes <= 0) {
      throw new Error("shared-payload exact session ownership accounting changed");
    }
  } else if (sample.workload.session_mode !== "finite-executor"
      || sample.workload.session_pump_quantum !== 0
      || sample.workload.residency_plateau_after_wakes !== 0
      || sample.execution.session_pumps !== null
      || sample.execution.session_reserved_bytes !== null
      || sample.execution.session_host_wakes !== null
      || sample.execution.session_timer_wakes !== null
      || sample.execution.residency_plateau_verified !== null
      || [sample.execution.residency_checkpoint_queue_items_high_water,
        sample.execution.residency_checkpoint_queue_payload_bytes_high_water,
        sample.execution.residency_checkpoint_ready_slots_high_water,
        sample.execution.residency_checkpoint_evidence_slots_high_water].some((value) => value !== null)) {
    throw new Error("finite executor fixture carries persistent session state");
  }
  const pressured = overload || fanout;
  if (pressured && sample.workload.consumer_pattern === "bursty") {
    if (sample.workload.consumer_burst_items <= 0
        || sample.phases.pressure_ns !== null || sample.phases.recovery_ns !== null
        || sample.phases.pressure_cycles < 2
        || ![sample.phases.pressure_cycles, sample.phases.pressure_cycles + 1]
          .includes(sample.phases.recovery_cycles)) {
      throw new Error("bursty consumer did not expose repeated exact pressure/recovery cycles");
    }
  } else if (pressured) {
    if (sample.workload.consumer_pattern !== "sustained-slow-then-recover"
        || sample.workload.consumer_burst_items !== 0
        || sample.phases.pressure_cycles !== 1) {
      throw new Error("sustained consumer identity or pressure cycle changed");
    }
  } else if (sample.workload.consumer_pattern !== "none"
      || sample.workload.consumer_burst_items !== 0
      || sample.phases.pressure_cycles !== null || sample.phases.recovery_cycles !== null) {
    throw new Error("local-depth fixture carries consumer pressure identity");
  }
  if (sample.workload.termination_request === "complete") {
    if (sample.workload.cancel_after_offers !== 0
        || sample.execution.drain_ns !== null || sample.execution.abort_ns !== null
        || sample.execution.pressured_items_at_stop !== null
        || sample.outcomes.cancelled !== 0) {
      throw new Error("complete fixture carries cancellation state or timing");
    }
  } else if (persistentResidency) {
    if (sample.workload.termination_request !== "drain"
        || sample.workload.cancel_after_offers !== sample.workload.input_values
        || sample.execution.drain_ns === null || sample.execution.abort_ns !== null
        || sample.execution.pressured_items_at_stop !== null
        || sample.outcomes.cancelled !== 0) {
      throw new Error("persistent wake Drain identity is invalid");
    }
  } else if (sharedPayload) {
    if (sample.workload.termination_request !== "abort"
        || sample.workload.cancel_after_offers !== 1
        || sample.execution.drain_ns !== null || sample.execution.abort_ns === null
        || sample.execution.pressured_items_at_stop !== sample.workload.fanout_branches
        || sample.outcomes.cancelled !== 1) {
      throw new Error("shared-payload Abort identity is invalid");
    }
  } else if (!["drain", "abort"].includes(sample.workload.termination_request)
      || sample.workload.cancel_after_offers <= sample.workload.queue_capacity_items
      || sample.execution.pressured_items_at_stop <= 0) {
    throw new Error("cancellation fixture identity is invalid");
  }
  if (sample.outcomes.terminal !== 1) throw new Error("fixture must report one terminal signal");
  if (sample.runtime.id === "conduit-reference-scheduler"
      && sample.outcomes.retried > 0 && sample.execution.producer_stall_ns <= 0) {
    throw new Error("a blocked Conduit producer reported no stall duration");
  }
  if (overload) {
    const outcomes = sample.outcomes;
    const capacity = sample.workload.queue_capacity_items;
    if (sample.memory.queue_items_high_water > capacity) throw new Error("overload queue exceeded declared capacity");
    if (outcomes.offered > sample.workload.input_values) throw new Error("overload offered more than its finite source");
    const pressureId = sample.workload.pressure.split("/")[0];
    const termination = sample.workload.termination_request;
    if (["drain", "abort"].includes(termination)) {
      if (!["block", "reject", "coalesce", "sample", "drop-disposable"].includes(pressureId)
          || outcomes.offered !== sample.workload.cancel_after_offers
          || outcomes.offered !== outcomes.admitted + outcomes.rejected + outcomes.sampled + outcomes.dropped + outcomes.cancelled) {
        throw new Error("pressured cancellation accounting is not conservative");
      }
      if (pressureId === "block" && (outcomes.retried < 1
          || [outcomes.rejected, outcomes.sampled, outcomes.coalesced, outcomes.dropped].some((value) => value !== 0))) {
        throw new Error("block cancellation did not preserve FIFO retry accounting");
      }
      if (pressureId === "reject" && (outcomes.rejected < 1
          || [outcomes.sampled, outcomes.coalesced, outcomes.dropped].some((value) => value !== 0))) {
        throw new Error("reject cancellation did not preserve explicit rejection accounting");
      }
      if (pressureId === "coalesce" && (outcomes.coalesced < 1
          || [outcomes.rejected, outcomes.sampled, outcomes.dropped].some((value) => value !== 0))) {
        throw new Error("coalesce cancellation did not preserve replacement accounting");
      }
      if (pressureId === "sample" && (outcomes.sampled < 1
          || outcomes.rejected !== 0 || outcomes.coalesced !== 0)) {
        throw new Error("sample cancellation did not preserve schedule/loss accounting");
      }
      if (pressureId === "drop-disposable" && (outcomes.dropped < 1
          || [outcomes.rejected, outcomes.sampled, outcomes.coalesced].some((value) => value !== 0))) {
        throw new Error("disposable-drop cancellation did not preserve explicit loss accounting");
      }
      const retainedAdmitted = outcomes.admitted - outcomes.coalesced;
      if (termination === "drain" && (outcomes.completed_useful !== retainedAdmitted
          || sample.execution.drain_ns === null || sample.execution.abort_ns !== null)) {
        throw new Error("Drain did not preserve all admitted work or exact timing identity");
      }
      if (termination === "abort" && (outcomes.completed_useful + sample.execution.pressured_items_at_stop !== retainedAdmitted
          || sample.execution.abort_ns === null || sample.execution.drain_ns !== null)) {
        throw new Error("Abort accounting or exact timing identity is invalid");
      }
      if (sample.phases.recovery_ns !== null || sample.phases.recovery_cycles !== 0) {
        throw new Error("pressured cancellation unexpectedly entered recovery");
      }
    } else if (pressureId === "block") {
      if (outcomes.offered !== sample.workload.input_values || outcomes.admitted !== outcomes.offered
          || outcomes.completed_useful !== outcomes.admitted || outcomes.retried < 1
          || [outcomes.rejected, outcomes.sampled, outcomes.coalesced, outcomes.dropped].some((value) => value !== 0)) {
        throw new Error("block overload accounting is not conservative");
      }
    } else if (pressureId === "reject") {
      if (outcomes.offered !== outcomes.admitted + outcomes.rejected || outcomes.completed_useful !== outcomes.admitted) {
        throw new Error("reject overload accounting is not conservative");
      }
    } else if (pressureId === "coalesce") {
      if (outcomes.offered !== outcomes.admitted || outcomes.completed_useful + outcomes.coalesced !== outcomes.admitted) {
        throw new Error("coalesce overload accounting is not conservative");
      }
    } else if (pressureId === "sample") {
      if (outcomes.offered !== outcomes.admitted + outcomes.sampled + outcomes.dropped
          || outcomes.completed_useful !== outcomes.admitted) {
        throw new Error("sample overload accounting is not conservative");
      }
    } else if (pressureId === "drop-disposable") {
      if (outcomes.offered !== outcomes.admitted + outcomes.dropped || outcomes.completed_useful !== outcomes.admitted) {
        throw new Error("disposable-drop overload accounting is not conservative");
      }
    } else if (["disconnect", "fail"].includes(pressureId)) {
      if (outcomes.offered !== outcomes.admitted + 1 || outcomes.completed_useful > outcomes.admitted) {
        throw new Error(`${pressureId} overload accounting is not conservative`);
      }
    } else {
      throw new Error(`unknown overload pressure policy ${pressureId}`);
    }
    if (["drain", "abort"].includes(termination)) {
      // Cancellation terminal timing is checked above; it has no recovery region.
    } else if (sample.workload.consumer_pattern === "bursty") {
      // Repeated exact cycles are checked above and do not claim one contiguous phase duration.
    } else if (["disconnect", "fail"].includes(pressureId)) {
      if (sample.phases.recovery_ns !== null) throw new Error("terminal overload unexpectedly entered recovery");
    } else if (sample.phases.pressure_ns === null || sample.phases.recovery_ns === null) {
      throw new Error("finite overload fixture did not expose both pressure and recovery regions");
    }
  } else if (fanout) {
    const outcomes = sample.outcomes;
    const cordCount = sample.workload.fanout_branches + Number(sample.workload.fanout_mode === "isolated");
    const maximumItems = sample.workload.queue_capacity_items * cordCount;
    if (!["coupled", "isolated"].includes(sample.workload.fanout_mode) || ![2, 8, 32].includes(sample.workload.fanout_branches)) {
      throw new Error("fan-out identity does not name the current publication matrix");
    }
    if (!["one", "all"].includes(sample.workload.slow_branches)) {
      throw new Error("fan-out slow-branch mode is invalid");
    }
    if (sample.memory.queue_items_high_water > maximumItems) {
      throw new Error("aggregate fan-out queues exceeded declared cord capacities");
    }
    if (sample.memory.queue_max_cord_items_high_water > sample.workload.queue_capacity_items) {
      throw new Error("a fan-out cord exceeded its declared item capacity");
    }
    if (outcomes.offered !== sample.workload.input_values
        || outcomes.admitted !== outcomes.offered
        || outcomes.completed_useful !== outcomes.admitted * sample.workload.fanout_branches
        || outcomes.retried < 1
        || [outcomes.rejected, outcomes.sampled, outcomes.coalesced, outcomes.dropped].some((value) => value !== 0)) {
      throw new Error("fan-out accounting is not lossless and conservative");
    }
    if (sample.workload.consumer_pattern !== "bursty"
        && (sample.phases.pressure_ns === null || sample.phases.recovery_ns === null)) {
      throw new Error("finite fan-out fixture did not expose both pressure and recovery regions");
    }
  } else if (sharedPayload) {
    const branches = sample.workload.fanout_branches;
    const payloadBytes = sample.workload.payload_bytes;
    const aborted = sample.workload.termination_request === "abort";
    const expectedDeliveries = aborted ? 0 : branches;
    const expectedVerifierBytes = aborted ? 0 : branches * payloadBytes;
    const watchSlots = sample.workload.watch_slots;
    const watchPreviewBytes = sample.workload.watch_preview_bytes;
    const expectedWatchPreviewBytes = watchSlots * watchPreviewBytes;
    if (![2, 8, 32].includes(branches)
        || ![1024, 1048576].includes(payloadBytes)
        || sample.workload.payload_representation !== "hosted-generation-safe-shared-text-handle"
        || sample.workload.fanout_mode !== "coupled"
        || sample.workload.queue_capacity_items !== 1
        || sample.workload.input_values !== 1
        || sample.workload.slow_consumer_yields !== 0
        || ![0, 1, branches].includes(watchSlots)
        || watchPreviewBytes !== (watchSlots === 0 ? 0 : 64)
        || sample.workload.watch_retention !== (watchSlots === 0 ? "none" : "latest")
        || !["complete", "abort"].includes(sample.workload.termination_request)
        || sample.workload.cancel_after_offers !== (aborted ? 1 : 0)
        || sample.outcomes.offered !== 1
        || sample.outcomes.admitted !== 1
        || sample.outcomes.completed_useful !== expectedDeliveries
        || sample.outcomes.cancelled !== (aborted ? 1 : 0)
        || sample.execution.unique_value_handles !== 1
        || sample.execution.branch_deliveries !== expectedDeliveries
        || sample.execution.pressured_items_at_stop !== (aborted ? branches : null)
        || (aborted ? sample.execution.abort_ns === null : sample.execution.abort_ns !== null)
        || sample.allocations.calls !== 0
        || sample.allocations.bytes !== 0
        || sample.memory.value_resident_slots_after_terminal !== 0
        || sample.memory.value_resident_bytes_after_terminal !== 0
        || sample.memory.value_slots_high_water !== 1
        || sample.memory.value_bytes_high_water !== payloadBytes
        || sample.memory.value_slots_capacity < 1
        || sample.memory.value_bytes_capacity < payloadBytes
        || sample.memory.host_io_output_bytes !== expectedVerifierBytes
        || sample.memory.host_io_capacity_bytes < sample.memory.host_io_output_bytes
        || sample.memory.queue_max_cord_items_high_water > 1
        || sample.memory.watch_admitted_slots !== watchSlots
        || sample.memory.watch_attached_slots !== watchSlots
        || sample.memory.watch_retained_observations !== watchSlots
        || sample.memory.watch_retained_preview_bytes !== expectedWatchPreviewBytes
        || sample.memory.watch_dropped_observations !== 0
        || sample.memory.watch_maximum_observations !== watchSlots
        || sample.memory.watch_maximum_preview_bytes !== expectedWatchPreviewBytes) {
      throw new Error("shared-payload handle, residency, delivery, verifier, or Watch accounting changed");
    }
  } else {
    if (sample.outcomes.offered !== sample.workload.input_values) throw new Error("offered input count changed");
    if (sample.outcomes.admitted !== sample.workload.input_values) throw new Error("admitted input count changed");
    if (sample.outcomes.completed_useful !== expectedUseful(sample)) throw new Error("useful output count changed");
    if (["rejected", "sampled", "coalesced", "dropped"].some((field) => sample.outcomes[field] !== 0)) {
      throw new Error("lossless local-depth fixture reported value loss");
    }
  }
  if (sample.latency.samples_ns.length === 0 || sample.latency.samples_ns.some((value) => value <= 0)) {
    throw new Error("latency samples must be present and positive");
  }
  if (["conduit-reference-scheduler", "conduit-hosted-value-arena"].includes(sample.runtime.id)
      && sample.allocations.calls !== 0) {
    throw new Error("Conduit allocated after Start");
  }
}

function percentile(values, probability) {
  const sorted = [...values].sort((left, right) => left - right);
  if (sorted.length === 1) return sorted[0];
  const at = probability * (sorted.length - 1);
  const lower = Math.floor(at);
  const upper = Math.ceil(at);
  const weight = at - lower;
  return sorted[lower] + ((sorted[upper] - sorted[lower]) * weight);
}

function median(values) {
  return percentile(values, 0.5);
}

function optionalStats(values) {
  const available = values.filter((value) => value !== null && value !== undefined);
  return available.length === 0 ? null : {
    samples: available.length,
    median: median(available),
    min: Math.min(...available),
    max: Math.max(...available),
  };
}

function bootstrapMedian95(values, seed, count) {
  let state = BigInt(seed);
  const next = () => {
    state = (state * 6364136223846793005n + 1442695040888963407n) & ((1n << 64n) - 1n);
    return Number(state >> 11n) / 9007199254740992;
  };
  const medians = [];
  for (let repeat = 0; repeat < count; repeat += 1) {
    const resample = [];
    for (let index = 0; index < values.length; index += 1) {
      resample.push(values[Math.min(values.length - 1, Math.floor(next() * values.length))]);
    }
    medians.push(median(resample));
  }
  return { low: percentile(medians, 0.025), high: percentile(medians, 0.975) };
}

const measured = samples.filter((sample) => sample.sample_kind === "measured");
const groups = new Map();
for (const sample of measured) {
  const key = [sample.runtime.id, sample.workload.id, sample.workload.operators,
    sample.workload.queue_capacity_items, sample.workload.pressure,
    sample.workload.slow_consumer_yields, sample.workload.fanout_branches,
    sample.workload.fanout_mode, sample.workload.slow_branches,
    sample.workload.termination_request, sample.workload.cancel_after_offers,
    sample.workload.consumer_pattern, sample.workload.consumer_burst_items,
    sample.workload.session_mode, sample.workload.session_pump_quantum,
    sample.workload.residency_plateau_after_wakes, sample.workload.payload_bytes,
    sample.workload.payload_representation, sample.workload.watch_slots,
    sample.workload.watch_preview_bytes, sample.workload.watch_retention].join("/");
  if (!groups.has(key)) groups.set(key, []);
  groups.get(key).push(sample);
}

const summaries = [];
for (const [key, group] of [...groups].sort(([left], [right]) => left.localeCompare(right))) {
  if (group.length < 9) throw new Error(`${key} has ${group.length} measured trials; at least 9 are required`);
  const throughputs = group.map((sample) => sample.outcomes.completed_useful / (sample.phases.steady_ns / 1e9));
  const steady = group.map((sample) => sample.phases.steady_ns);
  const latency = group.flatMap((sample) => sample.latency.samples_ns);
  const sample = group[0];
  summaries.push({
    runtime: sample.runtime,
    workload: sample.workload,
    measured_trials: group.length,
    useful_outputs_per_second: {
      median: median(throughputs),
      median_confidence_95: bootstrapMedian95(throughputs, 241243 + summaries.length, 10000),
    },
    phases_ns: {
      assembly: optionalStats(group.map((value) => value.phases.assembly_ns)),
      plan_seal: optionalStats(group.map((value) => value.phases.plan_seal_ns)),
      start: optionalStats(group.map((value) => value.phases.start_ns)),
      steady: optionalStats(steady),
      pressure: optionalStats(group.map((value) => value.phases.pressure_ns)),
      recovery: optionalStats(group.map((value) => value.phases.recovery_ns)),
      pressure_cycles: optionalStats(group.map((value) => value.phases.pressure_cycles)),
      recovery_cycles: optionalStats(group.map((value) => value.phases.recovery_cycles)),
    },
    process_cpu_ns: optionalStats(group.map((value) => value.process_cpu_ns)),
    execution: {
      scheduler_decisions: optionalStats(group.map((value) => value.execution.scheduler_decisions)),
      producer_stall_ns: optionalStats(group.map((value) => value.execution.producer_stall_ns)),
      drain_ns: optionalStats(group.map((value) => value.execution.drain_ns)),
      abort_ns: optionalStats(group.map((value) => value.execution.abort_ns)),
      session_pumps: optionalStats(group.map((value) => value.execution.session_pumps)),
      session_reserved_bytes: optionalStats(group.map((value) => value.execution.session_reserved_bytes)),
      pressured_items_at_stop: optionalStats(group.map((value) => value.execution.pressured_items_at_stop)),
      session_host_wakes: optionalStats(group.map((value) => value.execution.session_host_wakes)),
      session_timer_wakes: optionalStats(group.map((value) => value.execution.session_timer_wakes)),
      residency_plateau_verified: group.some((value) => value.execution.residency_plateau_verified !== null)
        ? group.every((value) => value.execution.residency_plateau_verified === true)
        : null,
      residency_checkpoint_queue_items_high_water: optionalStats(group.map((value) => value.execution.residency_checkpoint_queue_items_high_water)),
      residency_checkpoint_queue_payload_bytes_high_water: optionalStats(group.map((value) => value.execution.residency_checkpoint_queue_payload_bytes_high_water)),
      residency_checkpoint_ready_slots_high_water: optionalStats(group.map((value) => value.execution.residency_checkpoint_ready_slots_high_water)),
      residency_checkpoint_evidence_slots_high_water: optionalStats(group.map((value) => value.execution.residency_checkpoint_evidence_slots_high_water)),
      unique_value_handles: optionalStats(group.map((value) => value.execution.unique_value_handles)),
      branch_deliveries: optionalStats(group.map((value) => value.execution.branch_deliveries)),
    },
    outcomes: Object.fromEntries([
      "offered", "admitted", "completed_useful", "rejected", "sampled", "coalesced", "dropped", "cancelled", "retried", "terminal",
    ].map((field) => [field, optionalStats(group.map((value) => value.outcomes[field]))])),
    allocations_after_start: {
      calls: optionalStats(group.map((value) => value.allocations.calls)),
      bytes: optionalStats(group.map((value) => value.allocations.bytes)),
    },
    resident_bytes: {
      before: optionalStats(group.map((value) => value.memory.resident_before_bytes)),
      after: optionalStats(group.map((value) => value.memory.resident_after_bytes)),
      peak: optionalStats(group.map((value) => value.memory.resident_peak_bytes)),
    },
    memory_accounting: {
      planned_bytes: optionalStats(group.map((value) => value.memory.planned_memory_bytes)),
      executor_overhead_bytes: optionalStats(group.map((value) => value.memory.executor_overhead_bytes)),
    },
    high_water: {
      queue_items: optionalStats(group.map((value) => value.memory.queue_items_high_water)),
      queue_max_cord_items: optionalStats(group.map((value) => value.memory.queue_max_cord_items_high_water)),
      queue_payload_bytes: optionalStats(group.map((value) => value.memory.queue_payload_bytes_high_water)),
      ready_slots: optionalStats(group.map((value) => value.memory.ready_slots_high_water)),
      evidence_slots: optionalStats(group.map((value) => value.memory.evidence_slots_high_water)),
      value_resident_slots_after_terminal: optionalStats(group.map((value) => value.memory.value_resident_slots_after_terminal)),
      value_resident_bytes_after_terminal: optionalStats(group.map((value) => value.memory.value_resident_bytes_after_terminal)),
      value_slots: optionalStats(group.map((value) => value.memory.value_slots_high_water)),
      value_bytes: optionalStats(group.map((value) => value.memory.value_bytes_high_water)),
      value_slots_capacity: optionalStats(group.map((value) => value.memory.value_slots_capacity)),
      value_bytes_capacity: optionalStats(group.map((value) => value.memory.value_bytes_capacity)),
      host_io_capacity_bytes: optionalStats(group.map((value) => value.memory.host_io_capacity_bytes)),
      host_io_output_bytes: optionalStats(group.map((value) => value.memory.host_io_output_bytes)),
      watch_admitted_slots: optionalStats(group.map((value) => value.memory.watch_admitted_slots)),
      watch_attached_slots: optionalStats(group.map((value) => value.memory.watch_attached_slots)),
      watch_retained_observations: optionalStats(group.map((value) => value.memory.watch_retained_observations)),
      watch_retained_preview_bytes: optionalStats(group.map((value) => value.memory.watch_retained_preview_bytes)),
      watch_dropped_observations: optionalStats(group.map((value) => value.memory.watch_dropped_observations)),
      watch_maximum_observations: optionalStats(group.map((value) => value.memory.watch_maximum_observations)),
      watch_maximum_preview_bytes: optionalStats(group.map((value) => value.memory.watch_maximum_preview_bytes)),
    },
    latency_ns: {
      samples: latency.length,
      p50: percentile(latency, 0.5),
      p95: percentile(latency, 0.95),
      p99: percentile(latency, 0.99),
      p99_9: percentile(latency, 0.999),
      max: Math.max(...latency),
    },
  });
}
summaries.sort((left, right) =>
  left.workload.id.localeCompare(right.workload.id)
  || left.runtime.id.localeCompare(right.runtime.id)
  || left.workload.operators - right.workload.operators
  || left.workload.queue_capacity_items - right.workload.queue_capacity_items
  || left.workload.pressure.localeCompare(right.workload.pressure)
  || left.workload.session_mode.localeCompare(right.workload.session_mode)
  || left.workload.consumer_pattern.localeCompare(right.workload.consumer_pattern)
  || left.workload.fanout_branches - right.workload.fanout_branches
  || left.workload.watch_slots - right.workload.watch_slots
  || left.workload.slow_branches.localeCompare(right.workload.slow_branches)
);

const result = {
  schema: "conduit.comparative-benchmark-summary",
  schema_version: 0,
  fixture_revision: 0,
  policy: {
    deterministic_invariants: "strict",
    wall_clock: "report-only unless a separate reviewed regression policy exactly matches the recorded machine class, architecture, input cardinality, and trial counts",
    confidence_interval: "deterministic percentile bootstrap of the per-trial median; 10000 resamples",
    claim_boundary: "measurements are not deadline, admission, safety, or portability guarantees",
  },
  samples: { total: samples.length, warmup: samples.length - measured.length, measured: measured.length },
  unavailable: [
    {
      runtime: "conduit-reference-scheduler",
      workload: "bounded-async",
      reason: "the reference runner is single-lane; a bounded cord is not relabelled as an asynchronous execution boundary",
    },
    {
      runtime: "rxjs",
      workload: "bounded-async",
      reason: "RxJS has no demand-bounded asynchronous boundary; an uncontrolled observeOn queue is not substituted",
    },
    {
      runtime: "conduit-optimized-hosted-streaming",
      workload: "all",
      reason: "unavailable pending #214/#242; reference-scheduler results are not substituted",
    },
    {
      runtime: "rxjs",
      workload: "overload",
      reason: "no demand-bounded queue matches the exact Conduit pressure policies; synchronous push is not substituted",
    },
    {
      runtime: "reactor-core",
      workload: "overload",
      reason: "a reviewed demand/buffer and loss-policy mapping is not yet implemented; local-depth publishOn is not substituted",
    },
    {
      runtime: "rxjs/reactor-core",
      workload: "fanout",
      reason: "reviewed coupled and isolated semantic mappings are not implemented; ordinary multicast is not substituted",
    },
    {
      runtime: "rxjs/reactor-core",
      workload: "shared-payload-fanout",
      reason: "no reviewed generation-safe shared-handle and residency mapping exists; language object references are not substituted",
    },
    {
      runtime: "conduit-hosted-value-arena",
      workload: "shared-payload-fanout payloads above 1 MiB or non-text media",
      reason: "the current production hosted literal binding is bounded to 1 MiB public text and Watch coverage is limited to exact pre-Start Latest previews; larger, PCM, image, encoded, fragment, browser, other Watch retention/lifecycle modes, coalesce, and slot-reuse slices remain unavailable",
    },
    {
      runtime: "rxjs/reactor-core",
      workload: "persistent-wake",
      reason: "no reviewed mapping for Conduit's exact named host-operation wait and production session reservation exists; a timer or subject is not substituted",
    },
    {
      runtime: "rxjs/reactor-core",
      workload: "persistent-timer",
      reason: "no reviewed mapping for Conduit's exact retained timer deadline and production session reservation exists; an interval operator is not substituted",
    },
  ],
  groups: summaries,
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(result, null, 2)}\n`, { flag: "wx" });

if (reportOutput) {
  const integer = new Intl.NumberFormat("en-US", { maximumFractionDigits: 0 });
  const duration = (value) => value === null ? "unavailable" : integer.format(value.median);
  const report = [
    "# Comparative benchmark report",
    "",
    "Wall-clock results are measurements for the exact metadata artifact. A separate regression evaluation may apply reviewed broad alarms only when its machine class, architecture, input cardinality, and trial counts match exactly. Neither a result nor an alarm is a semantic, deadline, admission, safety, or portability guarantee.",
    "",
    "## Unavailable comparisons",
    "",
    "- Conduit reference bounded-async: the runner is single-lane; a bounded cord is not relabelled as an asynchronous boundary.",
    "- RxJS bounded-async: no demand-bounded boundary exists; an uncontrolled `observeOn` queue is not substituted.",
    "- Conduit optimized hosted streaming: unavailable pending #214/#242; reference results are not substituted.",
    "- RxJS overload: synchronous push has no demand-bounded queue matching these pressure policies and is not substituted.",
    "- Reactor overload: a reviewed demand/buffer and loss-policy mapping is not yet implemented; `publishOn` is not substituted.",
    "- RxJS/Reactor fan-out: reviewed coupled and isolated semantic mappings are not implemented; ordinary multicast is not substituted.",
    "- RxJS/Reactor shared payloads: no reviewed generation-safe shared-handle and residency mapping exists; language object references are not substituted.",
    "- Conduit shared payloads beyond 1 MiB public text: larger, PCM, image, encoded, fragment, browser, Watch retention/lifecycle modes beyond exact pre-Start Latest previews, coalesce, and slot-reuse slices remain unavailable.",
    "- RxJS/Reactor persistent wake: no reviewed mapping exists for Conduit's exact named host-operation wait and production session reservation; a timer or subject is not substituted.",
    "- RxJS/Reactor persistent timer: no reviewed mapping exists for Conduit's exact retained timer deadline and production session reservation; an interval operator is not substituted.",
    "- Drain/Abort timing is present only on fixtures that explicitly request that transition; normal completion remains distinct and null.",
    "",
  ];
  for (const workload of [...new Set(summaries.map((group) => group.workload.id))].sort()) {
    const workloadGroups = summaries.filter((group) => group.workload.id === workload && group.runtime.comparison_role === "reactive-runtime");
    report.push(`## ${workload}: preparation regions`, "", "| Runtime | Policy | Session | Consumer pattern | Capacity | Fan-out | Slow branches | Depth | Assembly median ns | Plan seal median ns | Start median ns |", "| --- | --- | --- | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: |");
    for (const group of workloadGroups) {
      report.push(`| ${group.runtime.id} | ${group.workload.pressure} | ${group.workload.session_mode} | ${group.workload.consumer_pattern} | ${group.workload.queue_capacity_items} | ${group.workload.fanout_branches} | ${group.workload.slow_branches} | ${group.workload.operators} | ${duration(group.phases_ns.assembly)} | ${duration(group.phases_ns.plan_seal)} | ${duration(group.phases_ns.start)} |`);
    }
    report.push("", `## ${workload}: steady region`, "", "| Runtime | Policy | Session | Consumer pattern | Capacity | Fan-out | Slow branches | Depth | Useful outputs/s median | 95% CI | p50 ns | p95 ns | p99 ns | p99.9 ns | max ns | Producer stall median ns | Scheduler decisions median | Session pumps | Pressured at stop | Recovery median ns | Pressure cycles | Recovery cycles |", "| --- | --- | --- | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
    for (const group of workloadGroups) {
      const throughput = group.useful_outputs_per_second;
      report.push(`| ${group.runtime.id} | ${group.workload.pressure} | ${group.workload.session_mode} | ${group.workload.consumer_pattern} | ${group.workload.queue_capacity_items} | ${group.workload.fanout_branches} | ${group.workload.slow_branches} | ${group.workload.operators} | ${integer.format(throughput.median)} | ${integer.format(throughput.median_confidence_95.low)}–${integer.format(throughput.median_confidence_95.high)} | ${integer.format(group.latency_ns.p50)} | ${integer.format(group.latency_ns.p95)} | ${integer.format(group.latency_ns.p99)} | ${integer.format(group.latency_ns.p99_9)} | ${integer.format(group.latency_ns.max)} | ${duration(group.execution.producer_stall_ns)} | ${duration(group.execution.scheduler_decisions)} | ${duration(group.execution.session_pumps)} | ${duration(group.execution.pressured_items_at_stop)} | ${duration(group.phases_ns.recovery)} | ${duration(group.phases_ns.pressure_cycles)} | ${duration(group.phases_ns.recovery_cycles)} |`);
    }
    report.push("");
    if (workload === "overload") {
      report.push("## overload: outcome accounting", "", "Counts are per-trial medians. Completed-useful is the throughput numerator; admitted, replaced, aborted, cancelled-before-admission, or discarded work is never counted as success.", "", "| Runtime | Policy | Session | Consumer pattern | Stop | Capacity | Offered | Admitted | Useful | Rejected | Sampled | Coalesced | Dropped | Cancelled | Retried | Terminal | Stop median ns | Queue high water |", "| --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
      for (const group of workloadGroups) {
        const outcome = (field) => integer.format(group.outcomes[field].median);
        const stopTime = group.workload.termination_request === "drain" ? group.execution.drain_ns : group.execution.abort_ns;
        report.push(`| ${group.runtime.id} | ${group.workload.pressure} | ${group.workload.session_mode} | ${group.workload.consumer_pattern} | ${group.workload.termination_request} | ${group.workload.queue_capacity_items} | ${outcome("offered")} | ${outcome("admitted")} | ${outcome("completed_useful")} | ${outcome("rejected")} | ${outcome("sampled")} | ${outcome("coalesced")} | ${outcome("dropped")} | ${outcome("cancelled")} | ${outcome("retried")} | ${outcome("terminal")} | ${duration(stopTime)} | ${duration(group.high_water.queue_items)} |`);
      }
      report.push("");
    }
    if (workload === "fanout") {
      report.push("## fanout: outcome accounting", "", "Completed-useful counts branch deliveries. Coupled mode publishes all branches atomically; isolated mode uses an ordinary duplicator with one profile-accounted retained input and independent finite branch transactions.", "", "| Runtime | Mode | Consumer pattern | Capacity | Branches | Slow branches | Offered | Admitted inputs | Useful branch deliveries | Retried | Terminal | Aggregate queue high water | Max cord high water |", "| --- | --- | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
      for (const group of workloadGroups) {
        const outcome = (field) => integer.format(group.outcomes[field].median);
        report.push(`| ${group.runtime.id} | ${group.workload.fanout_mode} | ${group.workload.consumer_pattern} | ${group.workload.queue_capacity_items} | ${group.workload.fanout_branches} | ${group.workload.slow_branches} | ${outcome("offered")} | ${outcome("admitted")} | ${outcome("completed_useful")} | ${outcome("retried")} | ${outcome("terminal")} | ${duration(group.high_water.queue_items)} | ${duration(group.high_water.queue_max_cord_items)} |`);
      }
      report.push("");
    }
    if (workload === "shared-payload-fanout") {
      report.push("## shared-payload-fanout: handle, Watch, and residency accounting", "", "The value arena retains one generation-safe topology-sized handle across every branch for both the 1 KiB and 1 MiB cases. Queue byte charges, content-verifying display buffers, and fixed Watch preview copies are separate bounded storage and are not described as zero-copy. Watch reads verify the copied 64-byte prefix and full content hash after the timed region. Abort rows cancel after atomic publication and before verifier consumption; retained Watch previews must not extend terminal executor-value residency.", "", "| Runtime | Terminal request | Payload bytes | Branches | Watch slots | Watch preview bytes retained | Planned bytes | Executor overhead bytes | Unique handles | Branch deliveries | Value slots high water | Value bytes high water | Terminal value slots | Terminal value bytes | Queue payload bytes | Host verifier output bytes | Alloc calls after Start |", "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
      for (const group of workloadGroups) {
        report.push(`| ${group.runtime.id} | ${group.workload.termination_request} | ${group.workload.payload_bytes} | ${group.workload.fanout_branches} | ${group.workload.watch_slots} | ${duration(group.high_water.watch_retained_preview_bytes)} | ${duration(group.memory_accounting.planned_bytes)} | ${duration(group.memory_accounting.executor_overhead_bytes)} | ${duration(group.execution.unique_value_handles)} | ${duration(group.execution.branch_deliveries)} | ${duration(group.high_water.value_slots)} | ${duration(group.high_water.value_bytes)} | ${duration(group.high_water.value_resident_slots_after_terminal)} | ${duration(group.high_water.value_resident_bytes_after_terminal)} | ${duration(group.high_water.queue_payload_bytes)} | ${duration(group.high_water.host_io_output_bytes)} | ${duration(group.allocations_after_start.calls)} |`);
      }
      report.push("");
    }
    if (["persistent-wake", "persistent-timer"].includes(workload)) {
      const wakeMetric = workload === "persistent-wake"
        ? "session_host_wakes"
        : "session_timer_wakes";
      report.push(
        `## ${workload}: residency plateau`,
        "",
        "Checkpoint and final high-water values must match exactly in every raw row. Process RSS is supplementary and is not used for this proof.",
        "",
        "| Runtime | Host wakes | Checkpoint after wakes | Plateau verified | Alloc calls after Start | Alloc bytes after Start | Checkpoint/final queue items | Checkpoint/final payload bytes | Checkpoint/final ready slots | Checkpoint/final evidence slots | Reserved session bytes | Drain median ns |",
        "| --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
      );
      for (const group of workloadGroups) {
        report.push(`| ${group.runtime.id} | ${duration(group.execution[wakeMetric])} | ${group.workload.residency_plateau_after_wakes} | ${group.execution.residency_plateau_verified} | ${duration(group.allocations_after_start.calls)} | ${duration(group.allocations_after_start.bytes)} | ${duration(group.execution.residency_checkpoint_queue_items_high_water)}/${duration(group.high_water.queue_items)} | ${duration(group.execution.residency_checkpoint_queue_payload_bytes_high_water)}/${duration(group.high_water.queue_payload_bytes)} | ${duration(group.execution.residency_checkpoint_ready_slots_high_water)}/${duration(group.high_water.ready_slots)} | ${duration(group.execution.residency_checkpoint_evidence_slots_high_water)}/${duration(group.high_water.evidence_slots)} | ${duration(group.execution.session_reserved_bytes)} | ${duration(group.execution.drain_ns)} |`);
      }
      report.push("");
    }
    const lowerBounds = summaries.filter((group) => group.workload.id === workload && group.runtime.comparison_role === "language-lower-bound");
    if (lowerBounds.length > 0) {
      report.push(`## ${workload}: language lower bounds`, "", "These loops have no reactive runtime, subscription, demand, scheduler, queues, evidence, or merge boundary. They are language-cost references, never competitors.", "", "| Language loop | Depth | Useful outputs/s median | 95% CI | p99 ns |", "| --- | ---: | ---: | ---: | ---: |");
      for (const group of lowerBounds) {
        const throughput = group.useful_outputs_per_second;
        report.push(`| ${group.runtime.id} | ${group.workload.operators} | ${integer.format(throughput.median)} | ${integer.format(throughput.median_confidence_95.low)}–${integer.format(throughput.median_confidence_95.high)} | ${integer.format(group.latency_ns.p99)} |`);
      }
      report.push("");
    }
  }
  fs.mkdirSync(path.dirname(reportOutput), { recursive: true });
  fs.writeFileSync(reportOutput, `${report.join("\n")}\n`, { flag: "wx" });
}
