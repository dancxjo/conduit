use std::collections::{BTreeMap, BTreeSet};

use conduit_core::{
    kind_id, port_id, ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConnectionBase, HostAdvertisement, HostId, HostProfileId, ImplementationId,
    ImplementationOffer, KindContractRevision, LineId, LinkBindingId, LinkEndpointId,
    OfferGeneration, PortDescriptor, PortDirection, PortTemporal, SignId, PROTOCOL_VERSION,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};
use conduit_planner::{
    classify_diversity, plan_with_options, prove_diverse_replacement,
    select_surviving_diverse_candidate, DiversityCandidate, DiversityRefusal,
    DiversityRelationship, FactDomain, LinePathHop, MechanismDependency, PlacementChoice,
    PlacementChoices, PlanningFactKey, PlanningOptions, PreviousPlanDisposition,
};

const SOURCE: &str = "test/diversity-source";
const STAGE: &str = "test/diversity-stage";
const SINK: &str = "test/diversity-sink";
const VALUE: &str = "test/diversity-value";

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
        inputs: (kind != SOURCE)
            .then(|| port(PortDirection::Input))
            .into_iter()
            .collect(),
        outputs: (kind != SINK)
            .then(|| port(PortDirection::Output))
            .into_iter()
            .collect(),
        configuration: vec![],
    }
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    for kind in [SOURCE, STAGE, SINK] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile.insert(definition(kind)).unwrap();
    }
    (startup, profile)
}

fn checked_form() -> conduit_form::CheckedForm {
    let (startup, profile) = catalogs();
    conduit_form::parse_with_startup(
        &format!(
            "form diversity {{\n source: {SOURCE}\n first: {STAGE}\n second: {STAGE}\n sink: {SINK}\n source > first > second > sink\n}}\n"
        ),
        &startup,
        &profile,
    )
    .unwrap()
}

fn offer(definition: &KindDefinition, part: &str) -> CapabilityOffer {
    let slug = definition.kind_id.as_str().replace('/', "-");
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("{part}/{slug}")),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: definition.kind_contract_revision.clone(),
        inputs: definition.inputs.clone(),
        outputs: definition.outputs.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: format!("test/{part}-profile").into(),
            implementation_id: ImplementationId::from(format!("test/{part}/{slug}@1")),
            artifact_id: ArtifactId::from(format!("test/{part}-image@1")),
        },
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 4,
            max_queue_bytes: 256,
        },
    }
}

fn host(part: &str, kinds: &[&str]) -> HostAdvertisement {
    let (_, profile) = catalogs();
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(format!("part-{part}")),
        boot_id: BootId::from(format!("boot-{part}")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from(format!("test/{part}")),
        resources: vec![],
        capabilities: kinds
            .iter()
            .map(|kind| offer(profile.get(&kind_id(kind)).unwrap(), part))
            .collect(),
        planner_capabilities: vec![],
    }
}

fn line(
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
    name: &str,
    base: ConnectionBase,
) -> conduit_core::LineOffer {
    let mut line = conduit_signal::distributed_websocket_line_offer();
    line.line_id = LineId::from(format!("diversity/{name}"));
    line.binding.binding_id = LinkBindingId::from(format!("diversity/{name}/binding"));
    line.binding.source.host_id = source.host_id.clone();
    line.binding.source.boot_id = source.boot_id.clone();
    line.binding.source.endpoint_id = LinkEndpointId::from(format!("{name}/egress"));
    line.binding.sink.host_id = sink.host_id.clone();
    line.binding.sink.boot_id = sink.boot_id.clone();
    line.binding.sink.endpoint_id = LinkEndpointId::from(format!("{name}/ingress"));
    line.binding.base = base;
    line.binding.base_instance_id = format!("diversity/{name}/base").into();
    line.binding.limits.maximum_in_flight_items = 1;
    line.binding.limits.maximum_payload_bytes = 64;
    line.binding.limits.maximum_buffered_bytes = 64;
    line.binding.limits.maximum_frame_bytes = 64;
    line.availability.line_id = line.line_id.clone();
    line.availability.binding_id = line.binding.binding_id.clone();
    line.availability.sign_id = SignId::from(format!("diversity/{name}/ready"));
    line
}

struct Fixture {
    form: conduit_form::CheckedForm,
    hosts: Vec<HostAdvertisement>,
    lines: Vec<conduit_core::LineOffer>,
}

