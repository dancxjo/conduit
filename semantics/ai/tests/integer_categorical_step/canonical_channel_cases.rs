//! Actual canonical Source reaches the existing categorical submit/output-ACK path.
//! This is a generic numerical bridge; it grants no Language or Current authority.
use super::*;
use conduit_ai::integer_categorical_step::channel::*;
use conduit_composite::KernelCompositeDefinition;
use conduit_plot::{CheckedSyntaxDocument, ProfileCatalog};
use std::collections::BTreeMap;

const ENTRY: &str = "canonical-categorical-channel";

fn canonical_definition(
    profile: &PreparedCategoricalStep,
) -> (
    CheckedSyntaxDocument,
    ProfileCatalog,
    KernelCompositeDefinition,
) {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut catalog = ProfileCatalog::new();
    profile.install(&mut startup, &mut catalog, true).unwrap();
    let kind = profile.kind_identity(true);
    let input_max =
        conduit_plot::maximum_prepared_canonical_value_bytes(profile.indices_type()).unwrap();
    let output_max =
        conduit_plot::maximum_prepared_canonical_value_bytes(profile.scores_type()).unwrap();
    let source = format!(
        "plot {ENTRY} (\n >> input: {kind}/indices...| <= {input_max}B\n output: {kind}/scores...| <= {output_max}B >>\n) {{\n model: {kind}\n input >> model.indices\n model.scores >> output\n}}\n"
    );
    let document = conduit_plot::check_syntax_document_with_literal_constructors(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
        &catalog,
    )
    .unwrap();
    let authored =
        conduit_plot::expand_canonical_plot_for_authoring(&document, ENTRY, &catalog).unwrap();
    // Named structured types determine the value Kind; explicit Source bounds
    // retain the selected Back's checked payload contract as a separate fact.
    let selected = profile.contract(true).unwrap();
    let selected_front = selected.checked_front();
    assert_eq!(
        authored
            .front
            .value_contract(&FrontValueLocation::Input(port_id("input"))),
        selected_front.value_contract(&FrontValueLocation::Input(port_id("indices"))),
    );
    assert_eq!(
        authored
            .front
            .value_contract(&FrontValueLocation::Output(port_id("output"))),
        selected_front.value_contract(&FrontValueLocation::Output(port_id("scores"))),
    );
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "categorical-test".into(),
        boot_id: "categorical-test-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "test/host@1".into(),
        bases: vec![],
        resources: vec![profile.resource_offer().clone()],
        planner_capabilities: vec![],
        capabilities: vec![profile.offer(true).unwrap()],
    }];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let maximum = input_max.max(output_max);
    let bounds = [
        (PortDirection::Input, "input"),
        (PortDirection::Output, "output"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port_id(name),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: maximum,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: maximum,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &bounds,
    )
    .unwrap();
    let definition = KernelCompositeDefinition::from_canonical_export(
        "canonical-categorical-export".into(),
        "canonical-categorical-export-boot".into(),
        OfferGeneration(1),
        "categorical/channel@1".into(),
        "categorical/channel@1".into(),
        "categorical/channel-artifact@1".into(),
        &document,
        ENTRY,
        &catalog,
        plan,
        FailureReason::CompositeCapabilityFailed,
    )
    .unwrap();
    (document, catalog, definition)
}

#[test]
fn canonical_channel_returns_exact_lease_plan_play_and_bytes_after_real_kernel_ack() {
    let profile = profile();
    let (document, catalog, definition) = canonical_definition(&profile);
    let mut channel = PreparedCategoricalChannel::prepare_canonical(
        &document,
        ENTRY,
        &catalog,
        definition.clone(),
        profile.clone(),
        1,
    )
    .unwrap();
    let plays = channel.active_plays().clone();
    assert_eq!(plays.len(), 1);
    let input = indices(&profile, &[1, 1]);
    // Move a non-Copy lease through the ordinary request/ACK route.
    let lease = String::from("canonical-caller/exact-request");
    assert_eq!(
        channel.submit(lease, &input).unwrap(),
        CategoricalChannelSubmission::Accepted
    );
    assert_eq!(
        channel.pending().map(String::as_str),
        Some("canonical-caller/exact-request")
    );
    let mut completed = None;
    for _ in 0..16 {
        if let Some(receipt) = channel.step().unwrap() {
            completed = Some(receipt);
            break;
        }
    }
    let (lease, receipt) = completed
        .expect("actual kernel output ACK within finite turns")
        .into_parts();
    assert_eq!(lease, "canonical-caller/exact-request");
    assert!(channel.pending().is_none());
    assert_eq!(receipt.invocation(), 1);
    assert_eq!(receipt.plan(), &definition.internal_plan);
    assert_eq!(receipt.active_plays(), &plays);
    assert_eq!(channel.active_plays(), &plays);
    let mut retained_input = vec![0; receipt.input_len()];
    receipt.copy_input_into(&mut retained_input).unwrap();
    assert_eq!(retained_input, input);
    let mut output = vec![0; receipt.output_len()];
    receipt.copy_output_into(&mut output).unwrap();
    assert_eq!(scores(&output), [4, -4]);
    let StructuredInfoTypeShape::Collection { element, .. } = profile.scores_type().shape() else {
        panic!("categorical scores are a checked collection");
    };
    let expected = StructuredInfoValue::collection(
        profile.scores_type().clone(),
        [4_i64, -4]
            .into_iter()
            .map(|score| {
                StructuredInfoValue::leaf(element.clone(), score.to_le_bytes().to_vec()).unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    assert_eq!(output, expected);
    let signs = channel.signs();
    for kind in [
        conduit_kernel::KernelEventKind::RemoteInputAdmitted,
        conduit_kernel::KernelEventKind::RemoteValueOffered,
        conduit_kernel::KernelEventKind::RemoteValueAccepted,
        conduit_kernel::KernelEventKind::RemoteValueDelivered,
    ] {
        assert_eq!(
            signs
                .values()
                .flatten()
                .filter(|event| event.kind == kind)
                .count(),
            1
        );
    }
    let pressure = channel
        .submit(String::from("second-caller"), &input)
        .unwrap_err();
    assert_eq!(pressure.refusal, CategoricalChannelRefusal::Pressure);
    assert_eq!(pressure.lease, "second-caller");
    assert!(!channel.is_closed());
}

#[test]
fn canonical_channel_rebuild_refuses_foreign_source_and_misbound_definition() {
    let profile = profile();
    let (document, catalog, definition) = canonical_definition(&profile);
    let mut foreign = document.clone();
    foreign.source_document_id = "source/foreign-canonical-document".into();
    assert!(matches!(
        PreparedCategoricalChannel::<()>::prepare_canonical(
            &foreign,
            ENTRY,
            &catalog,
            definition.clone(),
            profile.clone(),
            1,
        ),
        Err(CategoricalChannelRefusal::Definition)
    ));
    let mut misbound = definition;
    misbound.boundary.output_fronts[0].internal_port_id = port_id("indices");
    assert!(matches!(
        PreparedCategoricalChannel::<()>::prepare_canonical(
            &document, ENTRY, &catalog, misbound, profile, 1,
        ),
        Err(CategoricalChannelRefusal::Definition)
    ));
}
