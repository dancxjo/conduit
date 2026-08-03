import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const [metadataInput, summaryInput, policyInput, output, reportOutput] = process.argv.slice(2);
if (!metadataInput || !summaryInput || !policyInput || !output) {
  throw new Error("usage: node evaluate-regressions.mjs METADATA.json SUMMARY.json POLICY.json OUTPUT.json [REPORT.md]");
}

const readJson = (file) => JSON.parse(fs.readFileSync(file, "utf8"));
const metadata = readJson(metadataInput);
const summary = readJson(summaryInput);
const policy = readJson(policyInput);

if (metadata.schema !== "conduit.comparative-benchmark-metadata" || metadata.schema_version !== 0) {
  throw new Error("benchmark metadata does not use the current schema");
}
if (summary.schema !== "conduit.comparative-benchmark-summary" || summary.schema_version !== 0) {
  throw new Error("benchmark summary does not use the current schema");
}
if (policy.schema !== "conduit.comparative-regression-policy" || policy.schema_version !== 0 || policy.policy_revision !== 0) {
  throw new Error("regression policy does not use the current schema and revision");
}
if (!Array.isArray(policy.baselines) || policy.baselines.length === 0) {
  throw new Error("regression policy has no reviewed baseline entries");
}

const observedScope = {
  machine_class: metadata.execution_environment?.machine_class ?? null,
  architecture: metadata.machine,
  input_values: metadata.run.values,
  warmup_trials: metadata.run.warmup_trials,
  measured_trials: metadata.run.measured_trials,
};
const applicabilityReasons = Object.entries(policy.scope)
  .filter(([field, expected]) => observedScope[field] !== expected)
  .map(([field, expected]) => ({ field, expected, observed: observedScope[field] ?? null }));
const applicable = applicabilityReasons.length === 0;

const groupIdentity = (group) => ({
  runtime: group.runtime.id,
  workload: group.workload.id,
  operators: group.workload.operators,
  queue_capacity_items: group.workload.queue_capacity_items,
  pressure: group.workload.pressure,
  fanout_branches: group.workload.fanout_branches,
  fanout_mode: group.workload.fanout_mode,
  slow_branches: group.workload.slow_branches,
  consumer_pattern: group.workload.consumer_pattern,
  session_mode: group.workload.session_mode,
  termination_request: group.workload.termination_request,
});
const matches = (identity, expected) => Object.entries(expected)
  .every(([field, value]) => identity[field] === value);
const finitePositive = (value, label) => {
  if (!Number.isFinite(value) || value <= 0) throw new Error(`${label} must be finite and positive`);
  return value;
};

const checks = [];
if (applicable) {
  if (summary.samples.measured < policy.baselines.length * policy.thresholds.minimum_measured_trials) {
    throw new Error("summary cannot contain the minimum measured trials for every baseline entry");
  }
  for (const entry of policy.baselines) {
    const matching = summary.groups.filter((group) => matches(groupIdentity(group), entry.match));
    if (matching.length !== 1) {
      throw new Error(`baseline selector ${JSON.stringify(entry.match)} matched ${matching.length} groups`);
    }
    const group = matching[0];
    if (group.measured_trials < policy.thresholds.minimum_measured_trials) {
      throw new Error(`baseline selector ${JSON.stringify(entry.match)} has too few measured trials`);
    }
    const throughputBaseline = finitePositive(entry.baseline.useful_outputs_per_second_median, "baseline throughput");
    const p99Baseline = finitePositive(entry.baseline.p99_ns, "baseline p99");
    const p99_9Baseline = finitePositive(entry.baseline.p99_9_ns, "baseline p99.9");
    const throughputMedian = finitePositive(group.useful_outputs_per_second.median, "current throughput median");
    const throughputConfidenceHigh = finitePositive(group.useful_outputs_per_second.median_confidence_95.high, "current throughput confidence high");
    const p99 = finitePositive(group.latency_ns.p99, "current p99");
    const p99_9 = finitePositive(group.latency_ns.p99_9, "current p99.9");
    const ratios = {
      throughput_median: throughputMedian / throughputBaseline,
      throughput_confidence_high: throughputConfidenceHigh / throughputBaseline,
      p99: p99 / p99Baseline,
      p99_9: p99_9 / p99_9Baseline,
    };
    const alarmMetrics = policy.thresholds.alarm_metrics_by_runtime[entry.match.runtime];
    if (!Array.isArray(alarmMetrics) || alarmMetrics.length === 0) {
      throw new Error(`runtime ${entry.match.runtime} has no reviewed alarm metrics`);
    }
    const alarms = [];
    if (alarmMetrics.includes("useful-throughput")
        && ratios.throughput_confidence_high < policy.thresholds.throughput_confidence_high_minimum_ratio) {
      alarms.push("useful-throughput-collapse");
    }
    if (alarmMetrics.includes("p99") && ratios.p99 > policy.thresholds.p99_maximum_ratio) {
      alarms.push("p99-growth");
    }
    checks.push({
      match: entry.match,
      status: alarms.length === 0 ? "pass" : "alarm",
      measured_trials: group.measured_trials,
      baseline: entry.baseline,
      observed: {
        useful_outputs_per_second_median: throughputMedian,
        useful_outputs_per_second_confidence_95_high: throughputConfidenceHigh,
        p99_ns: p99,
        p99_9_ns: p99_9,
      },
      ratios,
      alarm_metrics: alarmMetrics,
      alarms,
    });
  }
}

