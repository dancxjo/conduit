//! Dispersion across repeated batches, separate from within-batch event latency.
use crate::timing;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn summarize(runs: &[Value]) -> Vec<Value> {
    let mut groups: BTreeMap<(String, String, String), Vec<u64>> = BTreeMap::new();
    for run in runs {
        let case = run["case"].as_str().expect("owned report case");
        let mode = run["mode"]
            .as_str()
            .unwrap_or("finite-ordinary-hosted-pipeline");
        for (metric, value) in [
            ("bulk_wall_ns", run["bulk_wall_ns"].as_u64()),
            ("bulk_cpu_ns", run["bulk_cpu_ns"].as_u64()),
            ("selection_wall_ns", run["selection_wall_ns"].as_u64()),
            (
                "full_host_call.wall_ns",
                run["full_host_call"]["wall_ns"].as_u64(),
            ),
            (
                "full_host_call.process_cpu_ns",
                run["full_host_call"]["process_cpu_ns"].as_u64(),
            ),
        ] {
            if let Some(value) = value {
                groups
                    .entry((case.into(), mode.into(), metric.into()))
                    .or_default()
                    .push(value);
            }
        }
    }
    groups.into_iter().map(|((case, mode, metric), samples)| json!({
        "case":case, "mode":mode, "metric":metric,
        "samples_ns":samples, "distribution":timing::distribution(&samples),
        "scope":"repeated batches within one worker process; descriptive variance, not independent-process confidence"
    })).collect()
}

#[cfg(test)]
mod tests {
    use super::summarize;
    use serde_json::json;

    #[test]
    fn batches_keep_modes_and_cpu_availability_separate() {
        let report = summarize(&[
            json!({"case":"add", "mode":"native", "bulk_wall_ns":1, "bulk_cpu_ns":null}),
            json!({"case":"add", "mode":"prepared", "bulk_wall_ns":20}),
            json!({"case":"add", "mode":"native", "bulk_wall_ns":5, "bulk_cpu_ns":3}),
            json!({"case":"pipeline", "full_host_call":{"wall_ns":9}}),
        ]);
        assert_eq!(report.len(), 4);
        let native_wall = report
            .iter()
            .find(|r| r["mode"] == "native" && r["metric"] == "bulk_wall_ns")
            .unwrap();
        assert_eq!(native_wall["samples_ns"], json!([1, 5]));
        assert_eq!(native_wall["distribution"]["mean_ns"], 3.0);
        let cpu = report
            .iter()
            .find(|r| r["metric"] == "bulk_cpu_ns")
            .unwrap();
        assert_eq!(cpu["distribution"]["count"], 1);
        assert!(cpu["distribution"]["sample_standard_deviation_ns"].is_null());
    }
}
