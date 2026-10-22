//! The same semantic contracts consumed by the private HTTP submission boundary.
#[test]
fn authored_network_configuration_contains_only_the_existing_finite_typed_flow() {
    let source = conduit_environment_make::compile_network_setup().unwrap();
    source.validate_expansion().unwrap();
    assert_eq!((source.gears.len(), source.connections.len()), (3, 2));
    for (name, kind) in [
        ("credentials", conduit_net::NETWORK_CREDENTIALS_OPERATION),
        ("join", conduit_net::NETWORK_JOIN_OPERATION),
        ("attached", conduit_net::NETWORK_ATTACHMENT_SIGN_OPERATION),
    ] {
        let gear = source
            .gears
            .iter()
            .find(|g| g.kind_id.as_str() == kind)
            .unwrap();
        assert!(gear.gear_id.as_str().ends_with(name));
        assert!(gear.configuration.is_empty());
        for port in gear.inputs.iter().chain(&gear.outputs) {
            assert_eq!(port.temporal, conduit_core::PortTemporal::Value);
        }
    }
    assert!(source
        .connections
        .iter()
        .any(|cord| cord.source_port_id.as_str() == "request"
            && cord.sink_port_id.as_str() == "request"));
    assert!(source
        .connections
        .iter()
        .any(|cord| cord.source_port_id.as_str() == "attachment"
            && cord.sink_port_id.as_str() == "attachment"));
}
