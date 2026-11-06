use super::*;
pub fn definition(
    host: &HostAdvertisement,
    prepared: &PreparedSourceChecker,
) -> NativeSourceDefinition {
    definition_for(
        host,
        prepared.kind().clone(),
        prepared.bounds(),
        prepared.retained_bytes() as u64,
    )
}
pub fn definition_for(
    host: &HostAdvertisement,
    kind: Kind,
    bounds: SourceCheckPreparationBounds,
    resource_bytes: u64,
) -> NativeSourceDefinition {
    let offer = conduit_core::capability_offer_from_parts! {
        startup_parameters: vec![], shorthand: None,
        capability_id: "fixture/source-check".into(), kind_id: kind.kind_id.clone(),
        kind_contract_revision: kind.kind_contract_revision.clone(), inputs: kind.inputs.clone(), outputs: kind.outputs.clone(), semantic_contract: kind.semantic_contract(),
        implementation: ImplementationOffer { execution_profile_id: "fixture/prepared-source-check@1".into(), implementation_id: "fixture/source-check".into(), artifact_id: "fixture/canonical-rust-checker".into() },
        host_calls: vec![HostCallRequirement { contract_id: SOURCE_CHECK_CALL.into(), target_kind: Some(kind.kind_id.clone()), maximum_in_flight: 1, maximum_input_bytes: bounds.request_bytes, maximum_output_bytes: bounds.report_bytes }],
        resource_requirements: vec![ResourceRequirement { class_id: SOURCE_CHECK_RESOURCE.into(), units: 1, protected_role: None, compute: None, content: None }],
        authority_requirements: vec![AuthorityRequirement { contract_id: SOURCE_CHECK_AUTHORITY.into(), host_call_contract_id: SOURCE_CHECK_CALL.into(), subject_kind: kind.kind_id.clone() }],
        limits: kind.limits.clone(),
    };
    let grant = AuthorityGrant {
        grant_id: "fixture/independent-source-read-grant".into(),
        contract_id: SOURCE_CHECK_AUTHORITY.into(),
        host_call_contract_id: SOURCE_CHECK_CALL.into(),
        subject_kind: kind.kind_id.clone(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    NativeSourceDefinition {
        kind,
        grant,
        base: BaseProviderEntry {
            base_id: "fixture/source-check-base".into(),
            provider_instance_id: "fixture/source-check-provider".into(),
            provider_generation: 1,
            implementation_id: "fixture/independent-source-read-owner".into(),
            mechanism_family: "fixture/prepared-analysis".into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![ResourceOffer {
                pool_id: "fixture/immutable-analyses".into(),
                class_id: SOURCE_CHECK_RESOURCE.into(),
                capacity_units: 1,
                compute: None,
                content: None,
            }],
        },
        resource_generation: ResourceGenerationId(
            "fixture/immutable-source-generation-set@1".into(),
        ),
        configuration: vec![],
        input_bytes: bounds.request_bytes,
        output_bytes: bounds.report_bytes,
        resource_bytes,
    }
}
pub fn plan(
    host: &StdHost,
    authored: &ExpandedAuthoringPlot,
    grant: &[AuthorityGrant],
    bounds: SourceCheckPreparationBounds,
) -> Result<Plan, String> {
    let hosts = [host.advertisement().clone()];
    let placements =
        default_expanded_placements(&authored.expanded, &hosts).map_err(|e| format!("{e:?}"))?;
    let boundaries = [
        (PortDirection::Input, "request", bounds.request_bytes),
        (PortDirection::Output, "reply", bounds.report_bytes),
    ]
    .into_iter()
    .map(|(direction, name, bytes)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: bytes,
            },
        )
    })
    .collect();
    plan_expanded_authoring_with_options(
        authored,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: bounds.report_bytes.max(bounds.request_bytes),
            authority_grants: grant,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .map_err(|e| format!("{e:?}"))
}
