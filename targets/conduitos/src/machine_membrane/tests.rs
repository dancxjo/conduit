use super::*;
use conduit_core::{
    ActivePlayId, AuthorityContractId, AuthorityGrant, AuthorityGrantId, BaseCapabilityAuthority,
    BaseCapabilityRefusal, BaseCapabilityScope, BaseCapabilityTable, BaseInstanceId,
    BaseOperationClaim, BootId, CapabilityEnvelopeId, CapabilityId, CapabilityIssueRequest,
    HostCallContractId, HostId, ImplementationId, KindId, PlanId, ResourceGenerationId,
    ResourcePoolId,
};

fn scope() -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: HostId::from("host/one"),
        boot_id: BootId::from("boot/one"),
        base_instance_id: BaseInstanceId::from("base/registers/instance"),
        base_provider_generation: 4,
        plan_id: PlanId::from("plan/one"),
        active_play_id: ActivePlayId::from("play/one"),
        authority_grant_id: AuthorityGrantId::from("grant/visible"),
        authority_contract_id: AuthorityContractId::from("authority/machine-register32@2"),
        capability_id: CapabilityId::from("machine/registers/read-write"),
        implementation_id: ImplementationId::from("implementation/file-copy@1"),
        operation_contract_id: HostCallContractId::from("conduit.host/machine-register32@2"),
        subject_kind: KindId::from("machine/memory/mmio/register32"),
        resource_pool_id: ResourcePoolId::from("registers/controller"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        envelope_id: CapabilityEnvelopeId::from("registers/controller/window-8"),
        maximum_parameter_bytes: 16,
        maximum_result_bytes: 4,
        maximum_work_units: 8,
        maximum_in_flight: 2,
        maximum_operations: 100000,
    }
}

fn authority() -> BaseCapabilityAuthority {
    BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: AuthorityGrantId::from("grant/visible"),
            contract_id: AuthorityContractId::from("authority/machine-register32@2"),
            host_call_contract_id: HostCallContractId::from("conduit.host/machine-register32@2"),
            subject_kind: KindId::from("machine/memory/mmio/register32"),
            host_id: HostId::from("host/one"),
            boot_id: BootId::from("boot/one"),
            capability_id: CapabilityId::from("machine/registers/read-write"),
        },
        base_instance_id: BaseInstanceId::from("base/registers/instance"),
        base_provider_generation: 4,
        resource_pool_id: ResourcePoolId::from("registers/controller"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        operation_contract_id: HostCallContractId::from("conduit.host/machine-register32@2"),
        envelope_id: CapabilityEnvelopeId::from("registers/controller/window-8"),
        maximum_parameter_bytes: 256,
        maximum_result_bytes: 512,
        maximum_work_units: 32,
        maximum_in_flight: 4,
        maximum_operations: 100000,
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
        BaseInstanceId::from("base/registers/instance"),
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
        parameter_bytes: 16,
        work_units: 1,
    }
}

fn leaf(registers: &mut [u32], writable: bool) -> RegisterLeaf {
    let mut table = table(7);
    let handle = table.issue(request()).unwrap();
    unsafe {
        RegisterLeaf::from_admitted_mapping(
            table,
            handle,
            claim(),
            registers.as_mut_ptr(),
            RegisterEnvelope {
                bytes: (registers.len() * 4) as u32,
                writable,
            },
        )
        .unwrap()
    }
}

#[test]
fn revoked_possession_cannot_touch_registers() {
    let mut registers = [0x12345678];
    let mut provider = leaf(&mut registers, true);
    assert_eq!(
        provider.invoke(RegisterOperation::Read32 { offset: 0 }),
        Ok(0x12345678)
    );
    provider.revoke().unwrap();
    assert_eq!(
        provider.invoke(RegisterOperation::Write32 {
            offset: 0,
            value: 42
        }),
        Err(RegisterRefusal::Capability(BaseCapabilityRefusal::Revoked))
    );
    assert_eq!(registers, [0x12345678]);
}

#[test]
fn range_alignment_and_direction_are_checked_before_effect() {
    let mut registers = [7, 8];
    let mut provider = leaf(&mut registers, false);
    assert_eq!(
        provider.invoke(RegisterOperation::Read32 { offset: 1 }),
        Err(RegisterRefusal::Alignment)
    );
    assert_eq!(
        provider.invoke(RegisterOperation::Read32 { offset: 8 }),
        Err(RegisterRefusal::Range)
    );
    assert_eq!(
        provider.invoke(RegisterOperation::Read32 {
            offset: u32::MAX - 3
        }),
        Err(RegisterRefusal::Range)
    );
    assert_eq!(
        provider.invoke(RegisterOperation::Write32 {
            offset: 0,
            value: 99
        }),
        Err(RegisterRefusal::ReadOnly)
    );
    assert_eq!(registers, [7, 8]);
    assert_eq!(
        provider.invoke(RegisterOperation::Read32 { offset: 4 }),
        Ok(8)
    );
}

#[test]
fn wrong_boot_or_provider_refuses_without_touching_memory() {
    let mut registers = [7];
    let mut provider = leaf(&mut registers, true);
    provider.claim.boot_id = BootId::from("boot/replacement");
    assert_eq!(
        provider.invoke(RegisterOperation::Write32 {
            offset: 0,
            value: 99
        }),
        Err(RegisterRefusal::Capability(
            BaseCapabilityRefusal::WrongScope
        ))
    );
    assert_eq!(registers, [7]);
}

#[test]
fn sustained_operations_keep_the_same_mapping_and_possession() {
    let mut registers = [0];
    let mut provider = leaf(&mut registers, true);
    for value in 0..100000 {
        assert_eq!(
            provider.invoke(RegisterOperation::Write32 { offset: 0, value }),
            Ok(value)
        );
    }
    assert_eq!(registers, [99999]);
    assert_eq!(
        provider.invoke(RegisterOperation::Read32 { offset: 0 }),
        Err(RegisterRefusal::Capability(
            BaseCapabilityRefusal::Exhausted
        ))
    );
}

#[test]
fn possession_from_another_issuer_cannot_exercise_the_mapping() {
    let mut registers = [7];
    let mut provider = leaf(&mut registers, true);
    let mut stranger = table(8);
    provider.handle = stranger.issue(request()).unwrap();
    assert_eq!(
        provider.invoke(RegisterOperation::Write32 {
            offset: 0,
            value: 99
        }),
        Err(RegisterRefusal::Capability(
            BaseCapabilityRefusal::UnknownCapability
        ))
    );
    assert_eq!(registers, [7]);
}

#[test]
fn wrong_plan_play_provider_and_envelope_refuse_before_effect() {
    for field in 0..4 {
        let mut registers = [7];
        let mut provider = leaf(&mut registers, true);
        match field {
            0 => provider.claim.plan_id = PlanId::from("plan/replacement"),
            1 => provider.claim.active_play_id = ActivePlayId::from("play/replacement"),
            2 => provider.claim.base_provider_generation += 1,
            _ => provider.claim.envelope_id = CapabilityEnvelopeId::from("envelope/other"),
        }
        assert_eq!(
            provider.invoke(RegisterOperation::Write32 {
                offset: 0,
                value: 99
            }),
            Err(RegisterRefusal::Capability(
                BaseCapabilityRefusal::WrongScope
            ))
        );
        assert_eq!(registers, [7]);
    }
}

#[path = "register_call_tests.rs"]
mod register_call_tests;

#[path = "register_binding_tests.rs"]
mod register_binding_tests;
