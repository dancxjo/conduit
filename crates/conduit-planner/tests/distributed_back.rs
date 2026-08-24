use std::collections::BTreeMap;

use conduit_core::{
    kind_id, port_id, ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConnectionBase, HostAdvertisement, HostId, HostProfileId, ImplementationId,
    ImplementationOffer, KindContractRevision, LineId, LinkBindingId, LinkEndpointId,
    OfferGeneration, PortDescriptor, PortDirection, PortTemporal, SignId, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, expand_canonical_form_with_backs,
    parse_syntax_document, CanonicalBackCatalog, KindDefinition, KindSignature, ProfileCatalog,
    StartupCatalog,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_options, PlacementChoice,
    PlacementChoices, PlanningOptions,
};

#[path = "distributed_back/execution.rs"]
mod execution;
#[path = "distributed_back/recovery.rs"]
mod recovery;
#[path = "distributed_back/survival_policy.rs"]
mod survival_policy;

const VALUES: [&str; 6] = [
    "test/provider-prompt",
    "test/provider-request-value",
    "test/provider-json-text",
    "test/provider-http-response",
    "test/provider-json-value",
    "test/provider-result-value",
];
const HIGH: &str = "test/provider-generate";
const SOURCE: &str = "test/provider-source";
const REQUEST: &str = "test/provider-request";
const ENCODE: &str = "test/json-encode";
const HTTP: &str = "test/provider-http-client";
const DECODE: &str = "test/json-decode";
const RESULT: &str = "test/provider-result";
const SINK: &str = "test/provider-sink";

fn port(name: &str, value: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn definition(kind: &str, input: Option<&str>, output: Option<&str>) -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(format!("{kind}@1")),
        inputs: input
            .map(|value| port("in", value, PortDirection::Input))
            .into_iter()
            .collect(),
        outputs: output
            .map(|value| port("out", value, PortDirection::Output))
            .into_iter()
            .collect(),
        configuration: vec![],
    }
}

fn catalogs() -> (StartupCatalog, ProfileCatalog, KindDefinition) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    let mut high = None;
    for (kind, input, output) in [
        (SOURCE, None, Some(VALUES[0])),
        (HIGH, Some(VALUES[0]), Some(VALUES[5])),
        (REQUEST, Some(VALUES[0]), Some(VALUES[1])),
        (ENCODE, Some(VALUES[1]), Some(VALUES[2])),
        (HTTP, Some(VALUES[2]), Some(VALUES[3])),
        (DECODE, Some(VALUES[3]), Some(VALUES[4])),
        (RESULT, Some(VALUES[4]), Some(VALUES[5])),
        (SINK, Some(VALUES[5]), None),
    ] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        let item = definition(kind, input, output);
        if kind == HIGH {
            high = Some(item.clone());
        }
        profile.insert(item).unwrap();
    }
    (startup, profile, high.unwrap())
}

fn expanded() -> conduit_form::ExpandedCanonicalForm {
    let (startup, profile, high) = catalogs();
    let user = check_syntax_document(
        &parse_syntax_document(&format!(
            "form distributed {{\n source: {SOURCE}\n generate: {HIGH}\n sink: {SINK}\n source > generate > sink\n}}\n"
        )),
        &startup,
    )
    .unwrap();
    let back = check_syntax_document(
        &parse_syntax_document(&format!(
            "form {HIGH} (\n in: {} > out: {}\n) {{\n request: {REQUEST}\n encode: {ENCODE}\n http: {HTTP}\n decode: {DECODE}\n result: {RESULT}\n in > request > encode > http > decode > result > out\n}}\n",
            VALUES[0], VALUES[5]
        )),
        &startup,
    )
    .unwrap();
    let mut backs = CanonicalBackCatalog::new();
    backs.insert(&high, &back, HIGH).unwrap();
    expand_canonical_form_with_backs(&user, "distributed", &profile, &backs).unwrap()
}

