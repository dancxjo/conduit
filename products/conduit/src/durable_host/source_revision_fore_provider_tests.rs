//! CPU descriptor fixtures only: no physical custody, member proof or Start.
use super::*;
use conduit_plot as p;
use std::path::PathBuf;
fn fixture_plan(bytes: u32, profile: &str) -> (c::Plan, c::HostAdvertisement) {
    let source = "plot increment (\n >> reading: U8\n result: U8 >>\n) = (reading + 1)\n";
    let checked =
        p::check_syntax_document(&p::parse_syntax_document(source), &p::StartupCatalog::new())
            .unwrap();
    let plot =
        p::expand_canonical_plot_for_authoring(&checked, "increment", &p::ProfileCatalog::new())
            .unwrap();
    let gear = &plot.expanded.gears[0];
    let c::ConfigurationValue::Text(program) = &gear.configuration[0].value else {
        panic!("program")
    };
    let program = p::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let _initialized = p::PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let offer = c::capability_offer_from_parts! {
        semantic_contract:gear.semantic_contract.clone(),startup_parameters:gear.startup_parameters.clone(),shorthand:gear.shorthand.clone(),
        capability_id:"fixture/increment".into(),kind_id:gear.kind_id.clone(),kind_contract_revision:gear.kind_contract_revision.clone(),
        implementation:c::ImplementationOffer {execution_profile_id:"fixture/pure".into(),implementation_id:"fixture/increment".into(),artifact_id:"fixture/increment".into()},
        inputs:gear.inputs.clone(),outputs:gear.outputs.clone(),host_calls:vec![],resource_requirements:vec![],authority_requirements:vec![],
        limits:c::CapabilityLimits {max_active_instances:1,max_queue_items:1,max_queue_bytes:1024},
    };
    let host = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: profile.into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![offer],
        planner_capabilities: vec![],
    };
    let hosts = [host];
    let choices = conduit_planner::default_expanded_placements(&plot.expanded, &hosts).unwrap();
    let mut fore = BTreeMap::new();
    for (direction, bindings) in [
        (c::PortDirection::Input, &plot.input_bindings),
        (c::PortDirection::Output, &plot.output_bindings),
    ] {
        for binding in bindings {
            fore.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            );
        }
    }
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &plot,
        &hosts,
        &choices,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: bytes,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &fore,
    )
    .unwrap();
    (plan, hosts[0].clone())
}

