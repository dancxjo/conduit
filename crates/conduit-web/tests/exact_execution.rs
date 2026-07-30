mod support;

use conduit_web::{run_panel, run_panel_exact};

const SOURCE: &str = "panel 1\n\
node greeting : conduit.std/literal { value = \"Hello from Conduit.\\n\" }\n\
node shout : conduit.std/uppercase\n\
node output : conduit.std/stdout\n\
cord greeting.out -> shout.in {\n\
  capacity = 2\n\
  max_value_bytes = 64\n\
  max_queued_bytes = 128\n\
  low_watermark = 1\n\
  high_watermark = 2\n\
  pressure = block\n\
}\n\
cord shout.out -> output.in {\n\
  capacity = 2\n\
  max_value_bytes = 64\n\
  max_queued_bytes = 128\n\
  low_watermark = 1\n\
  high_watermark = 2\n\
  pressure = block\n\
}\n";

#[test]
fn browser_entrypoint_executes_the_authored_exact_plan() {
    let result: serde_json::Value = serde_json::from_str(&run_panel(SOURCE.to_owned())).unwrap();

    assert_eq!(result["ok"], true);
    assert_eq!(result["profile"], "exact-plan-deterministic-executor");
    assert_eq!(result["completed_nodes"], 3);
    assert_eq!(result["cords_conducted"], 2);
    assert_eq!(result["stdout"], "HELLO FROM CONDUIT.\n");
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
