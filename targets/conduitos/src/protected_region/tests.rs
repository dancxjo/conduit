use super::*;
use crate::protection_domain::{
    KernelCapabilityRefusal, KernelCapabilityScope, KernelOperationClaim,
};

#[test]
fn binding_uses_verified_ordinary_plan_and_exact_active_play() {
    use crate::{
        identity::BootIdentities,
        offer::{CpuFeatures, HostOffer},
    };
    let ids = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &ids,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        256 * 1024,
    );
    let prepared = crate::ordinary_plan::prepare(&ids, &offer, "build").unwrap();
    let id = &prepared.plan.fragments[0].execution_regions[0].region_id;
    let domain = ProtectionDomainId(1);
    assert!(RegionBinding::admit(&prepared.plan, &prepared.active_play, id, domain).is_ok());
    assert_eq!(
        RegionBinding::admit(
            &prepared.plan,
            &prepared.active_play,
            id,
            ProtectionDomainId(0)
        ),
        Err(DomainRefusal::WrongBinding)
    );
    assert_eq!(
        RegionBinding::admit(
            &prepared.plan,
            &prepared.active_play,
            &"region/absent".into(),
            domain
        ),
        Err(DomainRefusal::WrongBinding)
    );
    let mut forged = prepared.active_play.clone();
    forged.play_sequence += 1;
    assert_eq!(
        RegionBinding::admit(&prepared.plan, &forged, id, domain),
        Err(DomainRefusal::WrongBinding)
    );
    let mut modified = prepared.plan.clone();
    modified.fragments[0].execution_regions[0].region_id = "region/changed".into();
    assert_eq!(
        RegionBinding::admit(&modified, &prepared.active_play, id, domain),
        Err(DomainRefusal::WrongBinding)
    );
}

struct TestBackend {
    result: Result<DomainReturn, DomainRefusal>,
    entered: u32,
    quarantined: bool,
}

impl DomainBackend for TestBackend {
    fn enter(&mut self, work: u32) -> Result<DomainReturn, DomainRefusal> {
        self.entered += 1;
        assert_eq!(work, 100);
        self.result
    }
    fn quarantine(&mut self) {
        self.quarantined = true;
    }
    fn cost(&self) -> DomainCost {
        DomainCost::default()
    }
}

fn binding() -> RegionBinding {
    RegionBinding {
        active: bind_active_play(&"plan".into(), &"host".into(), &"boot".into(), 1),
        region: "region/0".into(),
        domain: ProtectionDomainId(1),
    }
}

fn region(result: Result<DomainReturn, DomainRefusal>) -> ProtectedRegion<TestBackend> {
    ProtectedRegion::installed(
        binding(),
        TestBackend {
            result,
            entered: 0,
            quarantined: false,
        },
    )
}

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
        maximum_operations: 2,
    }
}

fn claim() -> KernelOperationClaim {
    let scope = scope();
    KernelOperationClaim {
        boot: scope.boot,
        plan: scope.plan,
        play: scope.play,
        base_generation: 1,
        resource_generation: 1,
        operation: 8,
        parameter_bytes: 1,
        work_units: 1,
    }
}

#[test]
fn terminal_returns_revoke_in_flight_authority_and_preserve_sibling() {
    for result in [
        Ok(DomainReturn::Completed),
        Ok(DomainReturn::Fault(DomainFault::Memory)),
        Err(DomainRefusal::BackendFailure),
    ] {
        let mut table = KernelCapabilityTable::new(123).unwrap();
        let owner = binding().domain;
        let sibling = ProtectionDomainId(2);
        let handle = table.issue(owner, scope()).unwrap();
        let sibling_handle = table.issue(sibling, scope()).unwrap();
        let lease = table.authorize(owner, handle, claim()).unwrap();
        let mut domain = region(result);
        assert_eq!(domain.resume(&binding(), 100, &mut table), result);
        assert!(domain.backend.quarantined);
        assert_eq!(
            table.complete(lease),
            Err(KernelCapabilityRefusal::StaleLease)
        );
        assert_eq!(
            table.authorize(owner, handle, claim()),
            Err(KernelCapabilityRefusal::Revoked)
        );
        assert!(table.authorize(sibling, sibling_handle, claim()).is_ok());
        assert_eq!(
            domain.resume(&binding(), 100, &mut table),
            Err(DomainRefusal::InvalidLifecycle)
        );
        assert_eq!(domain.backend.entered, 1);
    }
}

#[test]
fn yield_and_preemption_keep_current_domain_resumable() {
    for result in [
        DomainReturn::Yielded,
        DomainReturn::Preempted,
        DomainReturn::Gate,
    ] {
        let mut table = KernelCapabilityTable::new(123).unwrap();
        let mut domain = region(Ok(result));
        for _ in 0..2 {
            assert_eq!(domain.resume(&binding(), 100, &mut table), Ok(result));
            assert_eq!(domain.state(), DomainState::Suspended);
            assert!(!domain.backend.quarantined);
        }
    }
}

#[test]
fn mismatched_play_region_domain_and_zero_work_never_enter_backend() {
    let mut table = KernelCapabilityTable::new(123).unwrap();
    let mut domain = region(Ok(DomainReturn::Completed));
    let mut other = binding();
    other.active = bind_active_play(&"plan".into(), &"host".into(), &"boot".into(), 2);
    assert_eq!(
        domain.resume(&other, 100, &mut table),
        Err(DomainRefusal::WrongBinding)
    );
    other = binding();
    other.region = "region/sibling".into();
    assert_eq!(
        domain.resume(&other, 100, &mut table),
        Err(DomainRefusal::WrongBinding)
    );
    other = binding();
    other.domain = ProtectionDomainId(2);
    assert_eq!(
        domain.resume(&other, 100, &mut table),
        Err(DomainRefusal::WrongBinding)
    );
    assert_eq!(
        domain.resume(&binding(), 0, &mut table),
        Err(DomainRefusal::WorkExhausted)
    );
    assert_eq!(domain.backend.entered, 0);
}

#[test]
fn memory_admission_refuses_empty_code_stack_and_overflow() {
    let exact = DomainMemory {
        executable_bytes: 4096,
        private_bytes: 4096,
        stack_bytes: 4096,
        input_bytes: 4096,
        output_bytes: 4096,
    };
    assert_eq!(exact.total(), Ok(20480));
    assert_eq!(
        DomainMemory {
            executable_bytes: 0,
            ..exact
        }
        .total(),
        Err(DomainRefusal::InvalidMemory)
    );
    assert_eq!(
        DomainMemory {
            stack_bytes: 0,
            ..exact
        }
        .total(),
        Err(DomainRefusal::InvalidMemory)
    );
    assert_eq!(
        DomainMemory {
            private_bytes: u32::MAX,
            ..exact
        }
        .total(),
        Err(DomainRefusal::InvalidMemory)
    );
}
