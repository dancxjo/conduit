use conduit_core::{
    ActivePlayId, AuthorityContractId, AuthorityGrant, AuthorityGrantId, BaseCapabilityAuthority,
    BaseCapabilityRefusal, BaseCapabilityScope, BaseCapabilityTable, BaseInstanceId,
    BaseOperationClaim, BootId, CapabilityEnvelopeId, CapabilityId, CapabilityIssueRequest,
    CapabilityLifecycle, HostId, HostOperationContractId, ImplementationId, KindId, PlanId,
    ResourceGenerationId, ResourcePoolId,
};

fn scope() -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: HostId::from("host/one"),
        boot_id: BootId::from("boot/one"),
        base_instance_id: BaseInstanceId::from("base/files/instance"),
        base_provider_generation: 4,
        plan_id: PlanId::from("plan/one"),
        active_play_id: ActivePlayId::from("play/one"),
        authority_grant_id: AuthorityGrantId::from("grant/visible"),
        authority_contract_id: AuthorityContractId::from("authority/file-read@1"),
        capability_id: CapabilityId::from("file/copy"),
        implementation_id: ImplementationId::from("implementation/file-copy@1"),
        operation_contract_id: HostOperationContractId::from("conduit.host/file-read@1"),
        subject_kind: KindId::from("file/source"),
        resource_pool_id: ResourcePoolId::from("files/project/source"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        envelope_id: CapabilityEnvelopeId::from("file/read-only/project-source/max-64"),
        maximum_parameter_bytes: 64,
        maximum_result_bytes: 128,
        maximum_work_units: 8,
        maximum_in_flight: 2,
        maximum_operations: 3,
    }
}

fn authority() -> BaseCapabilityAuthority {
    BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: AuthorityGrantId::from("grant/visible"),
            contract_id: AuthorityContractId::from("authority/file-read@1"),
            host_operation_contract_id: HostOperationContractId::from("conduit.host/file-read@1"),
            subject_kind: KindId::from("file/source"),
            host_id: HostId::from("host/one"),
            boot_id: BootId::from("boot/one"),
            capability_id: CapabilityId::from("file/copy"),
        },
        base_instance_id: BaseInstanceId::from("base/files/instance"),
        base_provider_generation: 4,
        resource_pool_id: ResourcePoolId::from("files/project/source"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        operation_contract_id: HostOperationContractId::from("conduit.host/file-read@1"),
        envelope_id: CapabilityEnvelopeId::from("file/read-only/project-source/max-64"),
        maximum_parameter_bytes: 256,
        maximum_result_bytes: 512,
        maximum_work_units: 32,
        maximum_in_flight: 4,
        maximum_operations: 10,
    }
}

fn request() -> CapabilityIssueRequest {
    CapabilityIssueRequest {
        scope: scope(),
        authority: authority(),
    }
}

fn table(key: u8) -> BaseCapabilityTable {
    BaseCapabilityTable::new(
        HostId::from("host/one"),
        BootId::from("boot/one"),
        BaseInstanceId::from("base/files/instance"),
        4,
        [key; 32],
        4,
    )
    .unwrap()
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
        parameter_bytes: 32,
        work_units: 2,
    }
}

#[test]
fn issue_narrows_authority_and_exposes_only_non_secret_inspection() {
    let mut table = table(7);
    let handle = table.issue(request()).unwrap();
    let inspection = table.inspections().next().unwrap();

    assert_eq!(inspection.lifecycle, CapabilityLifecycle::Issued);
    assert_eq!(inspection.scope.maximum_operations, 3);
    assert_ne!(
        inspection.possession_id.as_str(),
        authority().grant.grant_id.as_str()
    );
    assert_eq!(format!("{handle:?}"), "BaseCapabilityHandle(<redacted>)");

    let mut broadened = request();
    broadened.scope.maximum_parameter_bytes = 257;
    assert_eq!(
        table.issue(broadened),
        Err(BaseCapabilityRefusal::ScopeBroadening)
    );
}

#[test]
fn copied_ids_and_another_issuers_bearer_cannot_manufacture_privilege() {
    let mut real_provider = table(7);
    let mut attacker_table = table(9);
    let attacker_handle = attacker_table.issue(request()).unwrap();

    // The attacker knows every descriptive ID and can recreate the entire
    // public claim. A bearer minted under another private issuer key is still
    // unknown at the actual provider boundary.
    assert_eq!(
        real_provider.authorize(&attacker_handle, &claim()),
        Err(BaseCapabilityRefusal::UnknownCapability)
    );
    assert!(real_provider.inspections().next().is_none());
}

