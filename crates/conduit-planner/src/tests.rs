use super::{
    default_placements, parse_placements, plan, plan_with_authority_grants,
    plan_with_connection_limits, plan_with_line_offers, startup_order, PlacementChoice,
    PlacementChoices, PlannerError,
};
use conduit_core::{
    authority_grant, kind_id, mandatory_sign_storage_requirement, present_authority_requirement,
    process_owned_line_offer, verify_plan, verify_plan_fragment, ArtifactId, CancellationPolicy,
    CapabilityLimits, CapabilityOffer, ConnectionBase, ExpandedFormId, GearId, HostAdvertisement,
    HostId, HostProfileId, ImplementationId, OfferGeneration, SourceDocumentId, StartupDependency,
    TerminalPolicy, PROTOCOL_VERSION,
};
use conduit_form::parse_with_startup;
use conduit_signal::{
    pico_local_advertisement, pulse_contract_revision, pulse_execution_profile,
    pulse_host_operation_requirements, pulse_outputs, pulse_resource_requirements,
    show_contract_revision, show_execution_profile, show_host_operation_requirements, show_inputs,
    show_resource_requirements, signal_profile_catalog, signal_resource_offers,
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, PICO_LOCAL_HOST_ID, PULSE_KIND, SHOW_KIND,
    SIGNAL_ENCODED_LEN, SIGNAL_PRESENTATION_KIND,
};
use std::collections::BTreeMap;

mod protected_resource_tests;

fn form() -> conduit_form::CheckedForm {
    parse_with_startup(
            "form signal-demo {\n    pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n    show: presentation/show\n\n\n    pulse > show\n}\n",
            &conduit_signal::signal_startup_catalog(),
            &signal_profile_catalog(),
        )
        .expect("form must parse")
}

