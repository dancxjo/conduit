mod support;

use conduit_web::{run_panel, run_panel_exact};

const SOURCE: &str = "panel 0\n\
greeting: std/literal { value = \"Hello from Conduit.\\n\" }\n\
shout: text/uppercase\n\
output: display/text\n\
greeting.value > shout.text {\n\
  capacity = 2\n\
  max_value_bytes = 64\n\
  max_queued_bytes = 128\n\
  low_watermark = 1\n\
  high_watermark = 2\n\
  pressure = block\n\
}\n\
shout.text > output.text {\n\
  capacity = 2\n\
  max_value_bytes = 64\n\
  max_queued_bytes = 128\n\
  low_watermark = 1\n\
  high_watermark = 2\n\
  pressure = block\n\
}\n";
const FORMAT_SOURCE: &str = include_str!("../../../examples/formatted-greeting.panel");

#[test]
fn browser_entrypoint_executes_the_authored_exact_plan() {
    let result: serde_json::Value = serde_json::from_str(&run_panel(SOURCE.to_owned())).unwrap();

    assert_eq!(result["ok"], true);
    assert_eq!(result["profile"], "exact-plan-deterministic-executor");
    assert_eq!(result["completed_nodes"], 3);
    assert_eq!(result["cords_conducted"], 2);
    assert_eq!(result["stdout"], "");
    assert_eq!(result["display"], "HELLO FROM CONDUIT.\n");
    assert!(result["scheduler_event_count"].as_u64().unwrap() > 0);
    assert!(result["high_water"]["queue_items"].as_u64().unwrap() <= 4);
    assert!(
        result["high_water"]["queue_payload_bytes"]
            .as_u64()
            .unwrap()
            <= 256
    );
    let evidence = result["evidence"].as_array().expect("typed evidence array");
    assert!(!evidence.is_empty());
    assert!(
        result["evidence_bytes"].as_u64().unwrap() <= 16 * 1024,
        "evidence bytes: {}",
        result["evidence_bytes"]
    );
    assert!(evidence.iter().all(|event| {
        event["schema"] == "conduit.exact-execution-evidence"
            && event["plan_epoch"] == 1
            && event["run_id"] == "conduit/browser-run"
    }));
    assert!(evidence.iter().any(|event| {
        event["subject_kind"] == "node"
            && event["implementation_id"]
                .as_str()
                .is_some_and(|value| value.starts_with("conduit/hosted-"))
            && event["artifact_id"].as_str().is_some()
            && event["host_id"] == "conduit/conduct-host"
    }));
    assert!(evidence.iter().any(|event| {
        event["subject_kind"] == "cord"
            && event["from_port"].as_str().is_some()
            && event["to_port"].as_str().is_some()
            && event["pressure"] == "block"
    }));
    assert!(evidence.iter().any(|event| {
        event["event_kind"] == "terminal" && event["terminal_cause"] == "succeeded"
    }));
    let logical = result["patchbay"]["topology"]["logical_nodes"]
        .as_array()
        .expect("semantic Patchbay nodes");
    assert!(logical.iter().all(|node| node.get("placement").is_none()));
    let planned = result["patchbay"]["topology"]["planned_realization"]["nodes"]
        .as_array()
        .expect("planned Patchbay nodes");
    assert!(planned.iter().all(|node| node.get("placement").is_none()));
}

#[test]
fn patchbay_projects_final_formatter_ports_from_authoritative_contracts() {
    let result: serde_json::Value =
        serde_json::from_str(&run_panel(FORMAT_SOURCE.to_owned())).unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(
        result["display"],
        "Hello, operator. Payload: {status = ready}\n"
    );
    assert!(result["evidence_bytes"].as_u64().unwrap() <= 64 * 1024);
    let layers = [
        result["patchbay"]["topology"]["logical_nodes"]
            .as_array()
            .unwrap(),
        result["patchbay"]["topology"]["planned_realization"]["nodes"]
            .as_array()
            .unwrap(),
    ];
    for (index, nodes) in layers.into_iter().enumerate() {
        let formatter = nodes
            .iter()
            .find(|node| {
                if index == 0 {
                    node["contract_id"] == "std/text/format"
                } else {
                    node["binding"]["contract_id"] == "std/text/format"
                }
            })
            .unwrap();
        let inputs = formatter["inputs"].as_array().unwrap();
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0]["id"], "template");
        assert_eq!(inputs[0]["type_id"], "std/text");
        assert_eq!(inputs[1]["id"], "values");
        assert_eq!(inputs[1]["type_id"], "std/format-values");
        let outputs = formatter["outputs"].as_array().unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0]["id"], "text");
        assert_eq!(outputs[0]["type_id"], "std/text");
    }
}

#[test]
fn browser_rejects_compile_candidate_as_executable_authority() {
    let input = support::browser_compile_input(SOURCE);
    let result: serde_json::Value = serde_json::from_str(&run_panel_exact(
        SOURCE.to_owned(),
        serde_json::to_string(&input).unwrap(),
    ))
    .unwrap();

    assert_eq!(result["ok"], false);
    assert_eq!(result["code"], "CND-RUN-007");
}

#[test]
fn browser_entrypoint_rejects_active_source_mutation() {
    let input = support::browser_compile_input(SOURCE);
    let result: serde_json::Value = serde_json::from_str(&run_panel_exact(
        SOURCE.replace("Hello", "Changed"),
        serde_json::to_string(&input).unwrap(),
    ))
    .unwrap();

    assert_eq!(result["ok"], false);
    assert_eq!(result["code"], "CND-CMP-003");
}
