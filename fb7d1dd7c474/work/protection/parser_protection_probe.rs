#![cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_joint_v2_fixture.rs"]
mod joint;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/scorer_model.rs"]
mod scorer_model;
#[test]
fn exact_native_fact_compact_projection_prepares() {
    let source = [
        joint::source(),
        include_str!("../parser_session.conduit").into(),
        include_str!("../parser_session_facts.conduit").into(),
        include_str!("../../../work/protection/parser_session_protection.conduit").into(),
    ]
    .join("\n");
    let _ = parser_kernel::Blueprint::prepare(source, "language-parser-protected-edge-projection");
}
