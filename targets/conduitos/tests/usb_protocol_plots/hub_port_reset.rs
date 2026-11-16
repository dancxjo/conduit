use conduit_core::validate_canonical_structured_value;
use conduit_plot::PreparedPortableExpressionEvaluator;
use conduitos::usb_base::control_contract::ControlContract;
fn program(entry: &str) -> conduit_plot::PortableExpressionProgram {
    let source = [
        include_str!("../../plots/usb/control-types.conduit"),
        include_str!("../../plots/usb/hub-port-reset.conduit"),
    ]
    .join("\n");
    let (startup, profile) = ControlContract::prepare().unwrap().catalogs();
    super::program_with_catalog(&source, entry, &startup, &profile)
}
fn tag(bytes: &[u8], expected: &str) {
    assert!(
        validate_canonical_structured_value(bytes)
            .unwrap()
            .variant_payload(expected)
            .unwrap()
            .is_some(),
        "expected {expected}"
    );
}
#[test]
fn hub_port_reset_and_change_acknowledgements_have_exact_wire_requests() {
    for (entry, opcode, feature) in [
        ("usb-hub-port-reset-request", 3, 4),
        ("usb-hub-port-reset-change-acknowledge-request", 1, 20),
        ("usb-hub-port-connection-change-acknowledge-request", 1, 16),
    ] {
        let program = program(entry);
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        tag(evaluator.evaluate(&[0]).unwrap(), "invalid-port");
        for port in 1..=255_u8 {
            let value =
                validate_canonical_structured_value(evaluator.evaluate(&[port]).unwrap()).unwrap();
            let request = value.variant_payload("request").unwrap().unwrap();
            assert_eq!(
                request
                    .record_field("setup")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                [0x23, opcode, feature, 0, port, 0, 0, 0]
            );
            assert_eq!(
                request
                    .record_field("output")
                    .unwrap()
                    .unwrap()
                    .collection_length()
                    .unwrap(),
                0
            );
        }
    }
}
#[test]
fn hub_port_readiness_preserves_faults_and_never_infers_speed_before_enable() {
    let program = program("usb-hub-port-readiness");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let allocations = super::allocation::allocations(|| {
        for (status, expected) in [
            (0_u16, "unpowered"),
            (8, "overcurrent"),
            (0x100, "disconnected"),
            (0x111, "resetting"),
            (0x105, "suspended"),
            (0x101, "disabled"),
            (0x701, "disabled"),
            (0x103, "ready-full-speed"),
            (0x303, "ready-low-speed"),
            (0x503, "ready-high-speed"),
            (0x703, "malformed"),
            (0x713, "resetting"),
            (0x70b, "overcurrent"),
        ] {
            tag(evaluator.evaluate(&status.to_le_bytes()).unwrap(), expected);
        }
    });
    assert_eq!(allocations, 0);
}
