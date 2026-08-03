import process from "node:process";
import { filter, map, merge, range, tap } from "rxjs";

function argument(name, fallback) {
  const at = process.argv.indexOf(`--${name}`);
  return at === -1 ? fallback : process.argv[at + 1];
}

const workload = argument("workload", "map");
const operators = Number.parseInt(argument("operators", "1"), 10);
const values = Number.parseInt(argument("values", "1000000"), 10);
const queueItems = Number.parseInt(argument("queue-items", "256"), 10);
const stride = Number.parseInt(argument("latency-sample-stride", "1024"), 10);
const warmupTrials = Number.parseInt(argument("warmup-trials", "2"), 10);
const measuredTrials = Number.parseInt(argument("measured-trials", "9"), 10);
const identityLoop = process.argv.includes("--identity-loop");
if ([operators, values, queueItems, stride, warmupTrials, measuredTrials].some((value) => !Number.isSafeInteger(value) || value < 1)) {
  throw new Error("operators, values, queue items, stride, warmups, and measured trials must be positive integers");
}
if (workload === "bounded-async") {
  throw new Error("RxJS has no bounded-demand asynchronous boundary; this case is intentionally unavailable");
}

const monotonic = () => process.hrtime.bigint();
const logicalFixture = `comparative-local-depth/${workload}/${operators}/${values}/${queueItems}/${stride}`;

function runIdentitySample(sampleKind, trial, thermalState) {
  if (workload === "bounded-async") throw new Error("an identity loop cannot model a bounded asynchronous boundary");
  const assemblyStarted = monotonic();
  const starts = new Array(Math.ceil(values / stride));
  const latencies = [];
  const transformCount = workload === "merge" ? Math.max(0, operators - 1) : operators;
  const assemblyNs = Number(monotonic() - assemblyStarted);
  const memoryBefore = process.memoryUsage();
  const cpuBefore = process.cpuUsage();
  let acceptedValues = 0;
  let usefulOutputs = 0;
  const steadyStarted = monotonic();
  for (let original = 0; original < values; original += 1) {
    acceptedValues += 1;
    if (original % stride === 0) starts[Math.floor(original / stride)] = monotonic();
    let value = original;
    let retained = true;
    for (let index = 0; index < transformCount; index += 1) {
      if (workload === "map-filter" && index % 2 === 1) {
        retained = value % 2 === 0;
        if (!retained) break;
      } else {
        value += 2;
      }
    }
    if (retained) {
      usefulOutputs += 1;
      if (original % stride === 0) latencies.push(Number(monotonic() - starts[Math.floor(original / stride)]));
      if (value < 0) throw new Error("unreachable identity result");
    }
  }
  const steadyNs = Number(monotonic() - steadyStarted);
  const cpu = process.cpuUsage(cpuBefore);
  const memoryAfter = process.memoryUsage();
  return {
    schema: "conduit.comparative-raw-sample",
    schema_version: 0,
    fixture_revision: 0,
    runtime: {
      id: "javascript-identity-loop",
      comparison_role: "language-lower-bound",
      version: process.version,
      execution_mode: "single-threaded-for-loop",
      build_profile: "node-default",
      scheduler: "none",
      fusion: "not-applicable",
      batching: "none",
      concurrency: 1,
    },
    workload: {
      id: workload,
      operators,
      input_values: values,
      queue_capacity_items: 0,
      ordering: "ascending input order; merge boundary omitted",
      pressure: "not-applicable",
      terminal: "loop exhaustion",
      loss: "none",
      slow_consumer_yields: 0,
      recovery_after_outputs: 0,
      fanout_branches: 1,
      fanout_mode: "none",
      slow_branches: "none",
    },
    exact_identity: {
      logical_fixture: logicalFixture,
      plan_identity: null,
      source_semantic_hash: null,
      artifact_digest: null,
    },
    sample_kind: sampleKind,
    trial,
    thermal_state: thermalState,
    phases: { assembly_ns: assemblyNs, plan_seal_ns: null, start_ns: null, steady_ns: steadyNs, pressure_ns: null, recovery_ns: null },
    process_cpu_ns: (cpu.user + cpu.system) * 1000,
    outcomes: {
      offered: values,
      admitted: acceptedValues,
      completed_useful: usefulOutputs,
      rejected: 0,
      sampled: 0,
      coalesced: 0,
      dropped: 0,
      retried: 0,
      terminal: 1,
    },
    allocations: { scope: "unavailable-from-node-public-api", calls: null, bytes: null },
    memory: {
      resident_before_bytes: memoryBefore.rss,
      resident_after_bytes: memoryAfter.rss,
      resident_peak_bytes: null,
      planned_memory_bytes: null,
      executor_overhead_bytes: null,
      queue_items_high_water: null,
      queue_max_cord_items_high_water: null,
      queue_payload_bytes_high_water: null,
      ready_slots_high_water: null,
      evidence_slots_high_water: null,
    },
    latency: { clock: "process.hrtime.bigint CLOCK_MONOTONIC", sample_stride: stride, samples_ns: latencies },
    semantic_notes: [
      "This no-framework JavaScript loop is a language-cost lower bound, not a reactive-runtime competitor.",
      "It has no subscription, scheduler, demand, queue, evidence, or merge boundary and cannot support runtime claims.",
    ],
  };
}

