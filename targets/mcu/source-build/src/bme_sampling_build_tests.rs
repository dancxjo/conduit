//! Actual standard-family fabrication of the repeated Source sampler.
use crate::{
    composite::{Specialization as S, Value, ValueReference},
    workset,
};
use conduit_core as c;
use conduit_plot as p;
use sha2::{Digest, Sha256};

fn library() -> String {
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
        .filter(|s| s.starts_with("with "))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n");
    let body = units
        .iter()
        .flat_map(|s| s.lines())
        .filter(|s| !s.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{imports}\n{body}")
}
#[test]
fn repeated_sampling_selects_real_finite_state_and_pairing_owners() {
    let mut startup = p::StartupCatalog::new();
    let mut profiles = p::ProfileCatalog::new();
    crate::protocol_catalog::install(&mut startup, &mut profiles).unwrap();
    let library = library();
    let types = p::prepare_source_types(&p::parse_syntax_document(&library), &startup).unwrap();
    let value = |name: &str| {
        let schema = types.named_type(name).unwrap().clone();
        let maximum = p::maximum_prepared_canonical_value_bytes(&schema).unwrap();
        Value {
            contract: c::CheckedValueContract::new(
                schema.profile().unwrap().value_kind().clone(),
                maximum,
                vec![],
            )
            .unwrap(),
            schema,
        }
    };
    assert_eq!(value("BmeSamplingBegin").contract.maximum_bytes, 359);
    let entries = [
        (
            "Cell",
            "state/seeded/flow/until",
            "BmeSamplingState",
            None,
            None,
        ),
        (
            "Contexts",
            "flow/merge/finite",
            "BmeSamplingClockContext",
            None,
            None,
        ),
        (
            "Seed",
            "flow/zip/finite",
            "BmeSamplingBegin",
            Some("BmeProtocolState"),
            Some("SeedPair"),
        ),
        (
            "NextStates",
            "flow/merge/finite",
            "BmeSamplingState",
            None,
            None,
        ),
        (
            "Advance",
            "flow/zip/finite",
            "BmeSamplingControl",
            Some("BmeProtocolState"),
            Some("AdvancePair"),
        ),
        (
            "Idle",
            "flow/zip/finite",
            "BmeSamplingState",
            Some("BmeSamplingIdleEvent"),
            Some("IdlePair"),
        ),
        (
            "Feedback",
            "flow/zip/feedback",
            "BmeProtocolState",
            Some("BmeProtocolEvent"),
            Some("FeedbackPair"),
        ),
        (
            "Clock",
            "flow/zip/finite",
            "BmeSamplingClockContext",
            Some("ClockResult"),
            Some("ClockPair"),
        ),
    ];
    let mut imports = String::new();
    let mut selected = Vec::new();
    for (alias, family, left, right, pair) in entries {
        let mut values = vec![value(left)];
        if let Some(right) = right {
            values.push(value(right));
        }
        let id = c::typed_kind_instance_id(family, values.iter().map(|v| (&v.schema, &v.contract)))
            .unwrap();
        imports.push_str(&format!("with {} as {alias}\n", id.as_str()));
        if let Some(pair) = pair {
            imports.push_str(&format!("with {}/paired as {pair}\n", id.as_str()));
        }
        let reference = |name: &'static str| ValueReference {
            type_name: name,
            maximum_bytes: value(name).contract.maximum_bytes,
        };
        selected.push(match family {
            "state/seeded/flow/until" => S::SeededUntilInstance(reference(left)),
            "flow/merge/finite" => S::MergeInstance(reference(left)),
            _ => S::ZipInstance {
                left: reference(left),
                right: reference(right.unwrap()),
                feedback: family == "flow/zip/feedback",
            },
        });
    }
    let mut wrappers = String::new();
    for (name, kind, left, right, pair) in [
        (
            "seed-pair",
            "Seed",
            "BmeSamplingBegin",
            "BmeProtocolState",
            "SeedPair",
        ),
        (
            "advance-pair",
            "Advance",
            "BmeSamplingControl",
            "BmeProtocolState",
            "AdvancePair",
        ),
        (
            "idle-pair",
            "Idle",
            "BmeSamplingState",
            "BmeSamplingIdleEvent",
            "IdlePair",
        ),
        (
            "feedback-pair",
            "Feedback",
            "BmeProtocolState",
            "BmeProtocolEvent",
            "FeedbackPair",
        ),
        (
            "clock-pair",
            "Clock",
            "BmeSamplingClockContext",
            "ClockResult",
            "ClockPair",
        ),
    ] {
        wrappers.push_str(&format!("plot {name} (\n >> left: {left}...|\n >> right: {right}...|\n paired: {pair}...| >>\n) {{\n pair: {kind}\n left >> pair.left\n right >> pair.right\n pair.paired >> paired\n}}\n"));
    }
    wrappers.push_str(r#"
plot state-cell (
 >> seed: BmeSamplingState...|
 >> next: BmeSamplingState...|
 current: BmeSamplingState...| >>
) {
 cell: Cell
 seed >> cell.seed
 next >> cell.next
 cell.current >> current
}
plot context-merge (
 >> left: BmeSamplingClockContext...|
 >> right: BmeSamplingClockContext...|
 merged: BmeSamplingClockContext...| >>
) {
 merge: Contexts
 left >> merge.left
 right >> merge.right
 merge.merged >> merged
}
plot state-merge (
 >> left: BmeSamplingState...|
 >> right: BmeSamplingState...|
 merged: BmeSamplingState...| >>
) {
 merge: NextStates
 left >> merge.left
 right >> merge.right
 merge.merged >> merged
}
plot sampled-environment (
 >> begin: BmeSamplingBegin...|
 >> result: I2cResult...|
 request: I2cTransaction...| >>
 captured: BmeProtocolState...| >>
 observation: BmeObservationResult...| >>
 refused: BmeSamplingFailure...| >>
 clock_failed: ClockResult...| >>
) {
 sampler: bme280-sampling-exchange-selected(seed-pair-type = SeedPair, advance-pair-type = AdvancePair, idle-pair-type = IdlePair, feedback-pair-type = FeedbackPair, clock-pair-type = ClockPair, state-cell = state-cell, seed-pairing = seed-pair, advance-pairing = advance-pair, idle-pairing = idle-pair, feedback-pairing = feedback-pair, context-merge = context-merge, state-merge = state-merge, clock-pairing = clock-pair)
 begin >> sampler.begin
 result >> sampler.result
 sampler.request >> request
 sampler.captured >> captured
 sampler.observation >> observation
 sampler.refused >> refused
 sampler.clock_failed >> clock_failed
}
"#);
    let source = format!("{imports}\n{library}\n{wrappers}");
    let sha = format!("{:x}", Sha256::digest(source.as_bytes()));
    let workset = workset::compile_specialized(
        source.as_bytes(),
        &sha,
        &["sampled-environment".into()],
        None,
        &selected,
    )
    .unwrap();
    let entry = &workset.entries[0].plot;
    let begin = c::FrontValueLocation::Input(c::port_id("begin"));
    assert_eq!(
        entry
            .front
            .value_contracts()
            .iter()
            .find(|v| v.location == begin)
            .unwrap()
            .contract
            .maximum_bytes,
        359
    );
    for binding in entry
        .input_bindings
        .iter()
        .filter(|b| b.front_port_id.as_str() == "begin")
    {
        let gear = entry
            .expanded
            .gears
            .iter()
            .find(|g| g.gear_id == binding.gear_id)
            .unwrap();
        let input = c::FrontValueLocation::Input(binding.gear_port_id.clone());
        assert_eq!(
            gear.semantic_contract
                .value_contracts()
                .iter()
                .find(|v| v.location == input)
                .unwrap()
                .contract
                .maximum_bytes,
            359
        );
    }
    let material = crate::software::compile_with_types(
        &workset.entries[0].plot.expanded,
        Some(&workset.checked_source),
        24 * 1024,
    )
    .unwrap();
    assert_eq!(
        material
            .nodes
            .iter()
            .filter(|n| matches!(
                n.role,
                crate::software::SoftwareRole::Composite(crate::composite::Role::SeededUntil(_))
            ))
            .count(),
        1
    );
    assert_eq!(
        material
            .nodes
            .iter()
            .filter(|n| matches!(
                n.role,
                crate::software::SoftwareRole::Composite(crate::composite::Role::Merge(_))
            ))
            .count(),
        4
    );
    assert_eq!(
        workset.entries[0]
            .plot
            .expanded
            .gears
            .iter()
            .filter(|g| g.kind_id.as_str() == "machine/clock/at")
            .count(),
        1
    );
    assert!(!material.nodes.is_empty());
}
