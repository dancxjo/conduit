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
  if (overload) {
    const pressureId = sample.workload.pressure.split("/")[0];
    if (identityParts.length !== 6
        || identityParts[0] !== "comparative-overload"
        || identityParts[1] !== pressureId
        || Number(identityParts[2]) !== sample.workload.input_values
        || Number(identityParts[3]) !== sample.workload.queue_capacity_items
        || Number(identityParts[4]) !== sample.workload.slow_consumer_yields
        || Number(identityParts[5]) !== sample.latency.sample_stride) {
      throw new Error("overload fixture identity does not match the raw sample");
    }
    if (sample.runtime.id !== "conduit-reference-scheduler") {
      throw new Error("the current overload slice has no cross-runtime substitute");
    }
  } else if (fanout) {
    if (identityParts.length !== 8
        || identityParts[0] !== "comparative-fanout"
        || identityParts[1] !== sample.workload.fanout_mode
        || identityParts[2] !== sample.workload.slow_branches
        || Number(identityParts[3]) !== sample.workload.fanout_branches
        || Number(identityParts[4]) !== sample.workload.input_values
        || Number(identityParts[5]) !== sample.workload.queue_capacity_items
        || Number(identityParts[6]) !== sample.workload.slow_consumer_yields
        || Number(identityParts[7]) !== sample.latency.sample_stride) {
      throw new Error("fan-out fixture identity does not match the raw sample");
    }
    if (sample.runtime.id !== "conduit-reference-scheduler") {
      throw new Error("the current fan-out slice has no cross-runtime substitute");
    }
  } else if (identityParts.length !== 6
      || identityParts[0] !== "comparative-local-depth"
      || identityParts[1] !== sample.workload.id
      || Number(identityParts[2]) !== sample.workload.operators
      || Number(identityParts[3]) !== sample.workload.input_values
      || Number(identityParts[5]) !== sample.latency.sample_stride) {
    throw new Error("local-depth fixture identity does not match the raw sample");
  }
  if (sample.runtime.id === "conduit-reference-scheduler"
      && [sample.exact_identity.plan_identity, sample.exact_identity.source_semantic_hash, sample.exact_identity.artifact_digest].some((value) => !value)) {
    throw new Error("Conduit sample omitted exact plan, source, or artifact identity");
  }
  if (sample.outcomes.terminal !== 1) throw new Error("fixture must report one terminal signal");
  if (overload) {
    const outcomes = sample.outcomes;
    const capacity = sample.workload.queue_capacity_items;
    if (sample.memory.queue_items_high_water > capacity) throw new Error("overload queue exceeded declared capacity");
    if (outcomes.offered > sample.workload.input_values) throw new Error("overload offered more than its finite source");
    const pressureId = sample.workload.pressure.split("/")[0];
    if (pressureId === "block") {
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
    if (["disconnect", "fail"].includes(pressureId)) {
      if (sample.phases.recovery_ns !== null) throw new Error("terminal overload unexpectedly entered recovery");
    } else if (sample.phases.pressure_ns === null || sample.phases.recovery_ns === null) {
      throw new Error("finite overload fixture did not expose both pressure and recovery regions");
    }
  } else if (fanout) {
    const outcomes = sample.outcomes;
    const maximumItems = sample.workload.queue_capacity_items * sample.workload.fanout_branches;
    if (sample.workload.fanout_mode !== "coupled" || ![2, 8, 32].includes(sample.workload.fanout_branches)) {
      throw new Error("fan-out identity does not name the current coupled matrix");
    }
    if (!["one", "all"].includes(sample.workload.slow_branches)) {
      throw new Error("fan-out slow-branch mode is invalid");
    }
    if (sample.memory.queue_items_high_water > maximumItems) {
      throw new Error("aggregate fan-out queues exceeded declared branch capacities");
    }
    if (outcomes.offered !== sample.workload.input_values
        || outcomes.admitted !== outcomes.offered
        || outcomes.completed_useful !== outcomes.admitted * sample.workload.fanout_branches
        || outcomes.retried < 1
        || [outcomes.rejected, outcomes.sampled, outcomes.coalesced, outcomes.dropped].some((value) => value !== 0)) {
      throw new Error("coupled fan-out accounting is not atomic and conservative");
    }
    if (sample.phases.pressure_ns === null || sample.phases.recovery_ns === null) {
      throw new Error("finite fan-out fixture did not expose both pressure and recovery regions");
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
  if (sample.runtime.id === "conduit-reference-scheduler" && sample.allocations.calls !== 0) {
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
    sample.workload.fanout_mode, sample.workload.slow_branches].join("/");
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
    },
    process_cpu_ns: optionalStats(group.map((value) => value.process_cpu_ns)),
    outcomes: Object.fromEntries([
      "offered", "admitted", "completed_useful", "rejected", "sampled", "coalesced", "dropped", "retried", "terminal",
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
    high_water: {
      queue_items: optionalStats(group.map((value) => value.memory.queue_items_high_water)),
      queue_payload_bytes: optionalStats(group.map((value) => value.memory.queue_payload_bytes_high_water)),
      ready_slots: optionalStats(group.map((value) => value.memory.ready_slots_high_water)),
      evidence_slots: optionalStats(group.map((value) => value.memory.evidence_slots_high_water)),
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
  || left.workload.fanout_branches - right.workload.fanout_branches
  || left.workload.slow_branches.localeCompare(right.workload.slow_branches)
);

const result = {
  schema: "conduit.comparative-benchmark-summary",
  schema_version: 0,
  fixture_revision: 0,
  policy: {
    deterministic_invariants: "strict",
    wall_clock: "report-only",
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
      runtime: "conduit-reference-scheduler",
      workload: "fanout/isolated",
      reason: "an ordinary duplicator with exact retained-value accounting is not implemented; coupled publication is not substituted",
    },
    {
      runtime: "rxjs/reactor-core",
      workload: "fanout",
      reason: "reviewed coupled and isolated semantic mappings are not implemented; ordinary multicast is not substituted",
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
    "Wall-clock results are report-only measurements for the exact metadata artifact. They are not semantic, deadline, admission, safety, or portability guarantees.",
    "",
    "## Unavailable comparisons",
    "",
    "- Conduit reference bounded-async: the runner is single-lane; a bounded cord is not relabelled as an asynchronous boundary.",
    "- RxJS bounded-async: no demand-bounded boundary exists; an uncontrolled `observeOn` queue is not substituted.",
    "- Conduit optimized hosted streaming: unavailable pending #214/#242; reference results are not substituted.",
    "- RxJS overload: synchronous push has no demand-bounded queue matching these pressure policies and is not substituted.",
    "- Reactor overload: a reviewed demand/buffer and loss-policy mapping is not yet implemented; `publishOn` is not substituted.",
    "- Conduit isolated fan-out: no ordinary duplicator with exact retained-value accounting is implemented; coupled publication is not substituted.",
    "- RxJS/Reactor fan-out: reviewed coupled and isolated semantic mappings are not implemented; ordinary multicast is not substituted.",
    "",
  ];
  for (const workload of [...new Set(summaries.map((group) => group.workload.id))].sort()) {
    const workloadGroups = summaries.filter((group) => group.workload.id === workload && group.runtime.comparison_role === "reactive-runtime");
    report.push(`## ${workload}: preparation regions`, "", "| Runtime | Policy | Capacity | Fan-out | Slow branches | Depth | Assembly median ns | Plan seal median ns | Start median ns |", "| --- | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: |");
    for (const group of workloadGroups) {
      report.push(`| ${group.runtime.id} | ${group.workload.pressure} | ${group.workload.queue_capacity_items} | ${group.workload.fanout_branches} | ${group.workload.slow_branches} | ${group.workload.operators} | ${duration(group.phases_ns.assembly)} | ${duration(group.phases_ns.plan_seal)} | ${duration(group.phases_ns.start)} |`);
    }
    report.push("", `## ${workload}: steady region`, "", "| Runtime | Policy | Capacity | Fan-out | Slow branches | Depth | Useful outputs/s median | 95% CI | p50 ns | p95 ns | p99 ns | p99.9 ns | max ns | Recovery median ns |", "| --- | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
    for (const group of workloadGroups) {
      const throughput = group.useful_outputs_per_second;
      report.push(`| ${group.runtime.id} | ${group.workload.pressure} | ${group.workload.queue_capacity_items} | ${group.workload.fanout_branches} | ${group.workload.slow_branches} | ${group.workload.operators} | ${integer.format(throughput.median)} | ${integer.format(throughput.median_confidence_95.low)}–${integer.format(throughput.median_confidence_95.high)} | ${integer.format(group.latency_ns.p50)} | ${integer.format(group.latency_ns.p95)} | ${integer.format(group.latency_ns.p99)} | ${integer.format(group.latency_ns.p99_9)} | ${integer.format(group.latency_ns.max)} | ${duration(group.phases_ns.recovery)} |`);
    }
    report.push("");
    if (workload === "overload") {
      report.push("## overload: outcome accounting", "", "Counts are per-trial medians. Completed-useful is the throughput numerator; admitted, replaced, or discarded work is never counted as success.", "", "| Runtime | Policy | Capacity | Offered | Admitted | Useful | Rejected | Sampled | Coalesced | Dropped | Retried | Terminal | Queue high water |", "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
      for (const group of workloadGroups) {
        const outcome = (field) => integer.format(group.outcomes[field].median);
        report.push(`| ${group.runtime.id} | ${group.workload.pressure} | ${group.workload.queue_capacity_items} | ${outcome("offered")} | ${outcome("admitted")} | ${outcome("completed_useful")} | ${outcome("rejected")} | ${outcome("sampled")} | ${outcome("coalesced")} | ${outcome("dropped")} | ${outcome("retried")} | ${outcome("terminal")} | ${duration(group.high_water.queue_items)} |`);
      }
      report.push("");
    }
    if (workload === "fanout") {
      report.push("## fanout: atomic outcome accounting", "", "Completed-useful counts branch deliveries. Coupled admission is one all-branches transaction; a retry publishes to no branch.", "", "| Runtime | Capacity | Branches | Slow branches | Offered | Admitted atomically | Useful branch deliveries | Retried | Terminal | Aggregate queue high water |", "| --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |");
      for (const group of workloadGroups) {
        const outcome = (field) => integer.format(group.outcomes[field].median);
        report.push(`| ${group.runtime.id} | ${group.workload.queue_capacity_items} | ${group.workload.fanout_branches} | ${group.workload.slow_branches} | ${outcome("offered")} | ${outcome("admitted")} | ${outcome("completed_useful")} | ${outcome("retried")} | ${outcome("terminal")} | ${duration(group.high_water.queue_items)} |`);
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
