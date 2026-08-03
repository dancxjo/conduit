use serde_json::Value;

const FIXTURE: &str = include_str!("../../../conformance/c4/performance.json");
const BASELINE: &str = include_str!("../../../benchmarks/baseline.json");
const COMPARATIVE_MANIFEST: &str = include_str!("../../../benchmarks/comparative/manifest.json");
const COMPARATIVE_RAW_SCHEMA: &str =
    include_str!("../../../benchmarks/comparative/raw-sample.schema.json");
const COMPARATIVE_REGRESSION_POLICY: &str =
    include_str!("../../../benchmarks/comparative/regression-policy.json");
const RXJS_LOCK: &str =
    include_str!("../../../benchmarks/comparative/javascript/package-lock.json");

#[test]
fn performance_fixture_and_reviewed_baseline_are_complete() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let baseline: Value = serde_json::from_str(BASELINE).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(fixture["suite"], "conduit.performance");
    assert_eq!(cases.len(), 24);
    assert!(cases.iter().all(|case| {
        case["id"].as_str().is_some_and(|id| !id.is_empty())
            && case["runner"]
                .as_str()
                .is_some_and(|runner| !runner.is_empty())
            && case["expected"].is_object()
    }));

    assert_eq!(baseline["schema"], "conduit.performance-baseline");
    assert_eq!(baseline["fixture_revision"], 18);
    assert_eq!(baseline["owner"], "@dancxjo");
    assert_eq!(baseline["workloads"].as_array().unwrap().len(), 9);
    assert!(
        baseline["artifacts"]
            .as_object()
            .unwrap()
            .values()
            .all(|artifact| {
                artifact["baseline_bytes"]
                    .as_u64()
                    .is_some_and(|value| value > 0)
                    && artifact["maximum_growth_percent"]
                        .as_u64()
                        .is_some_and(|value| value > 0)
                    && artifact["maximum_growth_bytes"]
                        .as_u64()
                        .is_some_and(|value| value > 0)
            })
    );
    assert_eq!(
        baseline["artifacts"]["conduit-core-thumbv6m-release"]["kind"],
        "embedded-library-archive-not-flash"
    );
    assert_eq!(baseline["deferred_workloads"]["tongues"], "issue #31");
    assert_eq!(baseline["deferred_workloads"]["netherwick"], "issue #33");
    assert_eq!(
        baseline["deferred_workloads"]["rp2040_flash_static_ram_and_stack"],
        "issue #28"
    );
    assert_eq!(
        baseline["deferred_workloads"]["plan_transition_overlap"],
        "issue #57"
    );
}