#[test]
fn provider_checks_every_exact_scope_field_at_operation_time() {
    type ClaimMutation = Box<dyn Fn(&mut BaseOperationClaim)>;
    let mut table = table(7);
    let handle = table.issue(request()).unwrap();
    let mutations: Vec<ClaimMutation> = vec![
        Box::new(|claim| claim.host_id = HostId::from("host/sibling")),
        Box::new(|claim| claim.boot_id = BootId::from("boot/stale")),
        Box::new(|claim| claim.base_instance_id = BaseInstanceId::from("base/network")),
        Box::new(|claim| claim.base_provider_generation += 1),
        Box::new(|claim| claim.plan_id = PlanId::from("plan/forged")),
        Box::new(|claim| claim.active_play_id = ActivePlayId::from("play/sibling")),
        Box::new(|claim| claim.implementation_id = ImplementationId::from("implementation/evil")),
        Box::new(|claim| {
            claim.operation_contract_id = HostOperationContractId::from("conduit.host/file-write@1")
        }),
        Box::new(|claim| claim.subject_kind = KindId::from("file/sibling")),
        Box::new(|claim| claim.resource_pool_id = ResourcePoolId::from("files/sibling")),
        Box::new(|claim| {
            claim.resource_generation_id = ResourceGenerationId("resource-generation/8".into())
        }),
        Box::new(|claim| claim.envelope_id = CapabilityEnvelopeId::from("file/all")),
    ];

    for mutate in mutations {
        let mut forged = claim();
        mutate(&mut forged);
        assert_eq!(
            table.authorize(&handle, &forged),
            Err(BaseCapabilityRefusal::WrongScope)
        );
    }
    let mut oversized = claim();
    oversized.parameter_bytes = 65;
    assert_eq!(
        table.authorize(&handle, &oversized),
        Err(BaseCapabilityRefusal::ParameterEnvelope)
    );
    let mut too_much_work = claim();
    too_much_work.work_units = 9;
    assert_eq!(
        table.authorize(&handle, &too_much_work),
        Err(BaseCapabilityRefusal::WorkEnvelope)
    );
}

#[test]
fn finite_pressure_completion_and_one_shot_replay_are_distinct() {
    let mut table = table(7);
    let mut one_shot = request();
    one_shot.scope.maximum_in_flight = 1;
    one_shot.scope.maximum_operations = 1;
    let handle = table.issue(one_shot).unwrap();
    let lease = table.authorize(&handle, &claim()).unwrap();

    assert_eq!(
        table.authorize(&handle, &claim()),
        Err(BaseCapabilityRefusal::InFlightFull)
    );
    assert_eq!(
        table.complete(lease.clone(), 129),
        Err(BaseCapabilityRefusal::ResultEnvelope)
    );
    table.complete(lease.clone(), 64).unwrap();
    assert_eq!(
        table.complete(lease, 64),
        Err(BaseCapabilityRefusal::UnknownLease)
    );
    assert_eq!(
        table.authorize(&handle, &claim()),
        Err(BaseCapabilityRefusal::Exhausted)
    );
}

#[test]
fn revocation_cancellation_and_replacement_make_possession_stale() {
    let mut provider = table(7);
    let handle = provider.issue(request()).unwrap();
    let lease = provider.authorize(&handle, &claim()).unwrap();
    provider.revoke(&handle).unwrap();
    assert_eq!(
        provider.authorize(&handle, &claim()),
        Err(BaseCapabilityRefusal::Revoked)
    );
    assert_eq!(
        provider.complete(lease, 64),
        Err(BaseCapabilityRefusal::StaleCompletion)
    );

    let mut replacement_boot = BaseCapabilityTable::new(
        HostId::from("host/one"),
        BootId::from("boot/two"),
        BaseInstanceId::from("base/files/replacement"),
        5,
        [11; 32],
        4,
    )
    .unwrap();
    assert_eq!(
        replacement_boot.authorize(&handle, &claim()),
        Err(BaseCapabilityRefusal::UnknownCapability)
    );

    let mut cancellation = table(7);
    let cancelled = cancellation.issue(request()).unwrap();
    cancellation.revoke_play(&ActivePlayId::from("play/one"));
    assert_eq!(
        cancellation.authorize(&cancelled, &claim()),
        Err(BaseCapabilityRefusal::Revoked)
    );

    let mut replacements = table(7);
    let plan_stale = replacements.issue(request()).unwrap();
    replacements.revoke_plan(&PlanId::from("plan/one"));
    assert_eq!(
        replacements.authorize(&plan_stale, &claim()),
        Err(BaseCapabilityRefusal::Revoked)
    );

    let mut authority_revoked = table(7);
    let grant_stale = authority_revoked.issue(request()).unwrap();
    authority_revoked.revoke_authority(&AuthorityGrantId::from("grant/visible"));
    assert_eq!(
        authority_revoked.authorize(&grant_stale, &claim()),
        Err(BaseCapabilityRefusal::Revoked)
    );

    let mut resource_replaced = table(7);
    let resource_stale = resource_replaced.issue(request()).unwrap();
    resource_replaced.revoke_resource_generation(
        &ResourcePoolId::from("files/project/source"),
        &ResourceGenerationId("resource-generation/7".into()),
    );
    assert_eq!(
        resource_replaced.authorize(&resource_stale, &claim()),
        Err(BaseCapabilityRefusal::Revoked)
    );
}