fn direct_expanded() -> conduit_form::ExpandedCanonicalForm {
    let (startup, profile, _) = catalogs();
    let user = check_syntax_document(
        &parse_syntax_document(&format!(
            "form distributed {{\n source: {SOURCE}\n generate: {HIGH}\n sink: {SINK}\n source > generate > sink\n}}\n"
        )),
        &startup,
    )
    .unwrap();
    expand_canonical_form(&user, "distributed", &profile).unwrap()
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
            execution_profile_id: conduit_core::ExecutionProfileId::from(format!(
                "test/{part}-profile"
            )),
            implementation_id: ImplementationId::from(format!("test/{part}/{slug}@1")),
            artifact_id: ArtifactId::from(format!("test/{part}-image@1")),
        },
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 64,
        },
    }
}

fn host(part: &str, kinds: &[&str]) -> HostAdvertisement {
    let (_, profile, _) = catalogs();
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
    suffix: &str,
) -> conduit_core::LineOffer {
    let mut line = conduit_signal::distributed_websocket_line_offer();
    line.line_id = LineId::from(format!("distributed-back/{suffix}"));
    line.binding.binding_id = LinkBindingId::from(format!("distributed-back/{suffix}/binding"));
    line.binding.source.host_id = source.host_id.clone();
    line.binding.source.boot_id = source.boot_id.clone();
    line.binding.source.endpoint_id = LinkEndpointId::from(format!("{suffix}/egress"));
    line.binding.sink.host_id = sink.host_id.clone();
    line.binding.sink.boot_id = sink.boot_id.clone();
    line.binding.sink.endpoint_id = LinkEndpointId::from(format!("{suffix}/ingress"));
    line.binding.limits.maximum_in_flight_items = 1;
    line.binding.limits.maximum_payload_bytes = 64;
    line.binding.limits.maximum_buffered_bytes = 64;
    line.binding.limits.maximum_frame_bytes = 64;
    line.availability.line_id = line.line_id.clone();
    line.availability.binding_id = line.binding.binding_id.clone();
    line.availability.sign_id = SignId::from(format!("distributed-back/{suffix}/ready"));
    line
}

fn plan_with_http_part(
    http_part: HostAdvertisement,
) -> (conduit_form::ExpandedCanonicalForm, conduit_core::Plan) {
    let form = expanded();
    let part_a = host("a", &[SOURCE, REQUEST, ENCODE, RESULT, SINK]);
    let hosts = [part_a.clone(), http_part.clone()];
    let placements = PlacementChoices {
        by_gear: form
            .gears
            .iter()
            .map(|gear| {
                let selected = if matches!(gear.kind_id.as_str(), HTTP | DECODE) {
                    &http_part
                } else {
                    &part_a
                };
                let capability = selected
                    .capabilities
                    .iter()
                    .find(|offer| offer.checked_face() == gear.checked_face())
                    .unwrap();
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: selected.host_id.clone(),
                        capability_id: capability.capability_id.clone(),
                    },
                )
            })
            .collect(),
    };
    let lines = [
        line(&part_a, &http_part, "a-to-http"),
        line(&http_part, &part_a, "http-to-a"),
    ];
    let plan = plan_expanded_canonical_with_options(
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local, ConnectionBase::WebSocket],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 64,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &lines,
        },
    )
    .unwrap();
    (form, plan)
}