async function runSample(sampleKind, trial, thermalState) {
  const starts = new Array(Math.ceil(values / stride));
  const latencies = [];
  let usefulOutputs = 0;
  let acceptedValues = 0;

  function observedSource(start, count) {
    return range(start, count).pipe(tap((value) => {
      acceptedValues += 1;
      if (value % stride === 0) starts[Math.floor(value / stride)] = monotonic();
    }));
  }

  function transforms(count) {
    const result = [];
    for (let index = 0; index < count; index += 1) {
      if (workload === "map-filter" && index % 2 === 1) {
        result.push(filter((value) => value % 2 === 0));
      } else {
        result.push(map((value) => value + 2));
      }
    }
    return result;
  }

  const assemblyStarted = monotonic();
  let source;
  let transformCount = operators;
  if (workload === "merge") {
    const split = Math.floor(values / 2);
    source = merge(observedSource(0, split), observedSource(split, values - split));
    transformCount = Math.max(0, operators - 1);
  } else {
    source = observedSource(0, values);
  }
  const stream = source.pipe(...transforms(transformCount));
  const assemblyNs = Number(monotonic() - assemblyStarted);
  const memoryBefore = process.memoryUsage();
  const cpuBefore = process.cpuUsage();
  const steadyStarted = monotonic();

  await new Promise((resolve, reject) => {
    stream.subscribe({
      next(value) {
        usefulOutputs += 1;
        const original = value - (2 * transformCount);
        if (original >= 0 && original % stride === 0) {
          const started = starts[Math.floor(original / stride)];
          if (started !== undefined) latencies.push(Number(monotonic() - started));
        }
      },
      error: reject,
      complete: resolve,
    });
  });

  const steadyNs = Number(monotonic() - steadyStarted);
  const cpu = process.cpuUsage(cpuBefore);
  const memoryAfter = process.memoryUsage();
  const semanticNotes = [
    "Synchronous RxJS subscription and steady execution cannot be separated without changing the graph; start_ns is therefore unavailable.",
    "RxJS has no demand-bounded asynchronous case; fusion and batching remain at the pinned implementation default.",
  ];

  return {
    schema: "conduit.comparative-raw-sample",
    schema_version: 0,
    fixture_revision: 0,
    runtime: {
      id: "rxjs",
      comparison_role: "reactive-runtime",
      version: "7.8.2",
      execution_mode: "synchronous-push",
      build_profile: "node-default",
      scheduler: "current-call-stack",
      fusion: "implementation-default",
      batching: "one-value-notification",
      concurrency: 1,
    },
    workload: {
      id: workload,
      operators,
      input_values: values,
      queue_capacity_items: 0,
      ordering: "source order; merge serializes synchronous sources",
      pressure: "synchronous push; no demand contract",
      terminal: "complete after all notifications drain",
      loss: "none",
      slow_consumer_yields: 0,
      recovery_after_outputs: 0,
      fanout_branches: 1,
      fanout_mode: "none",
      slow_branches: "none",
    },
    exact_identity: {
      logical_fixture: logicalFixture,
      plan_identity: null,
      source_semantic_hash: null,
      artifact_digest: null,
    },
    sample_kind: sampleKind,
    trial,
    thermal_state: thermalState,
    phases: {
      assembly_ns: assemblyNs,
      plan_seal_ns: null,
      start_ns: null,
      steady_ns: steadyNs,
      pressure_ns: null,
      recovery_ns: null,
    },
    process_cpu_ns: (cpu.user + cpu.system) * 1000,
    outcomes: {
      offered: values,
      admitted: acceptedValues,
      completed_useful: usefulOutputs,
      rejected: 0,
      sampled: 0,
      coalesced: 0,
      dropped: 0,
      retried: 0,
      terminal: 1,
    },
    allocations: { scope: "unavailable-from-node-public-api", calls: null, bytes: null },
    memory: {
      resident_before_bytes: memoryBefore.rss,
      resident_after_bytes: memoryAfter.rss,
      resident_peak_bytes: null,
      planned_memory_bytes: null,
      executor_overhead_bytes: null,
      queue_items_high_water: null,
      queue_max_cord_items_high_water: null,
      queue_payload_bytes_high_water: null,
      ready_slots_high_water: null,
      evidence_slots_high_water: null,
    },
    latency: {
      clock: "process.hrtime.bigint CLOCK_MONOTONIC",
      sample_stride: stride,
      samples_ns: latencies,
    },
    semantic_notes: semanticNotes,
  };
}

for (let trial = 0; trial < warmupTrials; trial += 1) {
  const sample = identityLoop
    ? runIdentitySample("warmup", trial, trial === 0 ? "cold" : "warming")
    : await runSample("warmup", trial, trial === 0 ? "cold" : "warming");
  process.stdout.write(`${JSON.stringify(sample)}\n`);
}
for (let trial = 0; trial < measuredTrials; trial += 1) {
  const sample = identityLoop
    ? runIdentitySample("measured", trial, "warmed")
    : await runSample("measured", trial, "warmed");
  process.stdout.write(`${JSON.stringify(sample)}\n`);
}