fn host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("std-host-1"),
        boot_id: conduit_core::BootId::from("boot-1"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rust-std"),
        resources: signal_resource_offers("test/timer", "test/presentation", 4),
        planner_capabilities: vec![],
        capabilities: vec![
            CapabilityOffer {
                startup_parameters: conduit_signal::pulse_face_startup_parameters(),
                shorthand: None,
                capability_id: conduit_core::CapabilityId::from("pulse-1"),
                kind_id: kind_id(PULSE_KIND),
                kind_contract_revision: pulse_contract_revision(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: pulse_execution_profile(),
                    implementation_id: ImplementationId::from("std/pulse-v1"),
                    artifact_id: ArtifactId::from("test/pulse-artifact-v1"),
                },
                inputs: vec![],
                outputs: pulse_outputs(),
                host_operations: pulse_host_operation_requirements(),
                resource_requirements: pulse_resource_requirements(),
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 4,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
            CapabilityOffer {
                startup_parameters: vec![],
                shorthand: None,
                capability_id: conduit_core::CapabilityId::from("stdout-show-1"),
                kind_id: kind_id(SHOW_KIND),
                kind_contract_revision: show_contract_revision(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: show_execution_profile(),
                    implementation_id: ImplementationId::from("std/stdout-show-signal-v1"),
                    artifact_id: ArtifactId::from("test/show-artifact-v1"),
                },
                inputs: show_inputs(),
                outputs: vec![],
                host_operations: show_host_operation_requirements(),
                resource_requirements: show_resource_requirements(),
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 4,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
        ],
    }
}

#[test]
fn parses_block_placement_file() {
    let placements = parse_placements(
            "placements 0\npulse:\n    host = \"std-host-1\"\n    capability = \"pulse-1\"\nshow:\n    host = \"std-host-1\"\n    capability = \"stdout-show-1\"\n",
        )
        .expect("placements should parse");
    assert_eq!(placements.by_gear.len(), 2);
}

#[test]
fn default_placement_uses_hosts() {
    let placements = default_placements(&form(), &[host()]).expect("placements must work");
    assert_eq!(placements.by_gear.len(), 2);
}

#[test]
fn default_placement_uses_capabilities_across_hosts() {
    let mut source = host();
    source.host_id = HostId::from("std-source");
    source
        .capabilities
        .retain(|offer| offer.kind_id.as_str() == PULSE_KIND);
    let mut sink = host();
    sink.host_id = HostId::from("std-sink");
    sink.capabilities
        .retain(|offer| offer.kind_id.as_str() == SHOW_KIND);

    let placements = default_placements(&form(), &[source, sink]).expect("placements must work");
    assert_eq!(
        placements.by_gear[&GearId::from("signal-demo/pulse")].host_id,
        HostId::from("std-source")
    );
    assert_eq!(
        placements.by_gear[&GearId::from("signal-demo/show")].host_id,
        HostId::from("std-sink")
    );
}

#[test]
fn planning_binds_exact_contract_profile_and_every_port() {
    let form = form();
    let host = host();
    let placements =
        default_placements(&form, std::slice::from_ref(&host)).expect("placements must resolve");
    let plan = plan(
        &form,
        std::slice::from_ref(&host),
        &placements,
        &[ConnectionBase::Local],
    )
    .expect("exact plan resolves");
    assert_eq!(plan.source_document_id, form.source_document_id);
    assert_eq!(plan.checked_form_id, form.checked_form_id);
    assert_eq!(plan.expanded_form_id, form.expanded_form_id);
    assert!(plan.fragments.iter().all(|fragment| {
        fragment.source_document_id == form.source_document_id
            && fragment.checked_form_id == form.checked_form_id
            && fragment.expanded_form_id == form.expanded_form_id
    }));
    for placement in &plan.fragments[0].placements {
        let gear = form
            .gears
            .iter()
            .find(|gear| gear.gear_id == placement.gear_id)
            .expect("checked gear exists");
        let capability = host
            .capabilities
            .iter()
            .find(|capability| capability.capability_id == placement.capability_id)
            .expect("capability exists");
        assert_eq!(
            placement.kind_contract_revision,
            gear.kind_contract_revision
        );
        assert_eq!(
            placement.kind_contract_revision,
            capability.kind_contract_revision
        );
        assert_eq!(
            placement.execution_profile_id,
            capability.implementation.execution_profile_id
        );
        assert_eq!(placement.inputs, gear.inputs);
        assert_eq!(placement.outputs, gear.outputs);
        assert_eq!(placement.host_operations, capability.host_operations);
        assert_eq!(
            placement.resources.len(),
            capability.resource_requirements.len()
        );
        for binding in &placement.resources {
            assert!(capability.resource_requirements.iter().any(|requirement| {
                requirement.class_id == binding.class_id && requirement.units == binding.units
            }));
            assert!(host.resources.iter().any(|resource| {
                resource.pool_id == binding.pool_id && resource.class_id == binding.class_id
            }));
        }
    }
    assert!(plan.fragments[0]
        .placements
        .iter()
        .all(|placement| !placement.host_operations.is_empty()));
    let fragment = &plan.fragments[0];
    assert_eq!(
        fragment.startup_dependencies,
        vec![StartupDependency {
            prerequisite_placement_id: fragment.connections[0].sink_placement_id.clone(),
            dependent_placement_id: fragment.connections[0].source_placement_id.clone(),
        }]
    );
    assert_eq!(
        fragment.startup_order,
        vec![
            fragment.connections[0].sink_placement_id.clone(),
            fragment.connections[0].source_placement_id.clone(),
        ]
    );
    assert_eq!(
        fragment.cancellation_policy,
        CancellationPolicy::CancelAllAndRejectLateCompletion
    );
    assert_eq!(
        fragment.terminal_policy,
        TerminalPolicy::RequireAllPlacementsAndConnections
    );
    assert_eq!(
        fragment.sign_storage_budget,
        mandatory_sign_storage_requirement(&fragment.expected_sign)
            .expect("focused sign fits public budget types")
    );
}

#[test]
fn unchanged_signal_form_plans_entirely_onto_pico_local_advertisement() {
    let form = parse_with_startup(
        include_str!("../../../fixtures/forms/signal-demo.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .expect("unchanged Signal demo form must parse");
    let host = pico_local_advertisement();
    assert_eq!(host.host_id.as_str(), PICO_LOCAL_HOST_ID);

    let placements = default_placements(&form, std::slice::from_ref(&host))
        .expect("Pico advertisement covers the exact Signal form");
    assert_eq!(placements.by_gear.len(), 2);
    assert!(placements.by_gear.values().all(|choice| {
        choice.host_id == host.host_id
            && host
                .capabilities
                .iter()
                .any(|offer| offer.capability_id == choice.capability_id)
    }));

    let plan = plan_with_connection_limits(
        &form,
        std::slice::from_ref(&host),
        &placements,
        &[ConnectionBase::Local],
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        SIGNAL_ENCODED_LEN,
    )
    .expect("Signal demo must plan onto one local Pico fragment");
    assert!(verify_plan(&plan));
    assert_eq!(plan.fragments.len(), 1);

    let fragment = &plan.fragments[0];
    assert!(verify_plan_fragment(fragment));
    assert_eq!(fragment.host_id, host.host_id);
    assert_eq!(fragment.boot_id, host.boot_id);
    assert_eq!(fragment.placements.len(), 2);
    assert_eq!(fragment.connections.len(), 1);
    assert!(fragment
        .placements
        .iter()
        .all(|placement| placement.host_id == host.host_id && placement.boot_id == host.boot_id));
    assert!(fragment
        .placements
        .iter()
        .any(|placement| placement.implementation_id.as_str() == "pico-w/kernel-pulse-timer-v1"));
    assert!(fragment
        .placements
        .iter()
        .any(|placement| placement.implementation_id.as_str()
            == "pico-w/kernel-cyw43-show-signal-v1"));

    let connection = &fragment.connections[0];
    assert!(connection.selected_line.is_none());
    assert!(connection.admitted_lines.is_empty());
    assert_eq!(
        connection.item_capacity,
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS
    );
    assert_eq!(connection.byte_capacity, SIGNAL_ENCODED_LEN);

    let lowered =
        conduit_runtime::lowering::lower_plan_fragment(fragment).expect("fragment lowers");
    assert_eq!(lowered.nodes.len(), 2);
    assert_eq!(lowered.cords.len(), 1);
    assert!(lowered.remote_endpoints.is_empty());
    assert_eq!(lowered.host_operations.len(), 2);
    assert_eq!(
        lowered.cord_value_slots,
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS
    );
    assert_eq!(lowered.cord_value_bytes, SIGNAL_ENCODED_LEN);
}

#[test]
fn planning_rejects_cyclic_startup_dependencies() {
    let form = form();
    let host = host();
    let placements =
        default_placements(&form, std::slice::from_ref(&host)).expect("placements must resolve");
    let plan = plan(
        &form,
        std::slice::from_ref(&host),
        &placements,
        &[ConnectionBase::Local],
    )
    .expect("acyclic plan resolves");
    let fragment = &plan.fragments[0];
    let mut connections = fragment.connections.clone();
    let mut reverse = connections[0].clone();
    core::mem::swap(
        &mut reverse.source_placement_id,
        &mut reverse.sink_placement_id,
    );
    connections.push(reverse);
    assert_eq!(startup_order(&fragment.placements, &connections), None);
}

#[test]
fn admitted_host_input_source_breaks_only_its_runtime_response_cycle() {
    let form = form();
    let host = host();
    let placements = default_placements(&form, std::slice::from_ref(&host)).unwrap();
    let plan = plan(
        &form,
        std::slice::from_ref(&host),
        &placements,
        &[ConnectionBase::Local],
    )
    .unwrap();
    let fragment = &plan.fragments[0];
    let mut cyclic_placements = fragment.placements.clone();
    let source = cyclic_placements
        .iter_mut()
        .find(|placement| placement.gear_id.as_str() == "signal-demo/pulse")
        .unwrap();
    source.host_operations[0].maximum_input_bytes = 0;
    source.host_operations[0].maximum_output_bytes = SIGNAL_ENCODED_LEN;
    source.host_operations[0].target_kind = Some(source.outputs[0].value_kind.clone());
    let source_placement_id = source.placement_id.clone();
    let mut connections = fragment.connections.clone();
    let mut reverse = connections[0].clone();
    core::mem::swap(
        &mut reverse.source_placement_id,
        &mut reverse.sink_placement_id,
    );
    connections.push(reverse);

    let order = startup_order(&cyclic_placements, &connections)
        .expect("an exact admitted host-input source can start the response loop");
    assert_eq!(order[0], source_placement_id);
}

#[test]
fn a_self_cord_is_runtime_routing_not_a_startup_cycle() {
    let form = form();
    let host = host();
    let placements = default_placements(&form, std::slice::from_ref(&host)).unwrap();
    let plan = plan(
        &form,
        std::slice::from_ref(&host),
        &placements,
        &[ConnectionBase::Local],
    )
    .unwrap();
    let fragment = &plan.fragments[0];
    let mut self_cord = fragment.connections[0].clone();
    self_cord.sink_placement_id = self_cord.source_placement_id.clone();
    assert!(startup_order(&fragment.placements, &[self_cord]).is_some());
}

#[test]
fn planning_rejects_invalid_host_operation_requirements() {
    let form = form();
    let mut host = host();
    host.capabilities[0].host_operations[0].maximum_in_flight = 0;
    let placements =
        default_placements(&form, std::slice::from_ref(&host)).expect("placements still resolve");
    assert!(matches!(
        plan(
            &form,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionBase::Local],
        ),
        Err(PlannerError::InvalidHostOperationRequirement(_))
    ));
}

#[test]
fn planning_rejects_invalid_unavailable_ambiguous_and_exhausted_resources() {
    let form = form();

    let mut advertised = host();
    advertised.capabilities[0].resource_requirements[0].units = 0;
    let placements = default_placements(&form, std::slice::from_ref(&advertised))
        .expect("placements still resolve");
    assert!(matches!(
        plan(
            &form,
            std::slice::from_ref(&advertised),
            &placements,
            &[ConnectionBase::Local],
        ),
        Err(PlannerError::InvalidResourceContract(_))
    ));

    let mut advertised = host();
    advertised
        .resources
        .retain(|resource| resource.class_id.as_str() != conduit_core::PRESENTATION_RESOURCE_CLASS);
    let placements = default_placements(&form, std::slice::from_ref(&advertised))
        .expect("placements still resolve");
    assert!(matches!(
        plan(
            &form,
            std::slice::from_ref(&advertised),
            &placements,
            &[ConnectionBase::Local],
        ),
        Err(PlannerError::UnavailableResource(_))
    ));

    let mut advertised = host();
    advertised.resources.push(conduit_core::resource_offer(
        "zz-test/presentation",
        conduit_core::PRESENTATION_RESOURCE_CLASS,
        4,
    ));
    let placements = default_placements(&form, std::slice::from_ref(&advertised))
        .expect("placements still resolve");
    assert!(matches!(
        plan(
            &form,
            std::slice::from_ref(&advertised),
            &placements,
            &[ConnectionBase::Local],
        ),
        Err(PlannerError::InvalidResourceContract(_))
    ));

    let mut advertised = host();
    advertised.capabilities[1].resource_requirements[0].units = 5;
    let placements = default_placements(&form, std::slice::from_ref(&advertised))
        .expect("placements still resolve");
    assert!(matches!(
        plan(
            &form,
            std::slice::from_ref(&advertised),
            &placements,
            &[ConnectionBase::Local],
        ),
        Err(PlannerError::ResourceCapacityExceeded(_))
    ));

    let mut advertised = host();
    advertised.resources[1].capacity_units = u32::MAX;
    advertised.capabilities[0].resource_requirements[0].units = u32::MAX;
    advertised.capabilities[1].resource_requirements[0] =
        conduit_core::resource_requirement(conduit_core::TIMER_RESOURCE_CLASS, 1);
    let placements = default_placements(&form, std::slice::from_ref(&advertised))
        .expect("placements still resolve");
    assert!(matches!(
        plan(
            &form,
            std::slice::from_ref(&advertised),
            &placements,
            &[ConnectionBase::Local],
        ),
        Err(PlannerError::ResourceCapacityExceeded(_))
    ));
}

#[test]
fn planning_binds_exact_authority_and_rejects_missing_stale_or_ambiguous_grants() {
    let form = form();
    let mut invalid = host();
    invalid.capabilities[1].authority_requirements = vec![conduit_core::AuthorityRequirement {
        contract_id: conduit_core::AuthorityContractId::from(
            conduit_core::PRESENT_AUTHORITY_CONTRACT,
        ),
        host_operation_contract_id: conduit_core::HostOperationContractId::from(
            conduit_core::WAIT_HOST_OPERATION_CONTRACT,
        ),
        subject_kind: kind_id(SIGNAL_PRESENTATION_KIND),
    }];
    let invalid_placements = default_placements(&form, std::slice::from_ref(&invalid))
        .expect("placements resolve before authority validation");
    assert!(matches!(
        plan_with_authority_grants(
            &form,
            std::slice::from_ref(&invalid),
            &invalid_placements,
            &[ConnectionBase::Local],
            &[],
        ),
        Err(PlannerError::InvalidAuthorityContract(_))
    ));

    let mut advertised = host();
    let requirement = present_authority_requirement(kind_id(SIGNAL_PRESENTATION_KIND));
    advertised.capabilities[1].authority_requirements = vec![requirement.clone()];
    let placements = default_placements(&form, std::slice::from_ref(&advertised))
        .expect("placements resolve without implying authority");

    assert!(matches!(
        plan_with_authority_grants(
            &form,
            std::slice::from_ref(&advertised),
            &placements,
            &[ConnectionBase::Local],
            &[],
        ),
        Err(PlannerError::AuthorityGrantMissing(_))
    ));

    let stale = authority_grant(
        "grant/stale",
        &requirement,
        advertised.host_id.clone(),
        conduit_core::BootId::from("stale-boot"),
        advertised.capabilities[1].capability_id.clone(),
    );
    assert!(matches!(
        plan_with_authority_grants(
            &form,
            std::slice::from_ref(&advertised),
            &placements,
            &[ConnectionBase::Local],
            &[stale],
        ),
        Err(PlannerError::AuthorityGrantMissing(_))
    ));

    let grant = authority_grant(
        "grant/show",
        &requirement,
        advertised.host_id.clone(),
        advertised.boot_id.clone(),
        advertised.capabilities[1].capability_id.clone(),
    );
    let mut duplicate_scope = grant.clone();
    duplicate_scope.grant_id = conduit_core::AuthorityGrantId::from("grant/show-alternate");
    assert!(matches!(
        plan_with_authority_grants(
            &form,
            std::slice::from_ref(&advertised),
            &placements,
            &[ConnectionBase::Local],
            &[grant.clone(), duplicate_scope],
        ),
        Err(PlannerError::AuthorityGrantAmbiguous(_))
    ));

    let plan = plan_with_authority_grants(
        &form,
        std::slice::from_ref(&advertised),
        &placements,
        &[ConnectionBase::Local],
        std::slice::from_ref(&grant),
    )
    .expect("exact grant resolves");
    let show = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == SHOW_KIND)
        .expect("show placement exists");
    assert_eq!(show.authority.len(), 1);
    assert_eq!(show.authority[0].grant_id, grant.grant_id);
    assert_eq!(show.authority[0].host_id, advertised.host_id);
    assert_eq!(show.authority[0].boot_id, advertised.boot_id);
}

#[test]
fn planning_binds_one_exact_observed_link_and_rejects_unproven_remote_bases() {
    let form = form();
    let source = host();
    let mut sink = host();
    sink.host_id = HostId::from("remote-host");
    sink.boot_id = conduit_core::BootId::from("remote-boot");
    let hosts = [source.clone(), sink.clone()];
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                conduit_core::GearId::from("signal-demo/pulse"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: conduit_core::CapabilityId::from("pulse-1"),
                },
            ),
            (
                conduit_core::GearId::from("signal-demo/show"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: conduit_core::CapabilityId::from("stdout-show-1"),
                },
            ),
        ]),
    };
    assert!(matches!(
        plan_with_line_offers(
            &form,
            &hosts,
            &placements,
            &[ConnectionBase::FixtureFrame],
            4,
            64,
            &[],
        ),
        Err(PlannerError::LineOfferMissing(_))
    ));

    let exact = process_owned_line_offer(
        "line/source-remote",
        "link/source-remote",
        ConnectionBase::FixtureFrame,
        "fixture/frame/source-remote",
        &source,
        &sink,
        4,
        64,
    );
    let mut stale = exact.clone();
    stale.binding.sink.boot_id = conduit_core::BootId::from("stale-boot");
    assert!(matches!(
        plan_with_line_offers(&form, &hosts, &placements, &[], 4, 64, &[stale]),
        Err(PlannerError::LineOfferMissing(_))
    ));

    let mut unavailable = exact.clone();
    unavailable.availability.availability = conduit_core::LineAvailability::Unavailable;
    assert!(matches!(
        plan_with_line_offers(&form, &hosts, &placements, &[], 4, 64, &[unavailable],),
        Err(PlannerError::LineOfferUnavailable(_))
    ));

    let mut underbounded = exact.clone();
    underbounded.binding.limits.maximum_buffered_bytes = 63;
    assert!(matches!(
        plan_with_line_offers(&form, &hosts, &placements, &[], 4, 64, &[underbounded],),
        Err(PlannerError::LineOfferUnavailable(_))
    ));

    let mut alternate = exact.clone();
    alternate.line_id = conduit_core::LineId::from("line/source-remote-alternate");
    alternate.availability.line_id = alternate.line_id.clone();
    alternate.binding.binding_id =
        conduit_core::LinkBindingId::from("link/source-remote-alternate");
    alternate.availability.binding_id = alternate.binding.binding_id.clone();
    assert!(matches!(
        plan_with_line_offers(
            &form,
            &hosts,
            &placements,
            &[],
            4,
            64,
            &[exact.clone(), alternate],
        ),
        Err(PlannerError::LineOfferAmbiguous(_))
    ));

    let mut invalid = exact.clone();
    invalid.binding.base_instance_id = conduit_core::ConnectionBaseInstanceId::from("");
    assert!(matches!(
        plan_with_line_offers(&form, &hosts, &placements, &[], 4, 64, &[invalid]),
        Err(PlannerError::InvalidLineOffer(_))
    ));

    let mut invalid_credential = exact.clone();
    invalid_credential.binding.credential = conduit_core::LinkCredentialReference::Opaque(
        conduit_core::CredentialReferenceId::from(""),
    );
    assert!(matches!(
        plan_with_line_offers(
            &form,
            &hosts,
            &placements,
            &[],
            4,
            64,
            &[invalid_credential],
        ),
        Err(PlannerError::InvalidLineOffer(_))
    ));

    let mut invalid_authority = exact.clone();
    invalid_authority.binding.authority =
        conduit_core::LinkAuthorityReference::Grant(conduit_core::AuthorityGrantId::from(""));
    assert!(matches!(
        plan_with_line_offers(&form, &hosts, &placements, &[], 4, 64, &[invalid_authority],),
        Err(PlannerError::InvalidLineOffer(_))
    ));

    let mut secured = exact;
    secured.binding.credential = conduit_core::LinkCredentialReference::Opaque(
        conduit_core::CredentialReferenceId::from("credential/source-remote"),
    );
    secured.binding.authority = conduit_core::LinkAuthorityReference::Grant(
        conduit_core::AuthorityGrantId::from("grant/source-remote"),
    );
    let plan = plan_with_line_offers(
        &form,
        &hosts,
        &placements,
        &[],
        4,
        64,
        std::slice::from_ref(&secured),
    )
    .expect("an observed link, not a global base enum, resolves the remote cord");
    assert!(verify_plan(&plan));
    let connection = plan.fragments[0]
        .connections
        .first()
        .expect("remote connection exists");
    assert_eq!(connection.selected_line.as_ref(), Some(&(&secured).into()));
}