const HOST: &str = r#"host fixture {
 schema = 1
 target = {architecture: "thumbv6m", machine: "pico-w", board: "pico-w"}
 preparation = {authorizer: "seed-owner", maximum_assignment_bytes: 1024, maximum_runtime_state_bytes: 1024}
 execution = {authorizer: "seed-owner", maximum_active_worksets: 1}
 base = {kind: "line/fore-carrier", implementation: "fixture/physical-fore@1", initialization: {authorizer: "seed-owner", maximum_lines: 2, maximum_frame_bytes: 512, maximum_payload_bytes: 460, maximum_buffered_bytes: 1024, maximum_in_flight_items: 1, maximum_frames: 100}}
 limits = {static_memory_bytes: 65536, heap_arena_bytes: 65536, queue_items: 64, buffered_bytes: 4096, active_instances: 8, operation_slots: 8, timer_slots: 8, line_sessions: 2, evidence_items: 64}
}"#;
struct Fixture {
    root: PathBuf,
    policy: DirectForeCarrierSelection,
    member: owner::MemberPresentationBasis,
    peer: conduit_body::FederationPeer,
    partitions: Vec<BodyPlotPlan>,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "conduit-direct-fore-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let definition = root.join("host.conduit");
        std::fs::write(&definition, HOST).unwrap();
        let config = conduit_host_make::parse_host_configuration_conduit(HOST).unwrap();
        let profile = conduit_host_make::host_configuration_identity(&config).unwrap();
        let (plan, ad) = fixture_plan(64, &profile);
        let plot = ResidentPlot::new(
            plan.fragments[0].source_document_id.clone(),
            plan.fragments[0].checked_plot_id.clone(),
        );
        let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
            &plan,
            &plan.fragments[0].fragment_id,
        )
        .unwrap();
        let ports = lowered
            .fore_ports
            .iter()
            .map(
                |p| super::super::super::super::local_fore_policy::LocalForePortIntent {
                    front_port_id: p.front_port_id.clone(),
                    direction: p.direction,
                    track: p.track,
                    value_kind: p.value_kind.clone(),
                    item_capacity: p.item_capacity,
                    byte_capacity: p.byte_capacity,
                },
            )
            .collect();
        let credential=serde_json::from_value(serde_json::json!({"credential_id":"fixture/credential","body_id":"fixture/body","part_id":"fixture/part","host_id":ad.host_id,"boot_id":ad.boot_id,"issued_at_millis":1})).unwrap();
        let member = owner::MemberPresentationBasis::new(credential, ad).unwrap();
        let peer = conduit_body::FederationPeer {
            host_id: "fixture/owner".into(),
            boot_id: "fixture/owner-boot".into(),
            offer_generation: c::OfferGeneration(1),
            verifying_key: [7; 32],
        };
        Self {
            root,
            policy: DirectForeCarrierSelection {
                host_definition: definition,
                installed_image_id: "image:fixture".into(),
                usb_serial: "fixture/serial".into(),
                plot: plot.clone(),
                ports,
            },
            member,
            peer,
            partitions: vec![BodyPlotPlan { plot, plan }],
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
#[test]
fn descriptive_rows_bind_the_original_plan_and_same_full_physical_session() {
    let fixture = Fixture::new();
    let (rows, sessions, _material) = describe(
        &fixture.policy,
        &fixture.member,
        &fixture.peer,
        &fixture.partitions,
    )
    .unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].session, sessions[1].session);
    let sealed = conduit_planner::bind_external_fore_lines(
        &fixture.partitions[0].plan,
        &rows.choices,
        &rows.offers,
        &rows.allowed_bases,
    )
    .unwrap();
    assert!(c::verify_plan(&sealed));
    assert_ne!(sealed.plan_id, fixture.partitions[0].plan.plan_id);
    let local = conduit_embedded_build::assigned_fore_inventory(
        &fixture.partitions[0].plan,
        &fixture.partitions[0].plan.fragments[0].fragment_id,
        16,
    )
    .unwrap();
    let selected = conduit_embedded_build::assigned_fore_inventory(
        &sealed,
        &sealed.fragments[0].fragment_id,
        16,
    )
    .unwrap();
    assert_eq!(local, selected);
}
#[test]
fn descriptive_selection_refuses_stale_profile_fore_geometry_and_absent_scope() {
    let fixture = Fixture::new();
    let mut member = fixture.member.clone();
    member.host_advertisement.profile = "fixture/stale-profile".into();
    assert!(describe(&fixture.policy, &member, &fixture.peer, &fixture.partitions).is_err());
    for change in 0..4 {
        let mut policy = fixture.policy.clone();
        match change {
            0 => policy.ports[0].byte_capacity += 1,
            1 => policy.ports[0].front_port_id = "fixture/absent".into(),
            2 => policy.ports.pop().map(|_| ()).unwrap(),
            _ => policy.plot.checked_plot_id = "fixture/foreign-plot".into(),
        }
        assert!(describe(&policy, &fixture.member, &fixture.peer, &fixture.partitions).is_err());
    }
    let mut member = fixture.member.clone();
    member.host_advertisement.boot_id = "fixture/stale-boot".into();
    assert!(describe(&fixture.policy, &member, &fixture.peer, &fixture.partitions).is_err());
}
#[test]
fn no_selected_carrier_is_not_descriptive_line_availability() {
    let fixture = Fixture::new();
    assert!(select_direct_fore_lines(
        &[],
        &mut [Err("fixture/absent".into())],
        &fixture.peer,
        &fixture.partitions
    )
    .is_err());
    let (choices, sessions) =
        select_direct_fore_lines(&[], &mut [], &fixture.peer, &fixture.partitions).unwrap();
    assert!(choices.is_empty());
    assert!(sessions.is_empty());
}

