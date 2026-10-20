//! Genuine configured Source -> sealed Plan -> actual input-consuming kernel.
use super::*;
use crate::host_execution::scalar_tape_fixture::{NoTimer, Outputs};
use crate::{ExternalForeInput, StdHost};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_semantic_catalog::NUMERICAL_NEAR_MAX_BYTES;
use std::collections::BTreeMap;
fn prepared(limit: &str, exact: bool) -> (StdHost, Plan) {
    prepared_with_extra(limit, exact, None)
}
fn prepared_with_extra(limit: &str, exact: bool, extra: Option<&str>) -> (StdHost, Plan) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let source=format!("plot project (\n >> value: Quantity <= 1800B\n result: NearQuantity <= 16384B >>\n) {{\n boundary: math/near(limit={limit})\n value >> boundary.value\n boundary.result >> result\n}}\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "project", &profile).unwrap();
    let mut host = StdHost::new();
    host.prepare_numerical_realizations(&authored.expanded)
        .unwrap();
    if let Some(extra) = extra {
        let mut advertisement = host.advertisement().clone();
        let request = [ConfigurationEntry {
            key: "limit".into(),
            value: ConfigurationValue::Quantity(
                QuantityConfigurationValue::from_value(
                    Quantity::parse_plot_literal(extra).unwrap(),
                )
                .unwrap(),
            ),
        }];
        advertisement.capabilities.extend(offers(&request).unwrap());
        crate::normalize_capability_offers(&mut advertisement.capabilities).unwrap();
        host = StdHost::from_advertisement(advertisement).unwrap();
    }
    let hosts = [host.advertisement().clone()];
    let mut placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    if exact {
        let choice = placements.by_gear.values_mut().next().unwrap();
        choice.capability_id = hosts[0]
            .capabilities
            .iter()
            .find(|o| {
                o.kind_id.as_str() == NUMERICAL_NEAR_KIND
                    && o.implementation.execution_profile_id.as_str() == EXACT
            })
            .unwrap()
            .capability_id
            .clone();
    }
    let boundaries = [
        (PortDirection::Input, "value"),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: NUMERICAL_NEAR_MAX_BYTES,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: NUMERICAL_NEAR_MAX_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(
        plan.source_document_id,
        parse_syntax_document(&source).source_document_id()
    );
    (host, plan)
}
fn third() -> Vec<u8> {
    let q = RationalQuantity::new(
        ExactRational::ratio(1, 3, &mut ExactNumericWork::new(100_000_000)).unwrap(),
        Unit::resolve("m").unwrap(),
        QuantityRole::Linear,
    )
    .unwrap();
    let mut bytes = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(q, &mut bytes).unwrap();
    bytes.truncate(n);
    bytes
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
fn raw(value: &StructuredInfoValue) -> &[u8] {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("leaf")
    };
    bytes
}
#[test]
fn actual_third_projects_at_the_selected_back_and_exact_alternative_keeps_the_same_source() {
    let input = third();
    for exact in [false, true] {
        let (mut host, plan) = prepared("0.0001m", exact);
        let placement = &plan.fragments[0].placements[0];
        assert_eq!(
            placement.execution_profile_id.as_str(),
            if exact { EXACT } else { FLOAT }
        );
        let mut outputs = Outputs(Vec::with_capacity(1));
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "value".into(),
                    track: ConnectionTrack::Payload,
                    bytes: input.clone(),
                }],
                &mut outputs,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        assert_eq!(outputs.0.len(), 1);
        let result = StructuredInfoValue::from_canonical_bytes(&outputs.0[0]).unwrap();
        assert_eq!(raw(field(&result, "source-wire")), input);
        assert_eq!(
            Unit::decode(raw(field(&result, "unit"))).unwrap(),
            Unit::resolve("m").unwrap()
        );
        let mut work = ExactNumericWork::new(100_000_000);
        let source = MathematicalValue::decode(raw(field(&result, "source")), &mut work).unwrap();
        assert_eq!(
            source.as_rational().unwrap(),
            ExactRational::ratio(1, 3, &mut work).unwrap()
        );
        let StructuredInfoValueShape::Variant { tag, payload } =
            field(&result, "representation").shape()
        else {
            panic!("representation")
        };
        assert_eq!(tag, if exact { "exact_rational" } else { "binary32" });
        if !exact {
            assert_eq!(
                u32::from_le_bytes(raw(field(payload, "target")).try_into().unwrap()),
                0x3eaa_aaab
            );
        }
        let error = MathematicalValue::decode(raw(field(&result, "maximum-error")), &mut work)
            .unwrap()
            .as_rational()
            .unwrap();
        assert_eq!(
            error,
            if exact {
                ExactRational::ZERO
            } else {
                ExactRational::ratio(1, 100_663_296, &mut work).unwrap()
            }
        );
        let verifier =
            PreparedNearQuantityVerifier::new(&conduit_semantic_catalog::near_quantity_type())
                .unwrap();
        let tolerance = RationalQuantity::from_quantity(
            Quantity::parse_plot_literal("0.0001m").unwrap(),
            &mut work,
        )
        .unwrap();
        let digest = QuantityValueView::decode(&input, &mut work)
            .unwrap()
            .representation_digest();
        let verified = verifier
            .verify(
                &outputs.0[0],
                QUANTITY_INFO_ID,
                tolerance,
                Some(digest),
                &mut work,
            )
            .unwrap();
        assert_eq!(verified.maximum_error(), error);
        assert_eq!(verified.require_exact_target().is_ok(), exact);
        assert_eq!(
            verifier.verify(
                &outputs.0[0],
                QUANTITY_INFO_ID,
                tolerance,
                Some([0; 32]),
                &mut work
            ),
            Err(NearQuantityEvidenceRefusal::SourceMismatch)
        );
        let wrong = RationalQuantity::from_quantity(
            Quantity::parse_plot_literal("0.01Hz").unwrap(),
            &mut work,
        )
        .unwrap();
        assert_eq!(
            verifier.verify(&outputs.0[0], QUANTITY_INFO_ID, wrong, None, &mut work),
            Err(NearQuantityEvidenceRefusal::Physical)
        );
        if !exact {
            let tighter = RationalQuantity::from_quantity(
                Quantity::parse_plot_literal("0.000000001m").unwrap(),
                &mut work,
            )
            .unwrap();
            assert_eq!(
                verifier.verify(&outputs.0[0], QUANTITY_INFO_ID, tighter, None, &mut work),
                Err(NearQuantityEvidenceRefusal::InsufficientTolerance)
            );
        }
        let kernel = report.kernel.as_ref().unwrap();
        assert_eq!(kernel.post_play_start_allocations, 0);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        println!("NEAR_RUNTIME exact={exact} plan={plan:?} report={report:?} output={result:?}");
    }
}

