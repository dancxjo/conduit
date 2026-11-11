//! Checked Source and ordinary planning; no MCU or installed Body is claimed.
#[path = "display_support/fixture.rs"]
mod fixture;
use conduit_environment_make::{DisplayOutputSource, PreparedDisplayOutput};
#[test]
fn authored_output_keeps_typed_frame_refusal_and_exact_electrical_selection() {
    let source = DisplayOutputSource::compile().unwrap();
    assert_eq!(source.plot.input_bindings.len(), 1);
    assert_eq!(source.plot.output_bindings.len(), 1);
    let fixture = fixture::fixture(&source);
    let bases = vec!["conduit.base/local@1".into(), "fixture/base".into()];
    assert!(PreparedDisplayOutput::prepare(&source, &fixture.host, &bases, &[]).is_err());
    let prepared = PreparedDisplayOutput::prepare(
        &source,
        &fixture.host,
        &bases,
        core::slice::from_ref(&fixture.grant),
    )
    .unwrap();
    assert!(conduit_core::verify_plan(prepared.plan()));
    assert_eq!(
        prepared.maximum_value_bytes(),
        source.maximum_value_bytes().unwrap()
    );
    assert_eq!(prepared.lowered().nodes.len(), 4);
    assert_eq!(prepared.lowered().cords.len(), 5);
    assert_eq!(prepared.lowered().fore_ports.len(), 2);
    assert_eq!(prepared.lowered().host_calls.len(), 3);
    assert!(prepared.lowered().remote_endpoints.is_empty());
    assert!(prepared
        .lowered()
        .fore_ports
        .iter()
        .all(|port| port.selected_line.is_none()));
    assert_eq!(prepared.plan().fragments[0].host_id, fixture.host.host_id);
    assert_eq!(prepared.plan().fragments[0].boot_id, fixture.host.boot_id);
    let mut missing = fixture.host.clone();
    missing
        .capabilities
        .retain(|offer| offer.kind_id.as_str() != conduit_tm1637_wire::WRITE_KIND);
    assert!(PreparedDisplayOutput::prepare(&source, &missing, &bases, &[fixture.grant]).is_err());
}

#[test]
fn payload_extent_comes_from_all_checked_types_and_refuses_unproved_ports() {
    let mut source = DisplayOutputSource::compile().unwrap();
    let extent = source.maximum_value_bytes().unwrap();
    assert!(extent >= conduit_environment_runtime::display_packets::DISPLAY_FRAME_BYTES as u32);
    assert!(extent < conduit_tm1637_wire::MAXIMUM_BYTES);
    assert_eq!(extent, 651);
    let original = source.plot.expanded.gears[0].inputs[0].value_kind.clone();
    source.plot.expanded.gears[0].inputs[0].value_kind = "fixture/unknown-type".into();
    assert!(source.maximum_value_bytes().is_err());
    source.plot.expanded.gears[0].inputs[0].value_kind = original;
    source.plot.expanded.gears[0].inputs[0].abnormal_kind = Some("fixture/abnormal".into());
    assert!(source.maximum_value_bytes().is_err());
}
