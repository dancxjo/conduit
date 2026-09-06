use std::collections::BTreeMap;

use conduit_core::{
    authority_grant, kind_id, port_id, ArtifactId, AuthorityContractId, AuthorityGrant,
    AuthorityRequirement, BaseImplementationId, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, HostAdvertisement, HostId, HostOperationContractId, HostOperationRequirement,
    HostProfileId, ImplementationId, KindContractRevision, LineId, LinkBindingId, LinkEndpointId,
    OfferGeneration, PortDescriptor, PortDirection, PortTemporal, ResourceClassId, ResourceHealth,
    ResourceObservation, ResourceOffer, ResourcePoolId, ResourceRequirement, SignId,
    PROTOCOL_VERSION,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};
use conduit_planner::{
    observe_dormant_candidate, plan_with_options, prove_dormant_readmission,
    DormantEquipmentHistory, DormantReadmissionRefusal, PlacementChoice, PlacementChoices,
    PlanningOptions, RequiredDormantLine,
};

const SOURCE: &str = "test/dormant-source";
const SINK: &str = "test/dormant-sink";
const VALUE: &str = "test/dormant-value";
const CPU: &str = "test/resource/cpu";
const OPERATION: &str = "test/operation/observe@1";
const AUTHORITY: &str = "test/authority/observe@1";

fn port(direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(match direction {
            PortDirection::Input => "in",
            PortDirection::Output => "out",
        }),
        value_kind: kind_id(VALUE),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn definition(kind: &str) -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(format!("{kind}@1")),
        inputs: (kind == SINK)
            .then(|| port(PortDirection::Input))
            .into_iter()
            .collect(),
        outputs: (kind == SOURCE)
            .then(|| port(PortDirection::Output))
            .into_iter()
            .collect(),
        configuration: vec![],
    }
}

fn form() -> conduit_form::CheckedForm {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    for kind in [SOURCE, SINK] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile.insert(definition(kind)).unwrap();
    }
    conduit_form::parse_with_startup(
        &format!("form dormant {{\n source: {SOURCE}\n sink: {SINK}\n source > sink\n}}\n"),
        &startup,
        &profile,
    )
    .unwrap()
}

fn offer(kind: &str, host: &str) -> CapabilityOffer {
    let definition = definition(kind);
    let sink = kind == SINK;
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("{host}/{}", kind.replace('/', "-"))),
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: format!("test/{host}-profile").into(),
            implementation_id: ImplementationId::from(format!("test/{host}/{kind}@1")),
            artifact_id: ArtifactId::from(format!("test/{host}-image@1")),
        },
        host_operations: sink
            .then(|| HostOperationRequirement {
                contract_id: HostOperationContractId::from(OPERATION),
                target_kind: Some(kind_id(SINK)),
                maximum_in_flight: 1,
                maximum_input_bytes: 64,
                maximum_output_bytes: 64,
            })
            .into_iter()
            .collect(),
        resource_requirements: sink
            .then(|| ResourceRequirement {
                content: None,
                class_id: ResourceClassId::from(CPU),
                units: 1,
                protected_role: None,
                compute: None,
            })
            .into_iter()
            .collect(),
        authority_requirements: sink
            .then(|| AuthorityRequirement {
                contract_id: AuthorityContractId::from(AUTHORITY),
                host_operation_contract_id: HostOperationContractId::from(OPERATION),
                subject_kind: kind_id(SINK),
            })
            .into_iter()
            .collect(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: 128,
        },
    }
}

fn host(name: &str, boot: &str, generation: u64, kinds: &[&str]) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(name),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(generation),
        profile: HostProfileId::from(format!("test/{name}")),
        resources: kinds
            .contains(&SINK)
            .then(|| ResourceOffer {
                content: None,
                pool_id: ResourcePoolId::from(format!("{name}/cpu")),
                class_id: ResourceClassId::from(CPU),
                capacity_units: 2,
                compute: None,
            })
            .into_iter()
            .collect(),
        capabilities: kinds.iter().map(|kind| offer(kind, name)).collect(),
        planner_capabilities: vec![],
    }
}

fn line(
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
    name: &str,
    base: BaseImplementationId,
) -> conduit_core::LineOffer {
    let mut line = conduit_signal_conformance::distributed_websocket_line_offer();
    line.line_id = LineId::from(format!("dormant/{name}"));
    line.binding.binding_id = LinkBindingId::from(format!("dormant/{name}/binding"));
    line.binding.source.host_id = source.host_id.clone();
    line.binding.source.boot_id = source.boot_id.clone();
    line.binding.source.endpoint_id = LinkEndpointId::from(format!("{name}/out"));
    line.binding.sink.host_id = sink.host_id.clone();
    line.binding.sink.boot_id = sink.boot_id.clone();
    line.binding.sink.endpoint_id = LinkEndpointId::from(format!("{name}/in"));
    line.binding.base = base;
    line.binding.base_instance_id = format!("dormant/{name}/base").into();
    line.binding.limits.maximum_in_flight_items = 1;
    line.binding.limits.maximum_payload_bytes = 64;
    line.binding.limits.maximum_buffered_bytes = 64;
    line.binding.limits.maximum_frame_bytes = 64;
    line.availability.line_id = line.line_id.clone();
    line.availability.binding_id = line.binding.binding_id.clone();
    line.availability.sign_id = SignId::from(format!("dormant/{name}/ready"));
    line
}