#[test]
fn comparative_methodology_pins_matrix_schema_and_runtimes() {
    let manifest: Value = serde_json::from_str(COMPARATIVE_MANIFEST).unwrap();
    let schema: Value = serde_json::from_str(COMPARATIVE_RAW_SCHEMA).unwrap();
    let regression: Value = serde_json::from_str(COMPARATIVE_REGRESSION_POLICY).unwrap();
    let rxjs_lock: Value = serde_json::from_str(RXJS_LOCK).unwrap();

    assert_eq!(manifest["schema"], "conduit.comparative-benchmark-manifest");
    assert_eq!(manifest["schema_version"], 0);
    assert_eq!(manifest["fixture_revision"], 0);
    assert_eq!(manifest["issues"], serde_json::json!([243, 244, 245, 249]));
    assert_eq!(manifest["values"], 1_000_000);
    assert_eq!(manifest["operator_depths"], serde_json::json!([1, 8, 32]));
    assert_eq!(
        manifest["workloads"],
        serde_json::json!(["map", "map-filter", "merge", "bounded-async"])
    );
    assert_eq!(manifest["warmup_trials"], 2);
    assert_eq!(manifest["measured_trials"], 9);
    assert_eq!(
        manifest["overload"]["queue_capacity_items"],
        serde_json::json!([4, 64, 1024])
    );
    assert_eq!(
        manifest["overload"]["pressure_policies"],
        serde_json::json!([
            "block",
            "reject",
            "coalesce",
            "sample",
            "drop-disposable",
            "disconnect",
            "fail"
        ])
    );
    assert_eq!(manifest["overload"]["branch_count"], 1);
    assert_eq!(
        manifest["fanout"]["queue_capacity_items"],
        serde_json::json!([4, 64, 1024])
    );
    assert_eq!(
        manifest["fanout"]["branches"],
        serde_json::json!([2, 8, 32])
    );
    assert_eq!(
        manifest["fanout"]["modes"],
        serde_json::json!(["coupled", "isolated"])
    );
    assert_eq!(
        manifest["fanout"]["slow_branches"],
        serde_json::json!(["one", "all"])
    );
    assert_eq!(
        manifest["cancellation"]["stop_policies"],
        serde_json::json!(["drain", "abort"])
    );
    assert_eq!(manifest["cancellation"]["pressure_policy"], "block");
    assert_eq!(manifest["bursty_consumers"]["queue_capacity_items"], 4);
    assert_eq!(manifest["bursty_consumers"]["consumer_burst_items"], 8);
    assert_eq!(manifest["bursty_consumers"]["consumer_pause_yields"], 8);
    assert_eq!(
        manifest["bursty_consumers"]["pressure_policies"],
        serde_json::json!(["block", "reject", "coalesce", "sample", "drop-disposable"])
    );
    assert_eq!(
        manifest["bursty_consumers"]["fanout_branches"],
        serde_json::json!([2, 8, 32])
    );
    assert_eq!(
        manifest["persistent_sessions"]["pressure_policies"],
        serde_json::json!(["block", "reject", "coalesce", "sample", "drop-disposable"])
    );
    assert_eq!(
        manifest["persistent_sessions"]["stop_policies"],
        serde_json::json!(["drain", "abort"])
    );
    assert_eq!(manifest["persistent_sessions"]["session_pump_quantum"], 8);
    assert_eq!(
        manifest["persistent_wake_residency"]["workload"],
        "persistent-wake"
    );
    assert_eq!(manifest["persistent_wake_residency"]["host_wakes"], 10_000);
    assert_eq!(
        manifest["persistent_wake_residency"]["residency_plateau_after_wakes"],
        1_000
    );
    assert_eq!(
        manifest["persistent_wake_residency"]["stop_policy"],
        "drain"
    );
    assert_eq!(
        manifest["persistent_timer_residency"]["workload"],
        "persistent-timer"
    );
    assert_eq!(
        manifest["persistent_timer_residency"]["timer_wakes"],
        10_000
    );
    assert_eq!(
        manifest["persistent_timer_residency"]["residency_plateau_after_wakes"],
        1_000
    );
    assert_eq!(
        manifest["persistent_timer_residency"]["stop_policy"],
        "drain"
    );
    assert_eq!(
        manifest["persistent_timer_residency"]["timer_advance_ticks"],
        64
    );
    assert_eq!(
        manifest["shared_payload_fanout"]["payload_bytes"],
        serde_json::json!([1024])
    );
    assert_eq!(
        manifest["shared_payload_fanout"]["branches"],
        serde_json::json!([2, 8, 32])
    );
    assert_eq!(
        manifest["shared_payload_fanout"]["termination_requests"],
        serde_json::json!(["complete", "abort"])
    );
    assert_eq!(
        manifest["wall_clock_policy"]["gate"],
        "strict only for the separately versioned reviewed regression policy when machine class, architecture, input cardinality, and trial counts match exactly; otherwise report-only"
    );
    assert_eq!(
        manifest["wall_clock_policy"]["policy"],
        "regression-policy.json"
    );
    assert_eq!(
        manifest["runtimes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|runtime| runtime["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "conduit-reference-scheduler",
            "conduit-hosted-value-arena",
            "rxjs",
            "reactor-core"
        ]
    );
    assert_eq!(
        manifest["language_lower_bounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|runtime| runtime["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "rust-identity-loop",
            "javascript-identity-loop",
            "java-identity-loop"
        ]
    );

    assert_eq!(
        schema["properties"]["schema"]["const"],
        "conduit.comparative-raw-sample"
    );
    assert_eq!(schema["properties"]["schema_version"]["const"], 0);
    assert_eq!(schema["properties"]["fixture_revision"]["const"], 0);
    assert!(
        schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == "sample_kind")
    );
    assert!(
        schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == "outcomes")
    );
    assert_eq!(
        schema["properties"]["outcomes"]["required"],
        serde_json::json!([
            "offered",
            "admitted",
            "completed_useful",
            "rejected",
            "sampled",
            "coalesced",
            "dropped",
            "cancelled",
            "retried",
            "terminal"
        ])
    );
    assert!(
        [
            "fanout_branches",
            "fanout_mode",
            "slow_branches",
            "termination_request",
            "cancel_after_offers",
            "consumer_pattern",
            "consumer_burst_items",
            "session_mode",
            "session_pump_quantum",
            "residency_plateau_after_wakes",
            "timer_advance_ticks",
            "payload_bytes",
            "payload_representation"
        ]
        .iter()
        .all(|field| schema["properties"]["workload"]["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|required| required == field))
    );
    assert!(["pressure_cycles", "recovery_cycles"].iter().all(|field| {
        schema["properties"]["phases"]["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|required| required == field)
    }));
    assert!(
        [
            "queue_max_cord_items_high_water",
            "value_resident_slots_after_terminal",
            "value_resident_bytes_after_terminal",
            "value_slots_high_water",
            "value_bytes_high_water",
            "value_slots_capacity",
            "value_bytes_capacity",
            "host_io_capacity_bytes",
            "host_io_output_bytes"
        ]
        .iter()
        .all(|field| schema["properties"]["memory"]["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|required| required == field))
    );
    assert_eq!(
        schema["properties"]["execution"]["required"],
        serde_json::json!([
            "scheduler_decisions",
            "producer_stall_ns",
            "drain_ns",
            "abort_ns",
            "session_pumps",
            "session_reserved_bytes",
            "pressured_items_at_stop",
            "session_host_wakes",
            "session_timer_wakes",
            "residency_plateau_verified",
            "residency_checkpoint_queue_items_high_water",
            "residency_checkpoint_queue_payload_bytes_high_water",
            "residency_checkpoint_ready_slots_high_water",
            "residency_checkpoint_evidence_slots_high_water",
            "unique_value_handles",
            "branch_deliveries"
        ])
    );
    assert_eq!(
        rxjs_lock["packages"]["node_modules/rxjs"]["version"],
        "7.8.2"
    );

    assert_eq!(
        regression["schema"],
        "conduit.comparative-regression-policy"
    );
    assert_eq!(regression["schema_version"], 0);
    assert_eq!(regression["policy_revision"], 0);
    assert_eq!(regression["issues"], serde_json::json!([241, 245]));
    assert_eq!(
        regression["scope"],
        serde_json::json!({
            "machine_class": "github-hosted-ubuntu-x86_64",
            "architecture": "x86_64",
            "input_values": 10_000,
            "warmup_trials": 2,
            "measured_trials": 9
        })
    );
    assert_eq!(regression["provenance"]["artifact_id"], 8_854_832_510_u64);
    assert_eq!(
        regression["provenance"]["head_commit"],
        "0f4c3eb5b74c7be31f7da50b2914e4616777661c"
    );
    assert_eq!(
        regression["thresholds"]["throughput_confidence_high_minimum_ratio"],
        0.5
    );
    assert_eq!(regression["thresholds"]["p99_maximum_ratio"], 3);
    assert_eq!(regression["thresholds"]["p99_9"], "report-only");
    assert_eq!(
        regression["thresholds"]["alarm_metrics_by_runtime"],
        serde_json::json!({
            "conduit-reference-scheduler": ["useful-throughput", "p99"],
            "rxjs": ["useful-throughput"],
            "reactor-core": ["useful-throughput"]
        })
    );
    assert_eq!(
        regression["threshold_calibration"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    let baselines = regression["baselines"].as_array().unwrap();
    assert_eq!(baselines.len(), 13);
    assert!(baselines.iter().all(|baseline| {
        baseline["match"]["runtime"]
            .as_str()
            .is_some_and(|runtime| !runtime.is_empty())
            && baseline["match"]["workload"]
                .as_str()
                .is_some_and(|workload| !workload.is_empty())
            && baseline["baseline"]["useful_outputs_per_second_median"]
                .as_f64()
                .is_some_and(|value| value > 0.0)
            && baseline["baseline"]["p99_ns"]
                .as_f64()
                .is_some_and(|value| value > 0.0)
            && baseline["baseline"]["p99_9_ns"]
                .as_f64()
                .is_some_and(|value| value > 0.0)
    }));
}

#[test]
fn small_medium_and_reviewed_maximum_linear_graphs_parse() {
    for nodes in [2_usize, 32, 256] {
        let mut source = String::from("panel 0\n");
        for index in 0..nodes {
            source.push_str(&format!("n{index}: fixture/node\n"));
        }
        for index in 0..nodes - 1 {
            source.push_str(&format!("n{index}.value > n{}.value\n", index + 1));
        }
        let panel = conduit_panel::parse(&source).unwrap();
        assert_eq!(panel.nodes.len(), nodes);
        assert_eq!(panel.cords.len(), nodes - 1);
    }
}
