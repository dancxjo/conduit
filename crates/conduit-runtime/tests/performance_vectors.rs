use serde_json::Value;

const FIXTURE: &str = include_str!("../../../conformance/c4/performance.json");
const BASELINE: &str = include_str!("../../../benchmarks/baseline.json");

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
