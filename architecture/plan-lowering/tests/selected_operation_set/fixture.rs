use super::*;
pub struct Fixture {
    pub kind: Kind,
    pub fragment: PlanFragment,
    pub lowered: LoweredPlanFragment,
    pub active: ActivePlayIdentity,
    pub placement: PlacementId,
    pub scopes: [BaseCapabilityScope; 2],
    pub owner: ResourceAdmissionOwner,
    pub resource: ResourceBinding,
    pub requirement: ResourceRequirement,
}
pub fn grant(scope: &BaseCapabilityScope) -> AuthorityGrant {
    AuthorityGrant {
        grant_id: scope.authority_grant_id.clone(),
        contract_id: scope.authority_contract_id.clone(),
        host_call_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        capability_id: scope.capability_id.clone(),
    }
}
pub fn operation(scope: BaseCapabilityScope) -> BoundBaseOperation {
    operation_with_claim(scope, |_: &mut BaseOperationClaim| {}).unwrap()
}
pub fn operation_with_claim(
    scope: BaseCapabilityScope,
    change: impl FnOnce(&mut BaseOperationClaim),
) -> Result<BoundBaseOperation, BaseCapabilityRefusal> {
    let authority = BaseCapabilityAuthority {
        grant: grant(&scope),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: scope.maximum_parameter_bytes,
        maximum_result_bytes: scope.maximum_result_bytes,
        maximum_work_units: scope.maximum_work_units,
        maximum_in_flight: scope.maximum_in_flight,
        maximum_operations: scope.maximum_operations,
    };
    let mut claim = BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: scope.maximum_parameter_bytes,
        work_units: scope.maximum_work_units,
    };
    change(&mut claim);
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        scope.base_provider_generation,
        [19; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();
    BoundBaseOperation::bind(table, handle, claim).map_err(|(_, _, _, error)| error)
}
impl Fixture {
    pub fn new() -> Self {
        let port = |name: &str, direction| PortDescriptor {
            port_id: name.into(),
            value_kind: "fixture/bytes".into(),
            direction,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        };
        let kind = Kind {
            startup_parameters: vec![],
            shorthand: None,
            kind_id: "fixture/record-service".into(),
            kind_contract_revision: "fixture/record-service@1".into(),
            inputs: vec![
                port("append", PortDirection::Input),
                port("read", PortDirection::Input),
            ],
            outputs: vec![
                port("appended", PortDirection::Output),
                port("records", PortDirection::Output),
            ],
            configuration: vec![],
            semantic_laws: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 4,
                max_queue_bytes: 32,
            },
        };
        let append = BaseCapabilityScope {
            host_id: "host".into(),
            boot_id: "boot".into(),
            base_instance_id: "provider".into(),
            base_provider_generation: 1,
            plan_id: "plan".into(),
            active_play_id: "play".into(),
            authority_grant_id: "grant/append".into(),
            authority_contract_id: "authority/append".into(),
            capability_id: "service".into(),
            implementation_id: "service@1".into(),
            operation_contract_id: "host/append".into(),
            subject_kind: "resource/append".into(),
            resource_pool_id: "residence".into(),
            resource_generation_id: ResourceGenerationId("generation/1".into()),
            envelope_id: "residence/envelope".into(),
            maximum_parameter_bytes: 8,
            maximum_result_bytes: 8,
            maximum_work_units: 4,
            maximum_in_flight: 1,
            maximum_operations: 8,
        };
        let mut read = append.clone();
        read.authority_grant_id = "grant/read".into();
        read.authority_contract_id = "authority/read".into();
        read.operation_contract_id = "host/read".into();
        read.subject_kind = "resource/read".into();
        let mut scopes = [append, read];
        let (mut fragment, _, _, placement) = selection_fixture::selected(
            &scopes[0],
            &kind,
            "host/append",
            8,
            "residence",
            "service",
        );
        let gear = &mut fragment.placements[0];
        gear.host_calls = scopes
            .iter()
            .map(|s| HostCallRequirement {
                contract_id: s.operation_contract_id.clone(),
                target_kind: Some(s.subject_kind.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: 8,
                maximum_output_bytes: 8,
            })
            .collect();
        gear.authority = scopes
            .iter()
            .map(|s| AuthorityBinding {
                grant_id: s.authority_grant_id.clone(),
                contract_id: s.authority_contract_id.clone(),
                host_call_contract_id: s.operation_contract_id.clone(),
                subject_kind: s.subject_kind.clone(),
                host_id: s.host_id.clone(),
                boot_id: s.boot_id.clone(),
                capability_id: s.capability_id.clone(),
            })
            .collect();
        let resource = gear.resources[0].clone();
        let requirement = resource_requirement(resource.class_id.as_str(), 1);
        let fragment = seal_plan(
            PlotIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_plot_id: fragment.checked_plot_id.clone(),
                expanded_plot_id: fragment.expanded_plot_id.clone(),
            },
            vec![fragment],
        )
        .fragments
        .remove(0);
        let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        for s in &mut scopes {
            s.plan_id = fragment.plan_id.clone();
            s.active_play_id = active.active_play_id.clone();
        }
        let lowered = lower_plan_fragment(&fragment).unwrap();
        let gear = &fragment.placements[0];
        let capability = BackOfferBuilder::new(
            kind.clone(),
            Back {
                capability_id: gear.capability_id.clone(),
                execution_profile_id: gear.execution_profile_id.clone(),
                implementation_id: gear.implementation_id.clone(),
                artifact_id: gear.artifact_id.clone(),
                host_calls: gear.host_calls.clone(),
                resource_requirements: vec![requirement.clone()],
                authority_requirements: scopes
                    .iter()
                    .map(|s| AuthorityRequirement {
                        contract_id: s.authority_contract_id.clone(),
                        host_call_contract_id: s.operation_contract_id.clone(),
                        subject_kind: s.subject_kind.clone(),
                    })
                    .collect(),
            },
        )
        .build();
        let host = HostAdvertisement {
            declared_name: None,
            protocol_version: PROTOCOL_VERSION,
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            offer_generation: fragment.offer_generation,
            profile: "fixture".into(),
            bases: vec![],
            capabilities: vec![capability],
            planner_capabilities: vec![],
            resources: vec![resource_offer(
                resource.pool_id.as_str(),
                resource.class_id.as_str(),
                1,
            )],
        };
        let observations = [ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: resource.pool_id.clone(),
            class_id: resource.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: 1,
            utilized_units: 0,
            sign_id: "current".into(),
        }];
        let mut owner = ResourceAdmissionOwner::new(host);
        owner
            .admit_planned_placement(fragment.plan_id.clone(), gear, &observations)
            .unwrap();
        Self {
            kind,
            fragment,
            lowered,
            active,
            placement,
            scopes,
            owner,
            resource,
            requirement,
        }
    }
    pub fn calls(&self) -> [SelectedServiceCall<'_>; 2] {
        core::array::from_fn(|i| SelectedServiceCall {
            contract: &self.scopes[i].operation_contract_id,
            target_kind: &self.scopes[i].subject_kind,
            input_port: &self.kind.inputs[i].port_id,
            output_port: &self.kind.outputs[i].port_id,
            input_bytes: 8,
            output_bytes: 8,
            work_units: 4,
            maximum_operations: 8,
        })
    }
    pub fn bind(
        &self,
        ops: &[&BoundBaseOperation],
    ) -> Result<SelectedOperationSet, SelectedOperationSetRefusal> {
        self.bind_calls(ops, &self.calls())
    }
    pub fn bind_calls(
        &self,
        ops: &[&BoundBaseOperation],
        calls: &[SelectedServiceCall<'_>],
    ) -> Result<SelectedOperationSet, SelectedOperationSetRefusal> {
        bind_selected_operation_set(
            ops,
            SelectedServiceContract {
                kind: &self.kind,
                configuration: &[],
                resource: &self.resource,
                requirement: &self.requirement,
                resource_generation: &self.scopes[0].resource_generation_id,
                envelope: &self.scopes[0].envelope_id,
                calls,
            },
            SelectedOperationPlan {
                fragment: &self.fragment,
                lowered: &self.lowered,
                active: &self.active,
                placement_id: &self.placement,
            },
            &self.owner,
        )
    }
}