#[test]
fn prepared_near_state_sizes_are_measured_for_driver_admission() {
    assert!(core::mem::size_of::<NearQuantityBack>() <= 64);
    println!(
        "NEAR_SIZES back={} driver={} rational={} projection={} unit={}",
        core::mem::size_of::<NearQuantityBack>(),
        core::mem::size_of::<InstalledBack>(),
        core::mem::size_of::<RationalQuantity>(),
        core::mem::size_of::<PreparedQuantityBinary32Projection>(),
        core::mem::size_of::<Unit>()
    );
}

#[test]
fn zero_limit_cannot_select_another_gears_approximate_offer() {
    let (_, plan) = prepared_with_extra("0m", false, Some("1m"));
    assert_eq!(
        plan.fragments[0].placements[0]
            .execution_profile_id
            .as_str(),
        EXACT
    );
}
#[test]
fn initialized_factory_rejects_stale_limit_and_missing_bounds_before_play() {
    let (_, plan) = prepared("0.0001m", false);
    let correct = &plan.fragments[0].placements[0];
    assert!(admitted(correct).is_ok());
    let mut stale = correct.clone();
    stale.configuration[0].value = ConfigurationValue::Quantity(
        QuantityConfigurationValue::from_value(Quantity::parse_plot_literal("1m").unwrap())
            .unwrap(),
    );
    assert!(admitted(&stale)
        .err()
        .unwrap()
        .contains("planned-realization-differs"));
    for i in 0..correct.realization_properties.len() {
        let mut missing = correct.clone();
        missing.realization_properties.remove(i);
        assert!(admitted(&missing).is_err());
    }
}

#[path = "near_fore_tests.rs"]
mod near_fore_tests;

#[path = "near_back_tests/source_dependencies.rs"]
mod source_dependencies;