#[test]
fn planning_link_binding_mutations_change_fragment_identity() {
    let form = form();
    let source = host();
    let mut sink = host();
    sink.host_id = HostId::from("remote-host");
    sink.boot_id = conduit_core::BootId::from("remote-boot");
    let hosts = [source.clone(), sink.clone()];
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                conduit_core::GearId::from("signal-demo/pulse"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: conduit_core::CapabilityId::from("pulse-1"),
                },
            ),
            (
                conduit_core::GearId::from("signal-demo/show"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: conduit_core::CapabilityId::from("stdout-show-1"),
                },
            ),
        ]),
    };
    let link = process_owned_line_offer(
        "line/mutation",
        "link/mutation",
        ConnectionBase::FixtureFrame,
        "fixture/frame/mutation",
        &source,
        &sink,
        4,
        64,
    );
    let original = plan_with_line_offers(&form, &hosts, &placements, &[], 4, 64, &[link])
        .expect("remote plan resolves")
        .fragments[0]
        .clone();

    for field in 0..15 {
        let mut mutated = original.clone();
        let binding = &mut mutated.connections[0]
            .selected_line
            .as_mut()
            .expect("remote line exists")
            .binding;
        match field {
            0 => binding.binding_id = conduit_core::LinkBindingId::from("mutated/link"),
            1 => binding.source.host_id = HostId::from("mutated-source"),
            2 => binding.source.boot_id = conduit_core::BootId::from("mutated-source-boot"),
            3 => {
                binding.source.endpoint_id =
                    conduit_core::LinkEndpointId::from("mutated-source-endpoint")
            }
            4 => binding.sink.host_id = HostId::from("mutated-sink"),
            5 => binding.sink.boot_id = conduit_core::BootId::from("mutated-sink-boot"),
            6 => {
                binding.sink.endpoint_id =
                    conduit_core::LinkEndpointId::from("mutated-sink-endpoint")
            }
            7 => binding.base = ConnectionBase::FixtureDatagram,
            8 => {
                binding.base_instance_id =
                    conduit_core::ConnectionBaseInstanceId::from("mutated/base")
            }
            9 => {
                binding.credential = conduit_core::LinkCredentialReference::Opaque(
                    conduit_core::CredentialReferenceId::from("mutated/credential"),
                )
            }
            10 => {
                binding.authority = conduit_core::LinkAuthorityReference::Grant(
                    conduit_core::AuthorityGrantId::from("mutated/grant"),
                )
            }
            11 => binding.limits.maximum_in_flight_items += 1,
            12 => binding.limits.maximum_payload_bytes += 1,
            13 => binding.limits.maximum_buffered_bytes += 1,
            14 => binding.limits.maximum_frame_bytes += 1,
            _ => unreachable!(),
        }
        mutated.connections[0].admitted_lines[0] =
            mutated.connections[0].selected_line.clone().unwrap();
        assert!(
            !verify_plan_fragment(&mutated),
            "field {field}: every admitted binding fact is sealed"
        );
    }
}

