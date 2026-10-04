//! Check the complete bus/time Source topology before native profile admission.
extern crate std;
use super::*;
use alloc::vec::Vec;
#[test]
fn automatic_bus_time_topology_checks_with_exact_generic_schemas_and_reports_its_size() {
    let i2c = crate::i2c_base::contract::I2cContract::prepare().unwrap();
    let (mut startup, mut profile) = i2c.catalogs();
    let clock = crate::monotonic_clock::contract::MonotonicClockContract::prepare().unwrap();
    let clock_types = check_syntax_document(
        &parse_syntax_document(crate::monotonic_clock::contract::CLOCK_TYPES),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    for (name, path) in [
        ("MonotonicClockRequest", "machine/clock/at/request"),
        ("MonotonicClockResult", "machine/clock/at/result"),
    ] {
        startup
            .insert_checked_native_type(
                path,
                clock_types
                    .native_types
                    .iter()
                    .find(|ty| ty.name == name)
                    .unwrap(),
            )
            .unwrap();
    }
    startup
        .insert(conduit_plot::KindSignature {
            kind: "machine/clock/at".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    startup
        .insert_fore("machine/clock/at", clock.kind().checked_front())
        .unwrap();
    profile.insert_kind(clock.kind().clone()).unwrap();
    let events = include_str!("../../../../../plots/device-protocols/bme280-clock-events.conduit");
    let automatic = include_str!("../../../../../plots/device-protocols/bme280-autonomous.conduit");
    let combine = |units: &[&str]| {
        let imports = units
            .iter()
            .flat_map(|unit| unit.lines())
            .filter(|line| line.starts_with("with "))
            .collect::<alloc::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join("\n");
        let bodies = units
            .iter()
            .flat_map(|unit| unit.lines())
            .filter(|line| !line.starts_with("with "))
            .collect::<Vec<_>>()
            .join("\n");
        alloc::format!("{imports}\n{bodies}")
    };
    let types = check_syntax_document(
        &parse_syntax_document(&combine(&[LIFECYCLE, events])),
        &startup,
    )
    .unwrap();
    let schema = |name: &str| {
        &types
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
    };
    let state = schema("BmeProtocolState");
    let event = schema("BmeProtocolEvent");
    let context = schema("BmeClockContext");
    let value = |ty: &StructuredInfoType, maximum| {
        CheckedValueContract::new(ty.profile().unwrap().value_kind().clone(), maximum, vec![])
            .unwrap()
    };
    let state_value = value(state, 4096);
    let event_value = value(event, 4096);
    let context_value = value(context, 4096);
    let result_value = value(clock.result_type(), 512);
    conduit_semantic_catalog::install_seeded_state_flow_kind(
        &state_value,
        state,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    conduit_semantic_catalog::install_flow_zip_feedback_kind(
        &state_value,
        state,
        &event_value,
        event,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    conduit_semantic_catalog::install_flow_merge_finite_kind(
        &context_value,
        context,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    conduit_semantic_catalog::install_flow_zip_finite_kind(
        &context_value,
        context,
        &result_value,
        clock.result_type(),
        &mut startup,
        &mut profile,
    )
    .unwrap();
    // The feedback unit supplies the ordinary tuple-to-transition assembler.
    let checked = check_syntax_document(
        &parse_syntax_document(&combine(&[LIFECYCLE, events, FEEDBACK, automatic])),
        &startup,
    )
    .unwrap();
    for stage in checked.plots.iter().flat_map(|plot| &plot.cords).flat_map(|cord| &cord.stages) {
        if let conduit_plot::CheckedCordStage::StructuredSelector { selector, .. } = stage {
            profile.insert(conduit_plot::structured_selector_definition(selector, PortTemporal::Flow { closes: true })).unwrap();
        }
    }
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "bme280-autonomous", &profile).unwrap();
    std::eprintln!(
        "automatic BME280 Source topology: {} gears",
        expanded.expanded.gears.len()
    );
    assert!(
        expanded.expanded.gears.len() > 16,
        "measure the full topology before changing native admission limits"
    );
    for kind in [
        "machine/i2c/transact",
        "machine/clock/at",
        "flow/merge/finite",
        "flow/zip/feedback",
        "state/seeded/flow/finite",
    ] {
        assert!(
            expanded
                .expanded
                .gears
                .iter()
                .any(|gear| gear.kind_id.as_str() == kind),
            "missing {kind}"
        );
    }
}
