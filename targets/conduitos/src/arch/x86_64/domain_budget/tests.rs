//! Pure policy/receipt tests. These do not execute RTC hardware or CPL3 entry.
use super::*;
#[test]
fn only_application_can_lower_existing_draft_cap() {
    assert_eq!(EntryBudget::Ordinary.ticks(), 16);
    assert_eq!(EntryBudget::Draft.ticks(), 64);
    for n in [1, 64] {
        assert_eq!(
            EntryBudget::Application(
                ApplicationRemaining::new(ServiceProfile::Application, n).unwrap()
            )
            .ticks(),
            n
        );
    }
    for n in [0, 65, u32::MAX] {
        assert!(ApplicationRemaining::new(ServiceProfile::Application, n).is_err());
    }
    for p in [
        ServiceProfile::Compositor,
        ServiceProfile::Draft,
        ServiceProfile::Home,
        ServiceProfile::Runtime,
        ServiceProfile::Network,
        ServiceProfile::OrdinaryRuntime,
        ServiceProfile::Mask,
        ServiceProfile::Tls,
    ] {
        assert!(ApplicationRemaining::new(p, 1).is_err());
    }
}
#[test]
fn immutable_counts_are_bound_to_instance_sequence_cap_and_actual_return() {
    assert!(EntryIdentity::next(7, u64::MAX).is_err());
    assert!(EntryIdentity::next(0, 0).is_err());
    assert_eq!(EntryIdentity::next(7, 0).unwrap().sequence(), 1);
    assert_eq!(EntryIdentity::next(7, 8).unwrap().sequence(), 9);
    assert!(EntryIdentity::new(0, 1).is_err());
    assert!(EntryIdentity::new(1, 0).is_err());
    let cap = EntryBudget::Application(
        ApplicationRemaining::new(ServiceProfile::Application, 1).unwrap(),
    );
    let first =
        NativeEntryReceipt::capture(EntryIdentity::new(7, 1).unwrap(), cap, (1, 9), 4, 2).unwrap();
    assert_eq!((first.instance(), first.origin()), (7, 2));
    assert_eq!(
        first.outcome(),
        DomainReturn::Fault(DomainFault::WorkExhausted)
    );
    assert_eq!((first.rtc(), first.source(), first.admitted()), (1, 9, 1));
    assert!(first.validate(7, 1).is_ok());
    assert!(first.validate(8, 1).is_err());
    assert!(first.validate(7, 2).is_err());
    let next =
        NativeEntryReceipt::capture(EntryIdentity::new(7, 2).unwrap(), cap, (0, 0), 0, 0).unwrap();
    assert_eq!(next.outcome(), DomainReturn::Yielded);
    assert!(next.validate(7, 1).is_err());
    assert_eq!((first.rtc(), first.source(), first.sequence()), (1, 9, 1));
    assert!(
        NativeEntryReceipt::capture(EntryIdentity::new(7, 3).unwrap(), cap, (2, 0), 0, 0).is_err()
    );
}
#[test]
fn native_fault_classification_is_preserved() {
    for (origin, value, fault) in [
        (1, 0x10d, DomainFault::PrivilegedOperation),
        (1, 0x10e, DomainFault::Memory),
        (1, 4, DomainFault::InvalidInstruction),
        (9, 0, DomainFault::InvalidGate),
    ] {
        assert_eq!(
            NativeEntryReceipt::capture(
                EntryIdentity::new(1, 1).unwrap(),
                EntryBudget::Draft,
                (0, 0),
                value,
                origin
            )
            .unwrap()
            .outcome(),
            DomainReturn::Fault(fault)
        );
    }
}

#[test]
fn every_profile_preserves_gate_and_preemption_accounting_without_charging_other_faults() {
    for budget in [
        EntryBudget::Ordinary,
        EntryBudget::Draft,
        EntryBudget::Application(
            ApplicationRemaining::new(ServiceProfile::Application, 1).unwrap(),
        ),
        EntryBudget::Application(
            ApplicationRemaining::new(ServiceProfile::Application, 64).unwrap(),
        ),
    ] {
        for (origin, value, gates, preemptions) in [
            (0, 0, 1, 0),
            (0, 0x200, 1, 0),
            (2, 4, 0, 1),
            (1, 0x10d, 0, 0),
            (1, 0x10e, 0, 0),
            (1, 4, 0, 0),
            (9, 0, 0, 0),
        ] {
            let receipt = NativeEntryReceipt::capture(
                EntryIdentity::new(7, 17).unwrap(),
                budget,
                (0, 0),
                value,
                origin,
            )
            .unwrap();
            let mut actual = DomainCost {
                entries: 17,
                gate_transitions: 5,
                preemptions: 2,
                interrupt_entries: 21,
                source_timer_interrupts: 9,
                ..DomainCost::default()
            };
            let mut expected = actual;
            expected.gate_transitions += gates;
            expected.preemptions += preemptions;
            receipt.account_outcome(&mut actual);
            assert_eq!(actual, expected);
        }
    }
}
