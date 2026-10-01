use conduit_core::*;

fn scope() -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: HostId::from("host/one"),
        boot_id: BootId::from("boot/one"),
        base_instance_id: BaseInstanceId::from("base/framebuffer/one"),
        base_provider_generation: 1,
        plan_id: PlanId::from("plan/mask"),
        active_play_id: ActivePlayId::from("play/mask"),
        authority_grant_id: AuthorityGrantId::from("grant/framebuffer"),
        authority_contract_id: AuthorityContractId::from("authority/present@1"),
        capability_id: CapabilityId::from("framebuffer/source"),
        implementation_id: ImplementationId::from("renderer/native@1"),
        operation_contract_id: HostCallContractId::from("conduit.host/present@1"),
        subject_kind: KindId::from("presentation/renderer"),
        resource_pool_id: ResourcePoolId::from("framebuffer/surface"),
        resource_generation_id: ResourceGenerationId("framebuffer/generation/1".into()),
        envelope_id: CapabilityEnvelopeId::from("framebuffer/present/bounded"),
        maximum_parameter_bytes: 64,
        maximum_result_bytes: 64,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 1,
    }
}

fn request() -> CapabilityIssueRequest {
    let scope = scope();
    CapabilityIssueRequest {
        authority: BaseCapabilityAuthority {
            grant: AuthorityGrant {
                grant_id: scope.authority_grant_id.clone(),
                contract_id: scope.authority_contract_id.clone(),
                host_call_contract_id: scope.operation_contract_id.clone(),
                subject_kind: scope.subject_kind.clone(),
                host_id: scope.host_id.clone(),
                boot_id: scope.boot_id.clone(),
                capability_id: scope.capability_id.clone(),
            },
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            operation_contract_id: scope.operation_contract_id.clone(),
            envelope_id: scope.envelope_id.clone(),
            maximum_parameter_bytes: 64,
            maximum_result_bytes: 64,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 1,
        },
        scope,
    }
}

fn claim() -> BaseOperationClaim {
    let scope = scope();
    BaseOperationClaim {
        host_id: scope.host_id,
        boot_id: scope.boot_id,
        base_instance_id: scope.base_instance_id,
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id,
        active_play_id: scope.active_play_id,
        implementation_id: scope.implementation_id,
        operation_contract_id: scope.operation_contract_id,
        subject_kind: scope.subject_kind,
        resource_pool_id: scope.resource_pool_id,
        resource_generation_id: scope.resource_generation_id,
        envelope_id: scope.envelope_id,
        parameter_bytes: 1,
        work_units: 1,
    }
}

fn connection() -> PlannedConnection {
    PlannedConnection {
        connection_id: ConnectionId::from("mask/framebuffer-cord"),
        source_placement_id: PlacementId::from("mask/framebuffer-source"),
        source_port_id: PortId::from("surface"),
        sink_placement_id: PlacementId::from("mask/renderer"),
        sink_port_id: PortId::from("surface"),
        value_kind: KindId::from("conduitos.resource/framebuffer@1"),
        resource: Some(PlannedResourceConnection {
            contract: ResourcePortContract {
                port_id: PortId::from("surface"),
                class_id: ResourceClassId::from("conduitos.resource/framebuffer@1"),
                ownership: ResourcePortOwnership::Move,
                lifecycle: ResourcePortLifecycle::Play,
                mobility: ResourcePortMobility::HostLocal,
            },
            owner_placement_id: PlacementId::from("mask/framebuffer-source"),
            source_binding: ResourceBinding {
                pool_id: ResourcePoolId::from("framebuffer/surface"),
                class_id: ResourceClassId::from("conduitos.resource/framebuffer@1"),
                units: 1,
                protected: None,
                compute: None,
                content: None,
            },
        }),
        abnormal_kind: None,
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        selected_line: None,
        admitted_lines: Vec::new(),
        item_capacity: 1,
        byte_capacity: 1,
    }
}

#[test]
fn move_only_bearer_crosses_only_the_exact_local_plan_cord_and_revokes_at_play_end() {
    let scope = scope();
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        scope.base_provider_generation,
        [7; 32],
        1,
    )
    .unwrap();
    let handle = table.issue(request()).unwrap();
    let planned = connection();
    let offered = OfferedResourceCord::new(
        scope.plan_id.clone(),
        scope.active_play_id.clone(),
        &planned,
        handle,
    )
    .unwrap();
    let (_, offered) = *offered
        .accept(&PlacementId::from("wrong"), &PortId::from("surface"))
        .expect_err("wrong sink refuses");
    let mut accepted = offered
        .accept(&planned.sink_placement_id, &planned.sink_port_id)
        .unwrap();
    let lease = accepted.authorize(&mut table, &claim()).unwrap();
    accepted.complete(&mut table, lease, 1).unwrap();
    let inspection = accepted
        .retire(&mut table, ResourcePortLifecycle::Play)
        .unwrap();
    assert_eq!(inspection.connection_id, planned.connection_id);
    assert_eq!(
        table.inspections().next().unwrap().lifecycle,
        CapabilityLifecycle::Revoked
    );
}
