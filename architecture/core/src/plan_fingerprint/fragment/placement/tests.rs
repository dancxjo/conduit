//! Historical byte oracle is test-only; production has one canonical encoder.
use super::*;
use crate::*;
use alloc::{vec, vec::Vec};
// Historical canonical placement fields, shared by complete and incremental hashing.
fn legacy_placement(
    canonical: &mut impl CanonicalSink,
    gear: &crate::PlannedGear,
    configuration: &[crate::ConfigurationEntry],
) {
    push_string(canonical, gear.placement_id.as_str());
    push_string(canonical, gear.gear_id.as_str());
    push_string(canonical, gear.kind_id.as_str());
    push_string(canonical, gear.kind_contract_revision.as_str());
    push_string(canonical, gear.execution_profile_id.as_str());
    crate::plan_fingerprint::push_configuration(canonical, configuration);
    push_string(canonical, gear.host_id.as_str());
    push_string(canonical, gear.boot_id.as_str());
    push_u64(canonical, gear.offer_generation.0);
    push_string(canonical, gear.capability_id.as_str());
    push_string(canonical, gear.implementation_id.as_str());
    push_string(canonical, gear.artifact_id.as_str());
    if let Some(span) = gear.source_span {
        push_string(canonical, "source-span@1");
        push_u64(canonical, span.start);
        push_u64(canonical, span.end);
        push_u64(canonical, span.line);
        push_u64(canonical, span.column);
        push_u64(canonical, span.end_line);
        push_u64(canonical, span.end_column);
    }
    if let Some(base) = &gear.base {
        push_string(canonical, "base-provider-binding@1");
        push_string(canonical, base.base_id.as_str());
        push_string(canonical, base.provider_instance_id.as_str());
        push_u64(canonical, base.provider_generation);
        push_string(canonical, base.implementation_id.as_str());
        push_string(canonical, base.mechanism_family.as_str());
        canonical.push(base.enforcement_class as u8);
    }
    push_u32(canonical, gear.realization_characteristics.len() as u32);
    for characteristic in &gear.realization_characteristics {
        characteristic::push_characteristic_canonical(canonical, characteristic);
    }
    if !gear.realization_properties.is_empty() {
        crate::realization_properties::push_canonical(canonical, &gear.realization_properties);
    }
    canonical.extend_from_slice(&gear.limits.max_active_instances.to_le_bytes());
    canonical.extend_from_slice(&gear.limits.max_queue_items.to_le_bytes());
    push_u32(canonical, gear.limits.max_queue_bytes);
    push_ports(canonical, &gear.inputs);
    push_ports(canonical, &gear.outputs);
    push_semantic_contract(canonical, &gear.semantic_contract);
    push_u32(canonical, gear.terminal_transductions.len() as u32);
    for profile in &gear.terminal_transductions {
        push_string(canonical, "terminal-transduction@2");
        push_terminal_transduction(canonical, profile);
    }
    push_u32(canonical, gear.host_calls.len() as u32);
    for requirement in &gear.host_calls {
        push_string(canonical, requirement.contract_id.as_str());
        match &requirement.target_kind {
            Some(target_kind) => {
                canonical.push(1);
                push_string(canonical, target_kind.as_str());
            }
            None => canonical.push(0),
        }
        canonical.extend_from_slice(&requirement.maximum_in_flight.to_le_bytes());
        push_u32(canonical, requirement.maximum_input_bytes);
        push_u32(canonical, requirement.maximum_output_bytes);
    }
    push_u32(canonical, gear.resources.len() as u32);
    for binding in &gear.resources {
        legacy_resource::push_resource_binding(canonical, binding);
    }
    push_u32(canonical, gear.authority.len() as u32);
    for binding in &gear.authority {
        push_string(canonical, binding.grant_id.as_str());
        push_string(canonical, binding.contract_id.as_str());
        push_string(canonical, binding.host_call_contract_id.as_str());
        push_string(canonical, binding.subject_kind.as_str());
        push_string(canonical, binding.host_id.as_str());
        push_string(canonical, binding.boot_id.as_str());
        push_string(canonical, binding.capability_id.as_str());
    }
    push_u32(canonical, gear.pool_references.len() as u32);
    for pool in &gear.pool_references {
        push_string(canonical, pool.as_str());
    }
}

