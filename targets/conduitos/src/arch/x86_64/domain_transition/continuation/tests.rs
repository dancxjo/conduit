use super::*;
use crate::protection_domain::ProtectionDomainId;
use conduit_core::{
    BootId, ExecutionRegionId, HostId, PlanId, ProtectedEntryAccounting, ProtectedEntryBudget,
    ProtectedOperation, SliceReturn, bind_active_play,
};
fn binding() -> RegionBinding {
    RegionBinding {
        active: bind_active_play(
            &PlanId::from("plan"),
            &HostId::from("host"),
            &BootId::from("boot"),
            1,
        ),
        region: ExecutionRegionId::from("region"),
        domain: ProtectionDomainId(1),
    }
}
const EXEC: [ExecutableRange; 1] = [ExecutableRange {
    start: 0x4000,
    end: 0x5000,
}];
fn mapping() -> Mapping<'static> {
    Mapping {
        instance: 1,
        cr3: 0x1000,
        image_sha256: [7; 32],
        stack_start: 0x8000,
        stack_end: 0x9000,
        executable: &EXEC,
    }
}
fn context() -> UserContext {
    let mut words = [0; 21];
    for (i, v) in words[..15].iter_mut().enumerate() {
        *v = i as u64 + 1;
    }
    words[16] = 0x4100;
    words[17] = 0x2b;
    words[18] = 0x250ed7;
    words[19] = 0x8ff8;
    words[20] = 0x33;
    UserContext::from_source_irq(
        &words,
        UserSegments {
            ds: 0,
            es: 1,
            fs: 0x28,
            gs: 0x33,
            fs_base: 0,
            gs_base: 0,
        },
    )
    .unwrap()
}
fn operation() -> ProtectedOperation<u64> {
    let mut op = ProtectedOperation::new(ProtectedEntryBudget {
        accounting: ProtectedEntryAccounting::SourceInterruptsV1,
        quantum_interrupts: 2,
        total_interrupts: 7,
    })
    .unwrap();
    op.begin(9, b"command").unwrap();
    op
}
#[test]
fn actual_stack_layout_and_all_supported_state_preserved() {
    let c = context();
    assert_eq!(c.registers.r15, 1);
    assert_eq!(c.registers.rax, 15);
    assert_eq!(c.returned.rflags, 0x250ed7);
    let binding = binding();
    let mapping = mapping();
    let mut owner = Custody::new(&binding, &mapping, 9).unwrap();
    let mut op = operation();
    let slice = op.grant(&9, b"command").unwrap();
    owner.capture(&binding, &mapping, 9, slice, c).unwrap();
    assert_eq!(owner.consume(&binding, &mapping, 9, slice), Ok(c));
    assert_eq!(
        owner.consume(&binding, &mapping, 9, slice),
        Err(Refusal::Consumed)
    );
    // Accounting owns new epoch; context consume used old interrupted identity.
    op.finish_slice(&9, b"command", slice, 2, SliceReturn::Preempted)
        .unwrap();
    let next = op.grant(&9, b"command").unwrap();
    assert_eq!(
        owner.capture(&binding, &mapping, 9, slice, c),
        Err(Refusal::Consumed)
    );
    owner.capture(&binding, &mapping, 9, next, c).unwrap();
    assert_eq!(owner.consume(&binding, &mapping, 9, next), Ok(c));
}
#[test]
fn foreign_owner_and_duplicate_do_not_destroy_current_saved_state() {
    let b = binding();
    let m = mapping();
    let mut owner = Custody::new(&b, &m, 9).unwrap();
    let mut op = operation();
    let slice = op.grant(&9, b"command").unwrap();
    let c = context();
    owner.capture(&b, &m, 9, slice, c).unwrap();
    let mut foreign = b.clone();
    foreign.domain = ProtectionDomainId(2);
    assert_eq!(
        owner.consume(&foreign, &m, 9, slice),
        Err(Refusal::ForeignOwner)
    );
    let mut foreign_map = mapping();
    foreign_map.instance = 2;
    assert_eq!(
        owner.consume(&b, &foreign_map, 9, slice),
        Err(Refusal::ForeignOwner)
    );
    assert_eq!(owner.consume(&b, &m, 10, slice), Err(Refusal::ForeignOwner));
    assert_eq!(
        owner.capture(&b, &m, 9, slice, c),
        Err(Refusal::AlreadyCaptured)
    );
    assert_eq!(owner.consume(&b, &m, 9, slice), Ok(c));
    owner.revoke();
    assert_eq!(owner.capture(&b, &m, 9, slice, c), Err(Refusal::Revoked));
}
#[test]
fn unsupported_state_refuses_without_installing_or_normalizing() {
    let b = binding();
    let m = mapping();
    let mut owner = Custody::new(&b, &m, 9).unwrap();
    let mut op = operation();
    let slice = op.grant(&9, b"command").unwrap();
    for change in 0..9 {
        let mut c = context();
        match change {
            0 => c.returned.rip = 0x5000,
            1 => c.returned.rsp = 0x9001,
            2 => c.returned.cs = 0x08,
            3 => c.returned.ss = 0x10,
            4 => c.returned.rflags |= 0x100,
            5 => c.returned.rflags &= !0x200,
            6 => c.segments.fs_base = 1,
            7 => c.segments.gs = 0x18,
            _ => c.returned.rflags |= 0x4000,
        };
        assert!(owner.capture(&b, &m, 9, slice, c).is_err());
    }
    let c = context();
    owner.capture(&b, &m, 9, slice, c).unwrap();
    assert_eq!(owner.consume(&b, &m, 9, slice), Ok(c));
    let mut words = [0; 21];
    words[15] = 1;
    assert_eq!(
        UserContext::from_source_irq(&words, c.segments),
        Err(Refusal::InvalidFrame)
    );
}
