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
fn every_current_authority_dimension_is_rechecked_before_a_gate_lease() {
    let domain = ProtectionDomainId(1);
    let exact = scope();
    let mut table = KernelCapabilityTable::new(123).unwrap();
    let handle = table.issue(domain, exact).unwrap();
    for mutation in 0..17 {
        let mut current = exact;
        match mutation {
            0 => current.host[0] ^= 1,
            1 => current.boot[0] ^= 1,
            2 => current.plan[0] ^= 1,
            3 => current.play[0] ^= 1,
            4 => current.implementation[0] ^= 1,
            5 => current.base[0] ^= 1,
            6 => current.base_generation += 1,
            7 => current.resource[0] ^= 1,
            8 => current.resource_generation += 1,
            9 => current.operation += 1,
            10 => current.subject[0] ^= 1,
            11 => current.authority[0] ^= 1,
            12 => current.maximum_parameter_bytes += 1,
            13 => current.maximum_work_units += 1,
            14 => current.maximum_in_flight += 1,
            15 => current.maximum_operations += 1,
            16 => current.maximum_parameter_bytes = 0,
            _ => unreachable!(),
        }
        assert_eq!(
            table.authorize_current(domain, handle, &current, claim(&exact)),
            Err(KernelCapabilityRefusal::WrongScope),
            "mutation {mutation}",
        );
    }
    let lease = table
        .authorize_current(domain, handle, &exact, claim(&exact))
        .unwrap();
    table.complete(lease).unwrap();
    assert_eq!(
        table.authorize_current(domain, handle, &exact, claim(&exact)),
        Err(KernelCapabilityRefusal::Exhausted)
    );
}

#[test]
fn every_owned_lifecycle_event_fences_the_old_handle() {
    for cause in [
        KernelRevocationCause::PlayCancelled,
        KernelRevocationCause::PlayCompleted,
        KernelRevocationCause::PlayFailed,
        KernelRevocationCause::PlanReplaced,
        KernelRevocationCause::AuthorityRevoked,
        KernelRevocationCause::ResourceReplaced,
        KernelRevocationCause::BaseReplaced,
        KernelRevocationCause::BootReplaced,
        KernelRevocationCause::ProtectionFault,
        KernelRevocationCause::ProviderLost,
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
