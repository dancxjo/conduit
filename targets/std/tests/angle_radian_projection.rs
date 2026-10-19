//! Checked original Angle, exact symbolic law and selected installed numerical Back.
use conduit_core::*;
use conduit_plot::*;
use conduit_std_host::{ExternalForeDelivery, ExternalForeOutputAdapter, StdHost, TimerAdapter};
use std::{collections::BTreeMap, time::Duration};
const SOURCE: &str = include_str!("fixtures/angle-radian.conduit");
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("no timing effect");
    }
}
#[derive(Default)]
struct Outputs(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes.to_vec());
        }
        Ok(())
    }
}
fn authored(source: &str) -> Result<ExpandedAuthoringPlot, String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profile)?;
    let syntax = parse_syntax_document(source);
    assert_eq!(syntax.round_trip(), source);
    let checked = check_syntax_document(&syntax, &startup)
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    expand_canonical_plot_for_authoring(&checked, "radians", &profile).map_err(|e| e.message)
}
fn math_field(value: &StructuredInfoValue, name: &str) -> MathematicalValue {
    let field = field(value, name);
    let StructuredInfoValueShape::Leaf(bytes) = field.shape() else {
        panic!("math leaf");
    };
    MathematicalValue::decode(bytes, &mut ExactNumericWork::new(100_000_000)).unwrap()
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record");
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
#[test]
fn original_angle_reaches_a_certified_binary64_receipt_through_installed_plan_play() {
    let authored = authored(SOURCE).unwrap();
    let original = &SOURCE[SOURCE.find("1turn").unwrap()..][..5];
    let reference = PreparedAngleReferenceConversion::from_checked(
        Quantity::parse_plot_literal(original).unwrap(),
        Unit::Radian,
        original,
        "rad",
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    let mut host = StdHost::new();
    host.prepare_numerical_realizations(&authored.expanded)
        .unwrap();
    let mut advertisement = host.advertisement().clone();
    for gear in &authored.expanded.gears {
        if gear.kind_id.as_str() == conduit_semantic_catalog::NUMERICAL_REALIZE_KIND {
            continue;
        }
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("authored expression");
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let offer =
            conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                .unwrap();
        assert!(gear.accepts_realization(&offer));
        advertisement.capabilities.push(offer);
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let mut host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let offer = hosts[0]
        .capabilities
        .iter()
        .find(|o| {
            o.implementation.implementation_id.as_str()
                == "conduit.std/numeric-enclosed-binary64-nearest@1"
        })
        .unwrap();
    let prepared = PreparedNumericalRealization::from_property(
        &offer.realization_properties[0],
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    assert_eq!(prepared.source(), reference.denotation());
    assert_eq!(prepared.source().as_rational(), None);
    let boundaries = [(
        conduit_planner::ForeBoundaryKey {
            direction: PortDirection::Output,
            front_port_id: "result".into(),
            track: ConnectionTrack::Payload,
        },
        conduit_planner::ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: 32768,
        },
    )]
    .into_iter()
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 8192,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(plan.fragments[0].placements.len(), 2);
    assert!(plan.fragments[0]
        .placements
        .iter()
        .any(|gear| gear.implementation_id.as_str()
            == "conduit.std/numeric-enclosed-binary64-nearest@1"));
    let selected = plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.implementation_id == offer.implementation.implementation_id)
        .unwrap();
    assert_eq!(selected.artifact_id, offer.implementation.artifact_id);
    let mut output = Outputs::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[],
            &mut output,
            &mut vec![],
            &mut NoTimer,
        )
        .unwrap();
    assert!(matches!(
        report.observations.last().map(|e| &e.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    assert_eq!(output.0.len(), 1);
    let observation = StructuredInfoValue::from_canonical_bytes(&output.0[0]).unwrap();
    let StructuredInfoValueShape::Leaf(angle) = field(&observation, "angle").shape() else {
        panic!("original Angle");
    };
    assert_eq!(Quantity::decode(angle).unwrap(), reference.source());
    let receipt = field(&observation, "projection");
    let StructuredInfoValueShape::Variant { tag, payload } =
        field(receipt, "representation").shape()
    else {
        panic!("representation");
    };
    assert_eq!(tag, "binary64");
    let StructuredInfoValueShape::Variant { tag, .. } = field(payload, "error-claim").shape()
    else {
        panic!("error claim");
    };
    assert_eq!(tag, "certified_bound");
    assert_eq!(math_field(receipt, "source"), *reference.denotation());
    assert_eq!(
        math_field(receipt, "maximum-error").as_rational(),
        Some(prepared.maximum_error())
    );
    assert_eq!(math_field(receipt, "lower"), math_field(receipt, "upper"));
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    println!(
        "angle-source={:?} plan={:?} play={:?} original={} reference-law={}",
        plan.source_document_id,
        plan.plan_id,
        kernel.active_play_id,
        original,
        reference.reference_law()
    );
}
#[test]
fn startup_reference_refuses_scalars_wrong_units_and_alias_cycles() {
    for value in ["42", "1m", "1rad", "angle"] {
        let source = SOURCE.replace("angle = 1turn", &format!("angle = {value}"));
        assert!(
            authored(&source).is_err(),
            "invalid reference input {value}"
        );
    }
}
#[test]
fn impossible_precision_refuses_before_any_play_or_offer_installation() {
    let source = SOURCE.replace("maximum-error:0.000000000001", "maximum-error:0");
    let authored = authored(&source).unwrap();
    let mut host = StdHost::new();
    let before = host.advertisement().clone();
    assert!(host
        .prepare_numerical_realizations(&authored.expanded)
        .is_err());
    assert_eq!(host.advertisement(), &before);
}