#[test]
fn planning_verification_rejects_each_top_level_form_identity_mutation() {
    let form = form();
    let host = host();
    let placements =
        default_placements(&form, std::slice::from_ref(&host)).expect("placements must resolve");
    let original = plan(
        &form,
        std::slice::from_ref(&host),
        &placements,
        &[ConnectionBase::Local],
    )
    .expect("exact plan resolves");

    let mut source_changed = form.clone();
    source_changed.source_document_id = SourceDocumentId::from("changed-source");
    let source_plan = plan(
        &source_changed,
        std::slice::from_ref(&host),
        &placements,
        &[ConnectionBase::Local],
    )
    .expect("source-identity plan resolves");
    assert_ne!(original.plan_id, source_plan.plan_id);

    let mut checked_changed = form.clone();
    checked_changed.checked_form_id = conduit_core::CheckedFormId::from("changed-checked");
    assert!(matches!(
        plan(
            &checked_changed,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionBase::Local],
        ),
        Err(PlannerError::InvalidFormIdentity(_))
    ));

    let mut expanded_changed = form.clone();
    expanded_changed.expanded_form_id = ExpandedFormId::from("changed-expanded");
    assert!(matches!(
        plan(
            &expanded_changed,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionBase::Local],
        ),
        Err(PlannerError::InvalidFormIdentity(_))
    ));

    let mut mutated = original.clone();
    mutated.source_document_id = SourceDocumentId::from("mutated-source");
    assert!(!verify_plan(&mutated));

    let mut mutated = original.clone();
    mutated.checked_form_id = conduit_core::CheckedFormId::from("mutated-checked");
    assert!(!verify_plan(&mutated));

    let mut mutated = original;
    mutated.expanded_form_id = ExpandedFormId::from("mutated-expanded");
    assert!(!verify_plan(&mutated));
}