fn observation(host: &HostAdvertisement, sign: &str) -> ResourceObservation {
    let pool = &host.resources[0];
    ResourceObservation {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        pool_id: pool.pool_id.clone(),
        class_id: pool.class_id.clone(),
        health: ResourceHealth::Ready,
        unreserved_units: 2,
        utilized_units: 0,
        sign_id: SignId::from(sign),
    }
}

fn grant(host: &HostAdvertisement) -> AuthorityGrant {
    let offer = host
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == SINK)
        .unwrap();
    let grant_id = format!("grant/{}/fresh", host.host_id.as_str());
    authority_grant(
        &grant_id,
        &offer.authority_requirements[0],
        host.host_id.clone(),
        host.boot_id.clone(),
        offer.capability_id.clone(),
    )
}

struct OtherTruth<'a> {
    hosts: &'a [HostAdvertisement],
    lines: &'a [conduit_core::LineOffer],
    grants: &'a [AuthorityGrant],
}

fn plan(
    checked: &conduit_form::CheckedForm,
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
    line: &conduit_core::LineOffer,
    grant: &AuthorityGrant,
    other: OtherTruth<'_>,
) -> conduit_core::Plan {
    let placements = PlacementChoices {
        by_gear: checked
            .gears
            .iter()
            .map(|gear| {
                let host = if gear.kind_id.as_str() == SOURCE {
                    source
                } else {
                    sink
                };
                let capability = host
                    .capabilities
                    .iter()
                    .find(|offer| offer.checked_face() == gear.checked_face())
                    .unwrap();
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: capability.capability_id.clone(),
                    },
                )
            })
            .collect(),
    };
    let mut hosts = vec![source.clone(), sink.clone()];
    hosts.extend_from_slice(other.hosts);
    let mut lines = vec![line.clone()];
    lines.extend_from_slice(other.lines);
    let mut grants = vec![grant.clone()];
    grants.extend_from_slice(other.grants);
    plan_with_options(
        checked,
        &hosts,
        &placements,
        &[
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 64,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &lines,
        },
    )
    .unwrap()
}

fn history() -> DormantEquipmentHistory {
    DormantEquipmentHistory {
        body_membership_id: "body/household/slow-laptop".into(),
        host_id: HostId::from("host-slow"),
        last_observed_boot_id: BootId::from("boot-slow-old"),
        last_offer_generation: OfferGeneration(1),
        absent_planning_generations: vec![2, 3, 4],
        last_selected_plan_id: None,
    }
}

#[test]
fn unused_host_returns_only_through_fresh_truth_and_ordinary_plan() {
    let checked = form();
    let source = host("host-source", "boot-source", 1, &[SOURCE]);
    let preferred = host("host-fast", "boot-fast", 1, &[SINK]);
    let dormant = host("host-slow", "boot-slow-fresh", 5, &[SINK]);
    let old_dormant = host("host-slow", "boot-slow-old", 1, &[SINK]);
    let preferred_line = line(
        &source,
        &preferred,
        "fast-wifi",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
    );
    let returned_line = line(
        &source,
        &dormant,
        "slow-serial",
        BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
    );
    let old_dormant_line = line(
        &source,
        &old_dormant,
        "slow-serial-old",
        BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
    );
    let preferred_plan = plan(
        &checked,
        &source,
        &preferred,
        &preferred_line,
        &grant(&preferred),
        OtherTruth {
            hosts: std::slice::from_ref(&old_dormant),
            lines: std::slice::from_ref(&old_dormant_line),
            grants: &[grant(&old_dormant)],
        },
    );
    let immutable_preferred = preferred_plan.clone();
    let returned_plan = plan(
        &checked,
        &source,
        &dormant,
        &returned_line,
        &grant(&dormant),
        OtherTruth {
            hosts: &[],
            lines: &[],
            grants: &[],
        },
    );
    let requirement = RequiredDormantLine {
        line_id: returned_line.line_id.clone(),
        contract: returned_line.contract,
    };
    let candidate = observe_dormant_candidate(
        checked
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str() == SINK)
            .unwrap(),
        &history(),
        &dormant,
        &[observation(&dormant, "sign/slow-cpu-fresh")],
        std::slice::from_ref(&requirement),
        std::slice::from_ref(&returned_line),
        &[grant(&dormant)],
    )
    .unwrap();
    let evidence = prove_dormant_readmission(&preferred_plan, &returned_plan, candidate).unwrap();
    assert!(evidence.candidate.unused_before);
    assert!(evidence.candidate.available_now);
    assert!(evidence.selected_because_preferred_path_is_gone);
    assert!(!evidence.historical_boot_reused);
    assert!(!evidence.historical_authority_restored);
    assert_eq!(evidence.candidate.boot_id.as_str(), "boot-slow-fresh");
    assert_eq!(evidence.candidate.offer_generation, OfferGeneration(5));
    assert_eq!(preferred_plan, immutable_preferred);
    assert!(preferred_plan.fragments.iter().all(|fragment| {
        fragment.host_id != old_dormant.host_id
            && fragment
                .connections
                .iter()
                .flat_map(|connection| &connection.admitted_lines)
                .all(|line| line.line_id != old_dormant_line.line_id)
    }));
    assert_ne!(preferred_plan.plan_id, returned_plan.plan_id);
    assert_eq!(
        preferred_plan.checked_form_id,
        returned_plan.checked_form_id
    );
    assert!(conduit_core::verify_plan(&preferred_plan));
    assert!(conduit_core::verify_plan(&returned_plan));
}