mod legacy_resource {
    use crate::{push_string, push_u32, push_u64, ResourceBinding, ResourceContentOffer};

    pub(crate) fn push_resource_binding(
        canonical: &mut impl crate::canonical_bytes::CanonicalSink,
        binding: &ResourceBinding,
    ) {
        push_string(canonical, binding.pool_id.as_str());
        push_string(canonical, binding.class_id.as_str());
        push_u32(canonical, binding.units);
        if let Some(content) = &binding.content {
            canonical.extend_from_slice(b"resource-content@1");
            push_content(canonical, content);
        }
        match &binding.compute {
            Some(compute) => {
                canonical.push(1);
                push_u32(canonical, compute.selected_lanes);
                canonical.push(compute.service_guarantee as u8);
                push_string(canonical, compute.architecture_base_id.as_str());
                canonical.push(compute.architecture_base_kind as u8);
                match &compute.topology_group_id {
                    Some(group) => {
                        canonical.push(1);
                        push_string(canonical, group.as_str());
                    }
                    None => canonical.push(0),
                }
                match &compute.performance_class {
                    Some(class) => {
                        canonical.push(1);
                        push_string(canonical, class.as_str());
                    }
                    None => canonical.push(0),
                }
                match compute.nominal_clock_hz {
                    Some(hz) => {
                        canonical.push(1);
                        push_u64(canonical, hz);
                    }
                    None => canonical.push(0),
                }
            }
            None => canonical.push(0),
        }
        match &binding.protected {
            Some(protected) => {
                canonical.push(1);
                push_string(canonical, protected.role_id.as_str());
                push_string(canonical, protected.handle_id.as_str());
                canonical.push(protected.access as u8);
                push_u64(canonical, protected.maximum_bytes);
                canonical.push(protected.commit_policy as u8);
            }
            None => canonical.push(0),
        }
    }

    fn push_content(
        bytes: &mut impl crate::canonical_bytes::CanonicalSink,
        offer: &ResourceContentOffer,
    ) {
        let c = &offer.contract;
        bytes.extend_from_slice(&c.identity.digest());
        bytes.extend_from_slice(&c.version.digest());
        push_string(bytes, c.content_profile.as_str());
        push_u32(bytes, c.maximum_bytes);
        push_u32(bytes, c.maximum_items);
        bytes.extend_from_slice(&[
            c.retention as u8,
            c.sharing as u8,
            c.access as u8,
            u8::from(c.sensitive),
        ]);
        for bound in [c.generation_slots, c.reader_leases, c.publication_slots] {
            bytes.extend_from_slice(&bound.to_le_bytes());
        }
        for id in [
            offer.owner_host.as_str(),
            offer.owner_boot.as_str(),
            offer.base_id.as_str(),
            offer.residence_profile.as_str(),
        ] {
            push_string(bytes, id);
        }
    }
}

