use super::{host, installed_std, RecordingTimer};
use conduit_core::{
    BaseImplementationId, ObservationKind, Quantity, QuantityUnit, StructuredFieldType,
    StructuredFieldValue, StructuredInfoType, StructuredInfoValue,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_presentation::{PresentationPropertyValue, StructuredSignPresentation};
use std::collections::BTreeMap;

fn quantity_record() -> (StructuredInfoType, StructuredInfoValue) {
    let quantity =
        StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::QUANTITY_INFO_ID)).unwrap();
    let record = StructuredInfoType::record(
        conduit_core::kind_id("measurement/timed-tone@1"),
        vec![
            StructuredFieldType::new("elapsed", quantity.clone()).unwrap(),
            StructuredFieldType::new("frequency", quantity.clone()).unwrap(),
        ],
    )
    .unwrap();
    let value = StructuredInfoValue::record(
        record.clone(),
        vec![
            StructuredFieldValue::new(
                "elapsed",
                StructuredInfoValue::leaf(
                    quantity.clone(),
                    Quantity::new(17, QuantityUnit::Millisecond)
                        .encode()
                        .to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "frequency",
                StructuredInfoValue::leaf(
                    quantity,
                    Quantity::new(440, QuantityUnit::Hertz).encode().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    (record, value)
}

fn catalogs() -> (
    StartupCatalog,
    ProfileCatalog,
    StructuredInfoType,
    StructuredInfoValue,
) {
    let (value_type, default) = quantity_record();
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_std_catalog::install_structured_value_catalogs(
        "TimedTone",
        &value_type,
        &default,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    (startup, profile, value_type, default)
}

#[test]
fn authored_quantities_survive_plan_play_sign_and_typed_presentation() {
    let (startup, profile, value_type, _) = catalogs();
    let source = "form quantity-proof {\n    value: structured-info/literal(value = {elapsed: 17ms, frequency: 440Hz})\n    show: presentation/structured-info\n    value > show\n}\n";
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).expect("quantity Form checks");
    let expanded =
        expand_canonical_form(&checked, "quantity-proof", &profile).expect("quantity Form expands");

    let literal = conduit_std_catalog::structured_literal_std_offer("TimedTone", &value_type);
    let presenter =
        conduit_std_catalog::structured_presentation_std_offer("TimedTone", &value_type);
    let mut advertisement = host("structured-quantity-host").advertisement().clone();
    advertisement.capabilities.extend([literal, presenter]);
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)
        .expect("exact structured quantity capabilities place");
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("structured quantity Form plans");

    let mut output = Vec::with_capacity(2_048);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let mut sign_sequence = 0;
    let report = installed_std::run_fragment(
        installed_std::InstalledRunHost {
            advertisement: &advertisement,
            playback: None,
            midi_input: None,
            midi_output: None,
            keyboard: None,
            local_model: None,
            vector_search: None,
            calendar: None,
        },
        &plan.fragments[0],
        0,
        &mut sign_sequence,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .expect("structured quantities execute in the production kernel");
    let presented = report
        .observations
        .iter()
        .find(|observation| matches!(observation.kind, ObservationKind::ValuePresented { .. }))
        .expect("runtime emits one correlated value presentation Sign");
    assert!(presented.active_play_id.is_some());
    assert!(presented.presentation_id.is_some());
    assert!(presented.plan_id.is_some());
    assert!(presented.placement_id.is_some());
    assert!(presented.connection_id.is_some());
    let artifact = StructuredSignPresentation::from_sign(1, presented, &value_type)
        .expect("typed Presentation inspects the runtime Sign");
    assert!(artifact.presentation.properties.iter().any(|property| {
        property.name == "quantity-unit"
            && property.value == PresentationPropertyValue::Identity("time/millisecond".into())
    }));
    assert!(artifact.presentation.properties.iter().any(|property| {
        property.name == "quantity-unit"
            && property.value == PresentationPropertyValue::Identity("frequency/hertz".into())
    }));
    assert!(artifact.presentation.text.is_empty());
    let kernel = report.kernel.expect("kernel evidence exists");
    assert_eq!(kernel.presentation_ids.len(), 1);
    assert_eq!(kernel.identity.lengths(), (1, 1, 2));
    assert_eq!(kernel.post_play_start_allocations, 0);
}

#[test]
fn malformed_or_untyped_quantity_literals_refuse_before_play() {
    let (startup, _, _, _) = catalogs();
    for authored in [
        "{elapsed: 17fortnights, frequency: 440Hz}",
        "{elapsed: 17, frequency: 440Hz}",
    ] {
        let source = format!(
            "form refusal {{\n value: structured-info/literal(value = {authored})\n show: presentation/structured-info\n value > show\n}}\n"
        );
        let syntax = parse_syntax_document(&source);
        assert!(
            !syntax.diagnostics.is_empty() || check_syntax_document(&syntax, &startup).is_err(),
            "untyped or unknown-unit literal unexpectedly checked: {authored}"
        );
    }
}