#[test]
fn client_binding_uses_original_child_bytes_custody_and_installed_image() {
    use sha2::{Digest, Sha256};
    let fixture = Fixture::new();
    let (rows, sessions, material) = describe(
        &fixture.policy,
        &fixture.member,
        &fixture.peer,
        &fixture.partitions,
    )
    .unwrap();
    let sealed = conduit_planner::bind_external_fore_lines(
        &fixture.partitions[0].plan,
        &rows.choices,
        &rows.offers,
        &rows.allowed_bases,
    )
    .unwrap();
    let fragment = &sealed.fragments[0];
    let generated = conduit_embedded_build::generate_line_fore_native_plan(
        &sealed,
        &fragment.fragment_id,
        conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING,
        conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536,
        },
        2,
        &sessions,
    )
    .unwrap();
    let projection = generated
        .encode(c::AssignedPlanMaxima {
            encoded_bytes: 16384,
            runtime_state_bytes: 32768,
            counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
        })
        .unwrap();
    let inventory: Vec<_> = generated
        .fore()
        .iter()
        .flat_map(|pin| pin.payload().iter().copied())
        .collect();
    let body = fixture.member.credential.body_id.as_str();
    let play = format!(
        "body-play/sha256:{}",
        conduit_body::body_play_digest(body, "fixture/wake", "fixture/body-plan", 1)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let (count, digest) =
        conduit_body::local_fragment_set_commitment(&[fragment.fragment_id.as_str()]).unwrap();
    let metadata = conduit_body::BodyFragmentApprovalMetadata {
        schema: 3,
        body,
        wake: "fixture/wake",
        body_plan: "fixture/body-plan",
        body_play: &play,
        play_sequence: 1,
        workload_revision: 1,
        source: fixture.policy.plot.source_document_id.as_str(),
        checked_plot: fixture.policy.plot.checked_plot_id.as_str(),
        expanded_plot: fragment.expanded_plot_id.as_str(),
        plot_plan: sealed.plan_id.as_str(),
        fragment: fragment.fragment_id.as_str(),
        host: fragment.host_id.as_str(),
        boot: fragment.boot_id.as_str(),
        offer_generation: fixture.member.host_advertisement.offer_generation.0,
        local_fragment_count: count,
        local_fragment_digest: digest,
        resource_bindings: &[],
        fore_ports: &inventory,
        projection_length: projection.len() as u32,
        projection_digest: Sha256::digest(&projection).into(),
    };
    let child = conduit_body::encode_body_fragment_document(&metadata, &projection, 65536).unwrap();
    let payload = conduit_body::encode_body_workset_document(&[&child], 65536).unwrap();
    let bindings = material.bindings(&payload, [8; 32]).unwrap();
    material.prepare_carriage(&payload, [8; 32]).unwrap();
    assert_ne!(bindings[0].scope_commitment, bindings[1].scope_commitment);
    let input = generated
        .profile()
        .lines
        .iter()
        .find(|row| row.fore().direction() == c::AssignedForeDirection::Input)
        .unwrap();
    let full = conduit_wire::fore_carrier_scope_commitment(
        &Sha256::digest(&child).into(),
        "image:fixture",
        fixture.member.host_advertisement.profile.as_str(),
        conduit_wire::ForeCarrierScopeBounds {
            maximum_lines: 2,
            maximum_frame_bytes: 512,
            maximum_payload_bytes: 460,
            maximum_buffered_bytes: 1024,
            maximum_in_flight_items: 1,
            maximum_frames: 100,
        },
    );
    assert_eq!(
        bindings[0].scope_commitment,
        conduit_wire::external_fore_scope_commitment(&full, &[8; 32], input.payload())
    );
    assert_ne!(
        bindings[0].scope_commitment,
        material.bindings(&payload, [9; 32]).unwrap()[0].scope_commitment
    );
    let mut other = fixture.policy.clone();
    other.installed_image_id = "image:other".into();
    let (_, _, other) =
        describe(&other, &fixture.member, &fixture.peer, &fixture.partitions).unwrap();
    assert_ne!(
        bindings[0].scope_commitment,
        other.bindings(&payload, [8; 32]).unwrap()[0].scope_commitment
    );
    let mut damaged = payload.clone();
    *damaged.last_mut().unwrap() ^= 1;
    assert!(material.bindings(&damaged, [8; 32]).is_err());
    assert!(material
        .bindings(&payload[..payload.len() - 1], [8; 32])
        .is_err());
}