#[test]
fn planning_accepts_face_preserving_revision_and_rejects_face_change() {
    let form = form();
    let original_host = host();
    let placements = default_placements(&form, std::slice::from_ref(&original_host))
        .expect("placements must resolve");

    let mut mismatched_revision = original_host.clone();
    mismatched_revision.capabilities[0].kind_contract_revision =
        conduit_core::KindContractRevision::from("mutated/flow-pulse@1");
    let revised = plan(
        &form,
        std::slice::from_ref(&mismatched_revision),
        &placements,
        &[ConnectionBase::Local],
    )
    .expect("face-preserving revision is compatible");
    assert_eq!(
        revised.fragments[0].placements[0]
            .kind_contract_revision
            .as_str(),
        "mutated/flow-pulse@1"
    );

    let mut mismatched_temporal = original_host.clone();
    mismatched_temporal.capabilities[0].outputs[0].temporal = conduit_core::PortTemporal::Current;
    assert!(matches!(
        plan(
            &form,
            std::slice::from_ref(&mismatched_temporal),
            &placements,
            &[ConnectionBase::Local]
        ),
        Err(PlannerError::IncompatibleCheckedFace(_))
    ));

    let mut mismatched_ports = original_host;
    mismatched_ports.capabilities[0]
        .outputs
        .push(conduit_core::PortDescriptor {
            port_id: conduit_core::PortId::from("unexpected"),
            value_kind: kind_id("value/unexpected"),
            direction: conduit_core::PortDirection::Output,
            temporal: conduit_core::PortTemporal::Value,
        });
    assert!(matches!(
        plan(
            &form,
            std::slice::from_ref(&mismatched_ports),
            &placements,
            &[ConnectionBase::Local]
        ),
        Err(PlannerError::IncompatibleCheckedFace(_))
    ));
}

#[test]
fn planning_rejects_unknown_host() {
    let form = form();
    let placements = parse_placements(
            "placements 0\nsignal-demo/pulse:\n    host = \"missing\"\n    capability = \"pulse-1\"\nsignal-demo/show:\n    host = \"missing\"\n    capability = \"stdout-show-1\"\n",
        )
        .expect("placements should parse");
    let error = plan(&form, &[host()], &placements, &[ConnectionBase::Local])
        .expect_err("planning should fail");
    assert!(matches!(error, PlannerError::UnknownHost(_)));
}
