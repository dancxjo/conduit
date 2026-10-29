//! Checked authored scan meaning; this test does not initialize a radio.
#[test]
fn scan_source_checks_with_a_connected_typed_evidence_sink() {
    let plot = conduit_environment_make::compile_network_scan().unwrap();
    assert_eq!(plot.name, "environment-network-scan");
    assert_eq!(plot.gears.len(), 2);
    assert_eq!(plot.connections.len(), 1);
    // Invocation generation and deadlines belong to the retained provider,
    // rather than an invented authored input or remote request port.
    assert!(conduit_net::network_scan_contract().inputs.is_empty());
    let sink = conduit_net::network_scan_sign_contract();
    assert_eq!(sink.inputs.len(), 1);
    assert!(sink.outputs.is_empty());
    // The existing installed join Source must still check with this catalog.
    conduit_environment_make::compile_network_setup().unwrap();
}
