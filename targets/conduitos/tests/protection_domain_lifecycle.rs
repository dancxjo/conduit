use conduitos::protection_domain::{
    KernelCapabilityRefusal, KernelCapabilityScope, KernelCapabilityTable, KernelOperationClaim,
    KernelRevocationCause, KernelRevocationReceipt, ProtectionDomainId,
};

fn scope() -> KernelCapabilityScope {
    KernelCapabilityScope {
        host: [1; 32],
        boot: [2; 32],
        plan: [3; 32],
        play: [4; 32],
        implementation: [5; 32],
        base: [6; 32],
        base_generation: 1,
        resource: [7; 32],
        resource_generation: 1,
        operation: 8,
        subject: [9; 32],
        authority: [10; 32],
        maximum_parameter_bytes: 1,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 1,
    }
}

fn claim(scope: &KernelCapabilityScope) -> KernelOperationClaim {
    KernelOperationClaim {
        boot: scope.boot,
        plan: scope.plan,
        play: scope.play,
        base_generation: scope.base_generation,
        resource_generation: scope.resource_generation,
        operation: scope.operation,
        parameter_bytes: 1,
        work_units: 1,
    }
}

#[test]
fn every_owned_lifecycle_event_fences_the_old_handle() {
    for cause in [
        KernelRevocationCause::PlayCancelled,
        KernelRevocationCause::PlayCompleted,
        KernelRevocationCause::PlanReplaced,
        KernelRevocationCause::AuthorityRevoked,
        KernelRevocationCause::ResourceReplaced,
        KernelRevocationCause::BaseReplaced,
        KernelRevocationCause::BootReplaced,
    ] {
        let domain = ProtectionDomainId(1);
        let exact = scope();
        let mut table = KernelCapabilityTable::new(123).unwrap();
        let handle = table.issue(domain, exact).unwrap();
        assert_eq!(
            table.revoke_domain(domain, cause),
            KernelRevocationReceipt {
                cause,
                revoked_handles: 1,
            }
        );
        assert_eq!(
            table.authorize(domain, handle, claim(&exact)),
            Err(KernelCapabilityRefusal::Revoked)
        );
    }
}
