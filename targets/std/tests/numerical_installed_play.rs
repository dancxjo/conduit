//! Installed std numerical receipts from one checked Source and exact Plan.
use conduit_core::*;
use conduit_plot::*;
use conduit_std_host::{ExternalForeDelivery, ExternalForeOutputAdapter, StdHost, TimerAdapter};
use std::{collections::BTreeMap, time::Duration};
const SOURCE: &str = "plot numerics (\n exact: NumericalResult <= 8192B >>\n loose: NumericalResult <= 8192B >>\n) {\n precise: math/realize(request={value:0.3,maximum-error:0})\n approximate: math/realize(request={value:0.3,maximum-error:1})\n precise.result >> exact\n approximate.result >> loose\n}\n";
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("no clock effect")
    }
}
#[derive(Default)]
struct Outputs(BTreeMap<String, Vec<u8>>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            assert!(self
                .0
                .insert(value.front_port_id.as_str().to_owned(), value.bytes.to_vec())
                .is_none());
        }
        Ok(())
    }
}
#[test]
fn installed_exact_and_binary64_receipts_preserve_one_authored_denotation() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profiles).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "numerics", &profiles).unwrap();
    let mut host = StdHost::new();
    host.prepare_numerical_realizations(&authored.expanded)
        .unwrap();
    let hosts = [host.advertisement().clone()];
    let mut placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let mut expected = BTreeMap::new();
    for gear in &authored.expanded.gears {
        let precise = gear.gear_id.as_str().ends_with("/precise");
        let implementation = if precise {
            "conduit.std/numeric-rational@1"
        } else {
            "conduit.std/numeric-binary64-nearest@1"
        };
        let offer = hosts[0]
            .capabilities
            .iter()
            .find(|o| o.implementation.implementation_id.as_str() == implementation)
            .unwrap();
        placements
            .by_gear
            .get_mut(&gear.gear_id)
            .unwrap()
            .capability_id = offer.capability_id.clone();
        let prepared = PreparedNumericalRealization::from_property(
            &offer.realization_properties[0],
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
        expected.insert(
            if precise { "exact" } else { "loose" }.to_owned(),
            conduit_semantic_catalog::numerical_result_value(&prepared)
                .unwrap()
                .canonical_bytes()
                .unwrap(),
        );
    }
    let boundaries = ["exact", "loose"]
        .into_iter()
        .map(|port| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction: PortDirection::Output,
                    front_port_id: port.into(),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 8192,
                },
            )
        })
        .collect();
    let build_plan = |placements: &conduit_planner::PlacementChoices| {
        conduit_planner::plan_expanded_authoring_with_options(
            &authored,
            &hosts,
            placements,
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
    };
    let plan = build_plan(&placements).unwrap();
    let original_plan_id = plan.plan_id.clone();
    let mut relaxed = placements.clone();
    let float = hosts[0]
        .capabilities
        .iter()
        .find(|o| {
            o.implementation.implementation_id.as_str() == "conduit.std/numeric-binary64-nearest@1"
        })
        .unwrap();
    let precise = authored
        .expanded
        .gears
        .iter()
        .find(|g| g.gear_id.as_str().ends_with("/precise"))
        .unwrap();
    relaxed
        .by_gear
        .get_mut(&precise.gear_id)
        .unwrap()
        .capability_id = float.capability_id.clone();
    assert!(build_plan(&relaxed).is_err());
    assert_eq!(plan.plan_id, original_plan_id);
    assert_eq!(plan.source_document_id, checked.source_document_id);
    assert!(verify_plan(&plan));
    let mut outputs = Outputs::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[],
            &mut outputs,
            &mut vec![],
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(outputs.0, expected);
    let kernel = report.kernel.as_ref().unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );

    let mut forged = plan.fragments[0].clone();
    forged.placements[0].artifact_id = "foreign/numerical-artifact".into();
    let mut refused_output = Outputs::default();
    assert!(host
        .run_external_plot_to(forged, &[], &mut refused_output, &mut vec![], &mut NoTimer)
        .is_err());
    assert!(refused_output.0.is_empty());
    assert_ne!(outputs.0["exact"], outputs.0["loose"]);
    let exact = StructuredInfoValue::from_canonical_bytes(&outputs.0["exact"]).unwrap();
    let loose = StructuredInfoValue::from_canonical_bytes(&outputs.0["loose"]).unwrap();
    let denotation = MathematicalValue::rational(
        ExactRational::ratio(3, 10, &mut ExactNumericWork::new(100_000_000)).unwrap(),
    );
    assert_eq!(math_field(&exact, "source"), denotation);
    assert_eq!(math_field(&loose, "source"), denotation);
    assert_eq!(math_field(&exact, "lower"), denotation);
    assert_eq!(math_field(&exact, "upper"), denotation);
    assert_eq!(
        math_field(&exact, "maximum-error"),
        MathematicalValue::rational(ExactRational::ZERO)
    );
    assert_ne!(math_field(&loose, "lower"), denotation);
    assert_eq!(math_field(&loose, "lower"), math_field(&loose, "upper"));
    assert_ne!(
        math_field(&loose, "maximum-error"),
        MathematicalValue::rational(ExactRational::ZERO)
    );

    eprintln!(
        "NUMERICAL_INSTALLED_EVIDENCE source={} checked={} expanded={} plan={} play={}",
        plan.source_document_id.as_str(),
        plan.checked_plot_id.as_str(),
        plan.expanded_plot_id.as_str(),
        plan.plan_id.as_str(),
        kernel.active_play_id.as_str()
    );
}

fn math_field(value: &StructuredInfoValue, name: &str) -> MathematicalValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("receipt record")
    };
    let value = fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value();
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("mathematical leaf")
    };
    MathematicalValue::decode(bytes, &mut ExactNumericWork::new(100_000_000)).unwrap()
}
