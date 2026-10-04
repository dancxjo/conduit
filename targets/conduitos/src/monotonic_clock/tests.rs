use super::contract::*;
use conduit_core::*;
use conduit_plot::*;

#[test]
fn reviewed_deadline_and_observation_types_have_finite_exact_port_contracts() {
    let contract = MonotonicClockContract::prepare().unwrap();
    contract.kind().validate().unwrap();
    for ty in [contract.request_type(), contract.result_type()] {
        assert!(maximum_prepared_canonical_value_bytes(ty).unwrap() <= CLOCK_MAXIMUM_BYTES);
    }
    let (startup, profile) = contract.catalogs();
    let source = "with machine/clock/at/request as ClockRequest\nwith machine/clock/at/result as ClockResult\nplot observe (\n deadline: U64...| >> result: ClockResult...|\n) {\n clock: machine/clock/at\n deadline >> request() >> clock >> result\n}\nplot request (\n deadline: U64...| >> request: ClockRequest...|\n) = ({ deadline: . })\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "observe", &profile).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 2);
    assert_eq!(contract.kind().inputs[0].port_id, port_id("request"));
    assert_eq!(contract.kind().outputs[0].port_id, port_id("result"));
    assert!(
        contract
            .kind()
            .inputs
            .iter()
            .chain(&contract.kind().outputs)
            .all(|port| port.temporal == PortTemporal::Flow { closes: true }
                && port.abnormal_kind.is_none())
    );
}
