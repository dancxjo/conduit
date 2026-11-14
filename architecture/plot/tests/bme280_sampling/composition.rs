//! Higher-order Source shape proof only. The fixture Fore contracts install no
//! runtime owners; actual standard-family admission remains source-build proof.
use super::*;
use conduit_core as c;
use conduit_plot::KindSignature;

fn install(
    startup: &mut StartupCatalog,
    profiles: &mut ProfileCatalog,
    name: &str,
    inputs: &[(&str, StructuredInfoType)],
    outputs: &[(&str, StructuredInfoType)],
) {
    let ports = |values: &[(&str, StructuredInfoType)], direction| {
        values
            .iter()
            .map(|(name, ty)| c::PortDescriptor {
                port_id: c::port_id(name),
                value_kind: ty.profile().unwrap().value_kind().clone(),
                direction,
                temporal: c::PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            })
            .collect()
    };
    let kind = c::Kind {
        kind_id: c::kind_id(name),
        kind_contract_revision: "fixture/checked-fore@1".into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: ports(inputs, c::PortDirection::Input),
        outputs: ports(outputs, c::PortDirection::Output),
        configuration: vec![],
        semantic_laws: vec![c::KindSemanticLaw::ValueContracts(
            inputs
                .iter()
                .map(|(name, ty)| (c::FrontValueLocation::Input(c::port_id(name)), ty))
                .chain(
                    outputs
                        .iter()
                        .map(|(name, ty)| (c::FrontValueLocation::Output(c::port_id(name)), ty)),
                )
                .map(|(location, ty)| c::FrontValueContract {
                    location,
                    contract: c::CheckedValueContract::new(
                        ty.profile().unwrap().value_kind().clone(),
                        conduit_plot::maximum_prepared_canonical_value_bytes(ty).unwrap(),
                        vec![],
                    )
                    .unwrap(),
                })
                .collect(),
        )],
        limits: c::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 4096,
        },
    };
    startup
        .insert(KindSignature {
            kind: name.into(),
            startup_parameters: vec![],
        })
        .unwrap();
    startup.insert_fore(name, kind.checked_front()).unwrap();
    profiles.insert_kind(kind).unwrap();
}
#[test]
fn selected_sampling_exchange_checks_and_expands_with_exact_paired_types() {
    let units = [
        include_str!("../../../../plots/device-protocols/bme280-lifecycle.conduit"),
        include_str!("../../../../plots/device-protocols/bme280-compensation.conduit"),
        include_str!("../../../../plots/device-protocols/main.conduit"),
        include_str!("../../../../plots/device-protocols/bme280-capture-observation.conduit"),
        include_str!("../../../../plots/device-protocols/bme280-clock-events.conduit"),
        include_str!("../../../../plots/device-protocols/bme280-exchange.conduit"),
        include_str!("../../../../plots/device-protocols/bme280-sampling.conduit"),
        include_str!("../../../../plots/device-protocols/bme280-sampling-exchange.conduit"),
    ];
    let imports = units
        .iter()
        .flat_map(|s| s.lines())
        .filter(|l| l.starts_with("with "))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n");
    let bodies = units
        .iter()
        .flat_map(|s| s.lines())
        .filter(|l| !l.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let library = format!("{imports}\n{bodies}");
    let mut startup = lifecycle_catalog();
    let types =
        conduit_plot::prepare_source_types(&parse_syntax_document(&library), &startup).unwrap();
    let ty = |name| types.named_type(name).unwrap().clone();
    let mut profiles = ProfileCatalog::new();
    for name in [
        "BmeProtocolState",
        "BmeProtocolEvent",
        "BmeSamplingBegin",
        "BmeSamplingState",
        "BmeSamplingControl",
        "BmeSamplingAdvanced",
        "BmeSamplingIdle",
        "BmeSamplingIdleEvent",
        "BmeSamplingIdleTransition",
        "BmeSamplingIdleResult",
    ] {
        let extent = conduit_plot::maximum_prepared_canonical_value_bytes(&ty(name)).unwrap();
        eprintln!("sampling extent {name}: {extent}");
        assert!(
            extent <= 4096,
            "{name} exceeds original MCU envelope: {extent}"
        );
    }
    for (name, left, right, alias) in [
        (
            "fixture/sampling-seed",
            "BmeSamplingBegin",
            "BmeProtocolState",
            "SeedPair",
        ),
        (
            "fixture/sampling-advance",
            "BmeSamplingControl",
            "BmeProtocolState",
            "AdvancePair",
        ),
        (
            "fixture/sampling-idle",
            "BmeSamplingState",
            "BmeSamplingIdleEvent",
            "IdlePair",
        ),
        (
            "fixture/sampling-feedback",
            "BmeProtocolState",
            "BmeProtocolEvent",
            "FeedbackPair",
        ),
        (
            "fixture/sampling-clock",
            "BmeSamplingClockContext",
            "ClockResult",
            "ClockPair",
        ),
    ] {
        let pair = c::tuple_info_type(vec![ty(left), ty(right)]).unwrap();
        let left_maximum = conduit_plot::maximum_prepared_canonical_value_bytes(&ty(left)).unwrap();
        let right_maximum =
            conduit_plot::maximum_prepared_canonical_value_bytes(&ty(right)).unwrap();
        let encoder =
            c::PreparedTypedTuplePairEncoder::new(ty(left), left_maximum, ty(right), right_maximum)
                .unwrap();
        assert_eq!(encoder.value_type(), &pair);
        assert!(
            encoder.maximum_encoded_bytes() <= 4096,
            "{alias} prepared pair exceeds original MCU envelope"
        );
        let extent = conduit_plot::maximum_prepared_canonical_value_bytes(&pair).unwrap();
        eprintln!("sampling paired extent {alias}: {extent}");
        assert!(
            extent <= 4096,
            "{alias} exceeds original MCU envelope: {extent}"
        );

        startup.insert_structured_type(alias, pair.clone()).unwrap();
        install(
            &mut startup,
            &mut profiles,
            name,
            &[("left", ty(left)), ("right", ty(right))],
            &[("paired", pair)],
        );
    }
    install(
        &mut startup,
        &mut profiles,
        "fixture/sampling-state",
        &[
            ("seed", ty("BmeSamplingState")),
            ("next", ty("BmeSamplingState")),
        ],
        &[("current", ty("BmeSamplingState"))],
    );
    install(
        &mut startup,
        &mut profiles,
        "fixture/sampling-merge",
        &[
            ("left", ty("BmeSamplingClockContext")),
            ("right", ty("BmeSamplingClockContext")),
        ],
        &[("merged", ty("BmeSamplingClockContext"))],
    );
    install(
        &mut startup,
        &mut profiles,
        "fixture/sampling-state-merge",
        &[
            ("left", ty("BmeSamplingState")),
            ("right", ty("BmeSamplingState")),
        ],
        &[("merged", ty("BmeSamplingState"))],
    );
    install(
        &mut startup,
        &mut profiles,
        "machine/clock/at",
        &[("request", ty("ClockRequest"))],
        &[("result", ty("ClockResult"))],
    );
    let mut wrappers = String::new();
    for (name, kind, left, right, output, pair) in [
        (
            "selected-seed",
            "SeedKind",
            "BmeSamplingBegin",
            "BmeProtocolState",
            "paired",
            "SeedPair",
        ),
        (
            "selected-advance",
            "AdvanceKind",
            "BmeSamplingControl",
            "BmeProtocolState",
            "paired",
            "AdvancePair",
        ),
        (
            "selected-idle",
            "IdleKind",
            "BmeSamplingState",
            "BmeSamplingIdleEvent",
            "paired",
            "IdlePair",
        ),
        (
            "selected-feedback",
            "FeedbackKind",
            "BmeProtocolState",
            "BmeProtocolEvent",
            "paired",
            "FeedbackPair",
        ),
        (
            "selected-clock",
            "ClockKind",
            "BmeSamplingClockContext",
            "ClockResult",
            "paired",
            "ClockPair",
        ),
        (
            "selected-state-merge",
            "StateMergeKind",
            "BmeSamplingState",
            "BmeSamplingState",
            "merged",
            "BmeSamplingState",
        ),
        (
            "selected-merge",
            "MergeKind",
            "BmeSamplingClockContext",
            "BmeSamplingClockContext",
            "merged",
            "BmeSamplingClockContext",
        ),
    ] {
        wrappers.push_str(&format!("plot {name} (\n >> left: {left}...|\n >> right: {right}...|\n {output}: {pair}...| >>\n) {{\n operation: {kind}\n left >> operation.left\n right >> operation.right\n operation.{output} >> {output}\n}}\n"));
    }
    wrappers.push_str("plot selected-state (\n >> seed: BmeSamplingState...|\n >> next: BmeSamplingState...|\n current: BmeSamplingState...| >>\n) {\n operation: StateKind\n seed >> operation.seed\n next >> operation.next\n operation.current >> current\n}\n");
    let caller = r#"
with fixture/sampling-seed as SeedKind
with fixture/sampling-advance as AdvanceKind
with fixture/sampling-idle as IdleKind
with fixture/sampling-feedback as FeedbackKind
with fixture/sampling-clock as ClockKind
with fixture/sampling-state as StateKind
with fixture/sampling-merge as MergeKind
with fixture/sampling-state-merge as StateMergeKind
plot sampling (
 >> begin: BmeSamplingBegin...|
 >> result: I2cResult...|
 request: I2cTransaction...| >>
 captured: BmeProtocolState...| >>
 observation: BmeObservationResult...| >>
 refused: BmeSamplingFailure...| >>
 clock_failed: ClockResult...| >>
) {
 sensor: bme280-sampling-exchange-selected(seed-pair-type = SeedPair, advance-pair-type = AdvancePair, idle-pair-type = IdlePair, feedback-pair-type = FeedbackPair, clock-pair-type = ClockPair, state-cell = selected-state, seed-pairing = selected-seed, advance-pairing = selected-advance, idle-pairing = selected-idle, feedback-pairing = selected-feedback, context-merge = selected-merge, state-merge = selected-state-merge, clock-pairing = selected-clock)
 begin >> sensor.begin
 result >> sensor.result
 sensor.request >> request
 sensor.captured >> captured
 sensor.observation >> observation
 sensor.refused >> refused
 sensor.clock_failed >> clock_failed
}
"#;
    let all = [caller, library.as_str(), wrappers.as_str()];
    let imports = all
        .iter()
        .flat_map(|s| s.lines())
        .filter(|l| l.starts_with("with "))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n");
    let bodies = all
        .iter()
        .flat_map(|s| s.lines())
        .filter(|l| !l.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("{imports}\n{bodies}");
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "sampling", &profiles)
        .unwrap()
        .expanded;
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|g| g.kind_id.as_str() == "machine/clock/at")
            .count(),
        1
    );
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|g| g.kind_id.as_str() == "fixture/sampling-state")
            .count(),
        1
    );
    assert_eq!(
        expanded
            .gears
            .iter()
            .filter(|g| g.kind_id.as_str() == "fixture/sampling-merge")
            .count(),
        2
    );
    assert!(!expanded
        .gears
        .iter()
        .any(|g| g.kind_id.as_str() == "machine/i2c/transact"));
}