impl Fixture {
    fn new() -> Self {
        let a = host("a", &[SOURCE, STAGE]);
        let b = host("b", &[SINK]);
        let c = host("c", &[STAGE]);
        let d = host("d", &[STAGE]);
        let lines = vec![
            line(&a, &b, "wifi", ConnectionBase::WebSocket),
            line(&a, &c, "serial", ConnectionBase::UsbCdc),
            line(&c, &d, "optical", ConnectionBase::FixtureFrame),
            line(&d, &b, "ethernet", ConnectionBase::WebRtcDataChannel),
        ];
        Self {
            form: checked_form(),
            hosts: vec![a, b, c, d],
            lines,
        }
    }

    fn plan(&self, distributed: bool) -> conduit_core::Plan {
        let placements = PlacementChoices {
            by_gear: self
                .form
                .gears
                .iter()
                .map(|gear| {
                    let part = match gear.gear_id.as_str() {
                        "diversity/source" => "part-a",
                        "diversity/first" if distributed => "part-c",
                        "diversity/second" if distributed => "part-d",
                        "diversity/first" | "diversity/second" => "part-a",
                        "diversity/sink" => "part-b",
                        other => panic!("unexpected gear {other}"),
                    };
                    let host = self
                        .hosts
                        .iter()
                        .find(|host| host.host_id.as_str() == part)
                        .unwrap();
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
        plan_with_options(
            &self.form,
            &self.hosts,
            &placements,
            &[
                ConnectionBase::Local,
                ConnectionBase::WebSocket,
                ConnectionBase::UsbCdc,
                ConnectionBase::FixtureFrame,
                ConnectionBase::WebRtcDataChannel,
            ],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 64,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &self.lines,
            },
        )
        .unwrap()
    }
}

fn fact(domain: FactDomain, identity: &str) -> PlanningFactKey {
    PlanningFactKey::exact(domain, identity)
}

fn candidate(
    id: &str,
    rank: u64,
    plan: &conduit_core::Plan,
    mut dependencies: Vec<PlanningFactKey>,
) -> DiversityCandidate {
    let mechanisms: Vec<MechanismDependency> = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .filter(|placement| placement.kind_id.as_str() == STAGE)
        .map(|placement| MechanismDependency {
            gear_id: placement.gear_id.clone(),
            implementation_id: placement.implementation_id.clone(),
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let line_path: Vec<LinePathHop> = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .flat_map(|connection| {
            connection.admitted_lines.iter().map(|line| LinePathHop {
                connection_id: connection.connection_id.clone(),
                line_id: line.line_id.clone(),
                base_instance_id: line.binding.base_instance_id.clone(),
            })
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    dependencies.extend(mechanisms.iter().map(|mechanism| {
        fact(
            FactDomain::Implementation,
            mechanism.implementation_id.as_str(),
        )
    }));
    dependencies.extend(
        line_path
            .iter()
            .map(|hop| fact(FactDomain::Line, hop.base_instance_id.as_str())),
    );
    dependencies.sort();
    dependencies.dedup();
    DiversityCandidate {
        candidate_id: id.into(),
        semantic_capability_id: "semantic/navigation@1".into(),
        semantic_cord_id: "semantic/navigation-observation@1".into(),
        policy_rank: rank,
        critical_dependencies: dependencies,
        mechanisms,
        line_path,
    }
}

#[test]
fn damage_selects_materially_different_mechanisms_and_three_line_path() {
    let fixture = Fixture::new();
    let preferred_plan = fixture.plan(false);
    let immutable_preferred = preferred_plan.clone();
    let replacement_plan = fixture.plan(true);
    let gpu = fact(FactDomain::Resource, "resource/accelerator-a");
    let wifi = fact(FactDomain::Line, "line/wifi-a-b");
    let replacement_dependencies = vec![
        fact(FactDomain::Resource, "resource/cpu-c"),
        fact(FactDomain::Resource, "resource/cpu-d"),
        fact(FactDomain::Line, "line/serial-a-c"),
        fact(FactDomain::Line, "line/optical-c-d"),
        fact(FactDomain::Line, "line/ethernet-d-b"),
    ];
    let preferred = candidate(
        "preferred-accelerator-wifi",
        0,
        &preferred_plan,
        vec![gpu, wifi],
    );
    let replacement = candidate(
        "surviving-cpu-serial-optical-ethernet",
        50,
        &replacement_plan,
        replacement_dependencies,
    );
    let current_dependencies = replacement.critical_dependencies.clone();

    assert_eq!(preferred.line_path.len(), 1);
    assert_eq!(replacement.line_path.len(), 3);
    let selected = select_surviving_diverse_candidate(
        &preferred,
        std::slice::from_ref(&replacement),
        &current_dependencies,
    )
    .unwrap();
    assert_eq!(selected.candidate_id, replacement.candidate_id);
    let evidence = prove_diverse_replacement(
        &preferred_plan,
        &replacement_plan,
        &preferred,
        selected,
        &current_dependencies,
    )
    .unwrap();
    assert_eq!(
        evidence.relationship,
        DiversityRelationship::MechanismAndLinePathDiverse
    );
    assert_eq!(
        evidence.previous_plan_disposition,
        PreviousPlanDisposition::InvalidatedRequiresTermination
    );
    assert_eq!(evidence.replacement_line_path.len(), 3);
    assert_eq!(preferred_plan, immutable_preferred);
    assert_ne!(preferred_plan.plan_id, replacement_plan.plan_id);
    assert_eq!(
        preferred_plan.checked_form_id,
        replacement_plan.checked_form_id
    );
    assert!(conduit_core::verify_plan(&preferred_plan));
    assert!(conduit_core::verify_plan(&replacement_plan));
}

#[test]
fn shared_critical_base_and_cosmetic_wrappers_are_not_diversity() {
    let fixture = Fixture::new();
    let preferred_plan = fixture.plan(false);
    let replacement_plan = fixture.plan(true);
    let shared = fact(FactDomain::Resource, "resource/shared-power-rail");
    let preferred = candidate(
        "wrapper-a",
        0,
        &preferred_plan,
        vec![shared.clone(), fact(FactDomain::Line, "line/wifi")],
    );
    let superficially_different = candidate(
        "wrapper-b",
        1,
        &replacement_plan,
        vec![shared.clone(), fact(FactDomain::Line, "line/optical")],
    );
    assert_eq!(
        classify_diversity(&preferred, &superficially_different).unwrap(),
        DiversityRelationship::DifferentButSharedCriticalDependency
    );
    let current = superficially_different.critical_dependencies.clone();
    assert_eq!(
        select_surviving_diverse_candidate(
            &preferred,
            std::slice::from_ref(&superficially_different),
            &current,
        ),
        Err(DiversityRefusal::SharedCriticalDependency)
    );
    assert_eq!(
        prove_diverse_replacement(
            &preferred_plan,
            &replacement_plan,
            &preferred,
            &superficially_different,
            &current,
        ),
        Err(DiversityRefusal::SharedCriticalDependency)
    );

    let mut cosmetic = preferred.clone();
    cosmetic.candidate_id = "wrapper-renamed".into();
    assert_eq!(
        classify_diversity(&preferred, &cosmetic).unwrap(),
        DiversityRelationship::SameRealization
    );
}

#[test]
fn stale_missing_or_semantically_relabelled_replacements_refuse() {
    let fixture = Fixture::new();
    let preferred_plan = fixture.plan(false);
    let replacement_plan = fixture.plan(true);
    let preferred = candidate(
        "preferred",
        0,
        &preferred_plan,
        vec![fact(FactDomain::Resource, "resource/lost")],
    );
    let required = fact(FactDomain::Line, "line/current");
    let replacement = candidate("replacement", 1, &replacement_plan, vec![required.clone()]);
    assert_eq!(
        select_surviving_diverse_candidate(&preferred, std::slice::from_ref(&replacement), &[]),
        Err(DiversityRefusal::NoSurvivingCandidate)
    );
    let mut relabelled = replacement.clone();
    relabelled.semantic_cord_id = "semantic/invented@1".into();
    assert_eq!(
        classify_diversity(&preferred, &relabelled),
        Err(DiversityRefusal::SemanticIdentityChanged)
    );
    let mut unsealed = replacement.clone();
    unsealed.line_path[0].line_id = "line/not-in-plan".into();
    assert_eq!(
        prove_diverse_replacement(
            &preferred_plan,
            &replacement_plan,
            &preferred,
            &unsealed,
            &[required],
        ),
        Err(DiversityRefusal::PlanDoesNotSealCandidate)
    );
}
