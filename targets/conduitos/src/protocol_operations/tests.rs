//! Plan the actual Source feedback graph against retained generic native owners.
use super::*;
use alloc::{collections::BTreeMap, vec};
use conduit_core::*;
use conduit_planner::{
    ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions, default_expanded_placements,
    plan_expanded_authoring_with_options,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};

const LIFECYCLE: &str = include_str!("../../../../plots/device-protocols/bme280-lifecycle.conduit");
const FEEDBACK: &str = include_str!("../../../../plots/device-protocols/bme280-feedback.conduit");

fn planned() -> (Plan, ProtocolOperations, CapabilityOffer) {
    let contract = crate::i2c_base::contract::I2cContract::prepare().unwrap();
    let (mut startup, mut profile) = contract.catalogs();
    let types = check_syntax_document(&parse_syntax_document(LIFECYCLE), &startup).unwrap();
    let schema = |name| {
        &types
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
    };
    let state = schema("BmeProtocolState");
    let event = schema("BmeProtocolEvent");
    let value = |ty: &StructuredInfoType| {
        CheckedValueContract::new(ty.profile().unwrap().value_kind().clone(), 4096, vec![]).unwrap()
    };
    let state_value = value(state);
    let event_value = value(event);
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
    let mut owners = ProtocolOperations::default();
    let state_offer = owners.states.install_flow(&state_value, state).unwrap();
    let mut capabilities = vec![
        state_offer.clone(),
        owners
            .joins
            .install_feedback(&state_value, state, &event_value, event)
            .unwrap(),
    ];
    // Canonical imports precede definitions when these reviewed Source units are combined.
    let (import, body) = FEEDBACK.split_once('\n').unwrap();
    let source = alloc::format!("{import}\n{LIFECYCLE}\n{body}");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "bme280-feedback", &profile).unwrap();
    for gear in &authoring.expanded.gears {
        if let [entry] = gear.configuration.as_slice() {
            assert_eq!(entry.key, "program");
            let ConfigurationValue::Text(encoded) = &entry.value else {
                panic!("expression program")
            };
            capabilities.push(
                crate::expression_host_call::offer(
                    &conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
                    PortTemporal::Flow { closes: true },
                )
                .unwrap(),
            );
        }
    }
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("fixture/feedback"),
        boot_id: BootId::from("fixture/feedback-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduitos/native@1"),
        bases: vec![],
        resources: vec![],
        capabilities,
        planner_capabilities: vec![],
    }];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let boundary_limits = authoring
        .input_bindings
        .iter()
        .map(|b| (PortDirection::Input, b))
        .chain(
            authoring
                .output_bindings
                .iter()
                .map(|b| (PortDirection::Output, b)),
        )
        .map(|(direction, binding)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 4096,
                },
            )
        })
        .collect();
    let plan = plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .unwrap();
    (plan, owners, state_offer)
}

#[test]
fn source_feedback_plans_with_exact_retained_state_event_and_generation_contracts() {
    let (plan, owners, _) = planned();
    owners.states.validate_plan(&plan).unwrap();
    owners.joins.validate_plan(&plan).unwrap();
    assert_eq!(plan.fragments.len(), 1);
    assert_eq!(plan.fragments[0].placements.len(), 16);
    assert!(
        plan.fragments[0]
            .placements
            .iter()
            .all(|gear| gear.base.is_none()
                && gear.resources.is_empty()
                && gear.authority.is_empty())
    );
}

mod execution;

mod automatic_events;

mod automatic_admission;