fn fragment() -> PlanFragment {
    let value_kind = KindId::from("fixture/byte@1");
    let state = PlannedStateBoundary {
        state_id: StateId::from("retained"),
        gear_id: GearId::from("cell"),
        value_kind: value_kind.clone(),
        initial_value: Some(vec![7]),
        lifetime: StateLifetime::Step,
        retained: None,
        maximum_value_bytes: 1,
        continuation: StateContinuation::ExternallyBounded,
    };
    let port = |name: &str, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: value_kind.clone(),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    PlanFragment {
        completion_policy: crate::PlanCompletionPolicy::Live,
        plan_id: PlanId::from(""),
        fragment_id: FragmentId::from(""),
        source_document_id: SourceDocumentId::from("source"),
        checked_plot_id: CheckedPlotId::from("checked"),
        expanded_plot_id: ExpandedPlotId::from("expanded"),
        realization_backs: vec![],
        host_id: HostId::from("host"),
        boot_id: BootId::from("boot"),
        offer_generation: OfferGeneration(1),
        placements: vec![crate::planned_gear_from_parts! {
            semantic_contract: Default::default(),
            placement_id: PlacementId::from("placement"),
            gear_id: state.gear_id.clone(),
            kind_id: kind_id("fixture/state"),
            kind_contract_revision: KindIdentity::from("fixture/state@1"),
            execution_profile_id: ExecutionProfileId::from("fixture/state@1"),
            configuration: vec![],
            host_id: HostId::from("host"),
            boot_id: BootId::from("boot"),
            offer_generation: OfferGeneration(1),
            capability_id: CapabilityId::from("state"),
            implementation_id: ImplementationId::from("state@1"),
            artifact_id: ArtifactId::from("state@1"),
            base: None,
            realization_characteristics: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
            inputs: vec![port("next", PortDirection::Input)],
            outputs: vec![port("current", PortDirection::Output)],
            terminal_transductions: Vec::new(),
            host_calls: vec![],
            resources: vec![],
            authority: vec![],
            pool_references: vec![],
        }],
        execution_regions: vec![],
        execution_fusions: vec![],
        states: vec![state],
        connections: vec![],
        fore_ports: vec![],
        shared_pools: vec![],
        startup_dependencies: vec![],
        startup_order: vec![PlacementId::from("placement")],
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        expected_terminals: vec![],
        expected_sign: vec![],
        sign_storage_budget: SignStorageBudget {
            item_capacity: 2,
            byte_capacity: 64,
        },
        plan_fragments: vec![],
    }
}