fn direct_plan() -> (conduit_form::ExpandedCanonicalForm, conduit_core::Plan) {
    let form = direct_expanded();
    let direct = host("direct", &[SOURCE, HIGH, SINK]);
    let placements = default_expanded_placements(&form, std::slice::from_ref(&direct)).unwrap();
    let plan = plan_expanded_canonical_with_options(
        &form,
        &[direct],
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 64,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    (form, plan)
}

#[test]
fn one_back_is_planned_as_ordinary_leaves_across_truthful_parts() {
    let part_b = host("b", &[HTTP, DECODE]);
    let (form, plan) = plan_with_http_part(part_b.clone());
    assert_eq!(form.realization_backs.len(), 1);
    assert_eq!(
        form.realization_backs[0].invocation_path,
        "distributed/generate"
    );
    assert_eq!(form.gears.len(), 7);
    for kind in [REQUEST, ENCODE, HTTP, DECODE, RESULT] {
        let gear = form
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str() == kind)
            .unwrap();
        let origin = form
            .provenance
            .iter()
            .find(|origin| origin.gear_id == gear.gear_id.as_str())
            .unwrap();
        assert_eq!(origin.source_form, HIGH);
        assert_eq!(origin.form_path, ["distributed", "generate"]);
    }
    assert_eq!(plan.fragments.len(), 2);
    assert_eq!(plan.realization_backs, form.realization_backs);
    assert!(conduit_core::verify_plan(&plan));

    let placements = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .collect::<Vec<_>>();
    assert_eq!(placements.len(), 7);
    for kind in [HTTP, DECODE] {
        let placement = placements
            .iter()
            .find(|item| item.kind_id.as_str() == kind)
            .unwrap();
        assert_eq!(placement.host_id, part_b.host_id);
        assert_eq!(placement.boot_id, part_b.boot_id);
    }
    assert!(!plan.fragments.iter().any(|fragment| {
        fragment.host_id.as_str() == "part-a"
            && fragment
                .placements
                .iter()
                .any(|item| item.kind_id.as_str() == HTTP)
    }));
    let remote = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .filter(|connection| connection.selected_line.is_some())
        .collect::<Vec<_>>();
    assert_eq!(
        remote.len(),
        4,
        "two cross-Part Cords appear in both fragments"
    );
    assert!(remote.iter().all(|connection| {
        connection.item_capacity == 1
            && connection.byte_capacity == 64
            && connection.admitted_lines.len() == 1
    }));
}

#[test]
fn loss_yields_a_fresh_plan_without_mutating_form_or_prior_plan() {
    let part_b = host("b", &[HTTP, DECODE]);
    let (form, first) = plan_with_http_part(part_b);
    let immutable = first.clone();
    let part_a_only = host("a", &[SOURCE, REQUEST, ENCODE, RESULT, SINK]);
    assert!(default_expanded_placements(&form, &[part_a_only]).is_err());

    let part_c = host("c", &[HTTP, DECODE]);
    let (_, second) = plan_with_http_part(part_c.clone());
    assert_eq!(first, immutable);
    assert_eq!(first.source_document_id, second.source_document_id);
    assert_eq!(first.checked_form_id, second.checked_form_id);
    assert_eq!(first.expanded_form_id, second.expanded_form_id);
    assert_ne!(first.plan_id, second.plan_id);
    assert!(second
        .fragments
        .iter()
        .any(|fragment| fragment.host_id == part_c.host_id));
    assert!(!second
        .fragments
        .iter()
        .any(|fragment| fragment.host_id.as_str() == "part-b"));
    let first_remote = first
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .find_map(|connection| connection.selected_line.as_ref())
        .unwrap();
    assert!(!second
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .any(|connection| connection.permits_line(first_remote)));
    let old_fragment = first
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == "part-a")
        .unwrap();
    let new_fragment = second
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == "part-a")
        .unwrap();
    let old_play = conduit_core::bind_active_play(
        &first.plan_id,
        &old_fragment.host_id,
        &old_fragment.boot_id,
        1,
    );
    let replacement_play = conduit_core::bind_active_play(
        &second.plan_id,
        &new_fragment.host_id,
        &new_fragment.boot_id,
        2,
    );
    assert_ne!(old_play.active_play_id, replacement_play.active_play_id);
    assert_ne!(old_play.plan_id, replacement_play.plan_id);
}

#[test]
fn distributed_back_plays_through_two_production_kernel_fragments() {
    let (_, plan) = plan_with_http_part(host("b", &[HTTP, DECODE]));
    execution::execute(&plan);
}