#[test]
fn stale_history_resource_line_authority_and_revisions_refuse_specifically() {
    let checked = form();
    let gear = checked
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == SINK)
        .unwrap();
    let source = host("host-source", "boot-source", 1, &[SOURCE]);
    let current = host("host-slow", "boot-slow-fresh", 5, &[SINK]);
    let current_line = line(
        &source,
        &current,
        "slow-serial",
        BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
    );
    let requirement = RequiredDormantLine {
        line_id: current_line.line_id.clone(),
        contract: current_line.contract,
    };
    let observe = |host: &HostAdvertisement,
                   resource: ResourceObservation,
                   line: conduit_core::LineOffer,
                   grants: &[AuthorityGrant]| {
        observe_dormant_candidate(
            gear,
            &history(),
            host,
            &[resource],
            std::slice::from_ref(&requirement),
            &[line],
            grants,
        )
    };

    let mut stale_boot = current.clone();
    stale_boot.boot_id = BootId::from("boot-slow-old");
    assert_eq!(
        observe(
            &stale_boot,
            observation(&stale_boot, "resource/stale-boot"),
            line(
                &source,
                &stale_boot,
                "slow-serial",
                BaseImplementationId::from("conduit.base/usb-cdc-acm@1")
            ),
            &[grant(&stale_boot)],
        ),
        Err(DormantReadmissionRefusal::StaleBoot)
    );

    let mut incompatible_protocol = current.clone();
    incompatible_protocol.protocol_version = PROTOCOL_VERSION + 1;
    assert_eq!(
        observe(
            &incompatible_protocol,
            observation(&incompatible_protocol, "resource/future-protocol"),
            line(
                &source,
                &incompatible_protocol,
                "slow-serial",
                BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
            ),
            &[grant(&incompatible_protocol)],
        ),
        Err(DormantReadmissionRefusal::IncompatibleHostProtocol)
    );

    let mut stale_generation = current.clone();
    stale_generation.offer_generation = OfferGeneration(1);
    assert_eq!(
        observe(
            &stale_generation,
            observation(&stale_generation, "resource/stale-generation"),
            line(
                &source,
                &stale_generation,
                "slow-serial",
                BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
            ),
            &[grant(&stale_generation)],
        ),
        Err(DormantReadmissionRefusal::StaleOfferGeneration)
    );

    let mut stale_resource = observation(&current, "resource/old");
    stale_resource.boot_id = BootId::from("boot-slow-old");
    assert_eq!(
        observe(
            &current,
            stale_resource,
            current_line.clone(),
            &[grant(&current)],
        ),
        Err(DormantReadmissionRefusal::StaleCurrentResourceObservation)
    );

    let mut stale_line = current_line.clone();
    stale_line.binding.sink.boot_id = BootId::from("boot-slow-old");
    assert_eq!(
        observe(
            &current,
            observation(&current, "resource/current"),
            stale_line,
            &[grant(&current)],
        ),
        Err(DormantReadmissionRefusal::StaleCurrentLine)
    );

    assert_eq!(
        observe(
            &current,
            observation(&current, "resource/current"),
            current_line.clone(),
            &[],
        ),
        Err(DormantReadmissionRefusal::MissingCurrentAuthority)
    );

    let historical_host = host("host-slow", "boot-slow-old", 1, &[SINK]);
    assert_eq!(
        observe(
            &current,
            observation(&current, "resource/current"),
            current_line.clone(),
            &[grant(&historical_host)],
        ),
        Err(DormantReadmissionRefusal::MissingCurrentAuthority)
    );

    let mut incompatible = current.clone();
    incompatible.capabilities[0].kind_contract_revision =
        KindContractRevision::from("test/dormant-sink@obsolete");
    assert_eq!(
        observe(
            &incompatible,
            observation(&incompatible, "resource/current"),
            line(
                &source,
                &incompatible,
                "slow-serial",
                BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
            ),
            &[grant(&incompatible)],
        ),
        Err(DormantReadmissionRefusal::IncompatibleContractRevision)
    );

    let mut incompatible_line = current_line;
    incompatible_line.contract.reliability = conduit_core::LineReliability::BestEffort;
    assert_eq!(
        observe(
            &current,
            observation(&current, "resource/current"),
            incompatible_line,
            &[grant(&current)],
        ),
        Err(DormantReadmissionRefusal::IncompatibleLineContract)
    );
}