fn populated() -> PlannedGear {
    let mut g = fragment().placements.remove(0);
    g.source_span = Some(SourceSpan {
        start: 1,
        end: u64::MAX,
        line: 2,
        column: 3,
        end_line: 4,
        end_column: 5,
    });
    g.offer_generation = OfferGeneration(u64::MAX);
    g.configuration = vec![ConfigurationEntry {
        key: "é\0".into(),
        value: ConfigurationValue::U64(u64::MAX),
    }];
    g.base = Some(BaseProviderBinding {
        base_id: "base".into(),
        provider_instance_id: "instance".into(),
        provider_generation: u64::MAX,
        implementation_id: "implementation".into(),
        mechanism_family: "mechanism".into(),
        enforcement_class: BaseEnforcementClass::HardwareGated,
    });
    g.limits = CapabilityLimits {
        max_active_instances: u16::MAX,
        max_queue_items: u16::MAX,
        max_queue_bytes: u32::MAX,
    };
    g.host_calls = vec![HostCallRequirement {
        contract_id: "call".into(),
        target_kind: Some("target".into()),
        maximum_in_flight: u16::MAX,
        maximum_input_bytes: u32::MAX,
        maximum_output_bytes: u32::MAX,
    }];
    g.resources = vec![ResourceBinding {
        pool_id: "pool".into(),
        class_id: "class".into(),
        units: u32::MAX,
        protected: Some(ProtectedResourceBinding {
            role_id: "role".into(),
            handle_id: "handle".into(),
            access: ProtectedResourceAccess::ReadExisting,
            maximum_bytes: u64::MAX,
            commit_policy: ProtectedResourceCommitPolicy::CreateOnly,
        }),
        compute: Some(ComputeReservation {
            selected_lanes: u32::MAX,
            service_guarantee: ComputeServiceGuarantee::Exclusive,
            architecture_base_id: "architecture".into(),
            architecture_base_kind: ArchitectureBaseKind::BareMetal,
            topology_group_id: Some("group".into()),
            performance_class: Some("performance".into()),
            nominal_clock_hz: Some(u64::MAX),
        }),
        content: Some(ResourceContentOffer {
            contract: ResourceContentRequirement {
                identity: ResourceSemanticIdentity::from_digest([1; 32]),
                version: ResourceVersionIdentity::from_digest([2; 32]),
                content_profile: "content".into(),
                maximum_bytes: u32::MAX,
                maximum_items: u32::MAX,
                retention: ResourceRetention::Play,
                sharing: ResourceSharing::SingleWriterPublished,
                access: ResourceAccessMode::WriteCandidatePublish,
                generation_slots: u16::MAX,
                reader_leases: u16::MAX,
                publication_slots: u16::MAX,
                sensitive: true,
            },
            owner_host: "owner".into(),
            owner_boot: "boot".into(),
            base_id: "residence".into(),
            residence_profile: "profile".into(),
        }),
    }];
    g.authority = vec![AuthorityBinding {
        grant_id: "grant".into(),
        contract_id: "contract".into(),
        host_call_contract_id: "call".into(),
        subject_kind: "subject".into(),
        host_id: "host".into(),
        boot_id: "boot".into(),
        capability_id: "capability".into(),
    }];
    g.terminal_transductions = vec![TerminalTransductionProfile {
        input_port_id: "next".into(),
        output_port_id: "current".into(),
        normal_close: NormalCloseTransduction::FlushThenPropagate(FiniteTerminalEmission {
            maximum_items: u16::MAX,
            maximum_bytes: u32::MAX,
        }),
        abnormal: AbnormalTerminalTransduction::DomainSpecific {
            law: "abnormal".into(),
        },
        cancellation: CancellationTransduction::Request {
            disposition_kind: "cancel".into(),
        },
    }];
    g.realization_characteristics = vec![crate::characteristic::stable_realization_boolean(
        "characteristic",
        "name",
        "help",
        true,
    )];
    let ty = StructuredInfoType::leaf(kind_id("structured")).unwrap();
    let profile = ty.profile().unwrap().value_kind().clone();
    let value = StructuredInfoValue::leaf(ty, vec![0, 255, 7]).unwrap();
    g.realization_properties =
        vec![StructuredConfigurationValue::new(profile, value.canonical_bytes().unwrap()).unwrap()];
    g.semantic_contract.laws = vec![KindSemanticLaw::TerminalTransduction(
        g.terminal_transductions[0].clone(),
    )];
    g.pool_references = vec!["shared".into()];
    g
}
#[test]
fn borrowed_placement_bytes_match_historical_encoder_for_optional_branches() {
    for mask in 0..4096 {
        let mut g = populated();
        if mask & 1 == 0 {
            g.source_span = None;
        }
        if mask & 2 == 0 {
            g.base = None;
        }
        if mask & 4 == 0 {
            g.resources[0].protected = None;
        }
        if mask & 8 == 0 {
            g.resources[0].compute = None;
        }
        if mask & 16 == 0 {
            g.resources[0].content = None;
        }
        if mask & 32 == 0 {
            g.host_calls[0].target_kind = None;
        }
        if mask & 64 == 0 {
            if let Some(c) = &mut g.resources[0].compute {
                c.topology_group_id = None;
            }
        }
        if mask & 128 == 0 {
            g.configuration.clear();
            g.inputs.clear();
            g.outputs.clear();
            g.host_calls.clear();
            g.authority.clear();
            g.terminal_transductions.clear();
            g.pool_references.clear();
        }
        if mask & 256 == 0 {
            g.realization_characteristics.clear();
            g.realization_properties.clear();
            g.semantic_contract.laws.clear();
        }
        if let Some(c) = &mut g.resources[0].compute {
            if mask & 512 == 0 {
                c.performance_class = None;
            }
            if mask & 1024 == 0 {
                c.nominal_clock_hz = None;
            }
        }
        if mask & 2048 == 0 {
            g.resources.clear();
        }
        let mut old = Vec::new();
        legacy_placement(&mut old, &g, &g.configuration);
        let mut bytes = Vec::new();
        push_placement(&mut bytes, (&g).into());
        assert_eq!(bytes, old, "owned refs mask {mask}");
        let resources: Vec<_> = g.resources.iter().map(Into::into).collect();
        let authority: Vec<_> = g.authority.iter().map(Into::into).collect();
        let mut view = PlannedGearRef::from(&g);
        view.resources = ResourceBindingRefs::Borrowed(&resources);
        view.authority = AuthorityBindingRefs::Borrowed(&authority);
        bytes.clear();
        push_placement(&mut bytes, view);
        assert_eq!(bytes, old, "independent refs mask {mask}");
    }
}