const alarms = checks.flatMap((check) => check.alarms.map((alarm) => ({ match: check.match, alarm })));
const status = !applicable ? "not-applicable" : alarms.length === 0 ? "pass" : "alarm";
const result = {
  schema: "conduit.comparative-regression-evaluation",
  schema_version: 0,
  policy: {
    id: policy.id,
    policy_revision: policy.policy_revision,
    provenance: policy.provenance,
    threshold_calibration: policy.threshold_calibration,
    thresholds: policy.thresholds,
    claim_boundary: policy.claim_boundary,
  },
  source: {
    commit: metadata.commit,
    fixture_revision: summary.fixture_revision,
    samples: summary.samples,
  },
  applicability: {
    applicable,
    expected: policy.scope,
    observed: observedScope,
    mismatches: applicabilityReasons,
  },
  status,
  checks,
  alarms,
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(result, null, 2)}\n`, { flag: "wx" });

if (reportOutput) {
  const number = new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 });
  const report = [
    "# Comparative regression evaluation",
    "",
    `Policy: \`${policy.id}\` revision ${policy.policy_revision}`,
    "",
    `Status: **${status}**`,
    "",
    policy.claim_boundary,
    "",
  ];
  if (!applicable) {
    report.push("## Not applicable", "", "This artifact is retained as evidence but does not match the reviewed gate scope.", "", "| Field | Expected | Observed |", "| --- | --- | --- |");
    for (const mismatch of applicabilityReasons) {
      report.push(`| ${mismatch.field} | ${mismatch.expected} | ${mismatch.observed} |`);
    }
  } else {
    report.push("## Reviewed broad alarms", "", "Throughput uses the current bootstrap 95% median-confidence upper bound, so an alarm requires even that bound to fall below the policy floor. Conduit p99 is gated; comparison-runtime p99 and every p99.9 ratio remain report-only because prior hosted artifacts show outlier-heavy tails. These alarms request review; they are not performance guarantees.", "", "| Runtime | Workload | Depth | Capacity | Fan-out | Alarm metrics | Throughput median ratio | Throughput CI-high ratio | p99 ratio | p99.9 ratio | Status |", "| --- | --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | --- |");
    for (const check of checks) {
      report.push(`| ${check.match.runtime} | ${check.match.workload} | ${check.match.operators} | ${check.match.queue_capacity_items} | ${check.match.fanout_branches} | ${check.alarm_metrics.join(", ")} | ${number.format(check.ratios.throughput_median)} | ${number.format(check.ratios.throughput_confidence_high)} | ${number.format(check.ratios.p99)} | ${number.format(check.ratios.p99_9)} | ${check.status} |`);
    }
  }
  fs.mkdirSync(path.dirname(reportOutput), { recursive: true });
  fs.writeFileSync(reportOutput, `${report.join("\n")}\n`, { flag: "wx" });
}

if (status === "alarm") process.exitCode = 1;
