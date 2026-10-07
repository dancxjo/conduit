extern crate alloc;
#[path = "../src/parser_custody_budget.rs"]
mod budget;
use budget::*;
fn limits() -> Limits {
    Limits {
        retained: Usage {
            origins: 4,
            facts: 8,
            rebases: 3,
            snapshots: 8,
            commits: 4,
            bytes: 100,
        },
        peak_bytes: 150,
    }
}
#[test]
fn reservation_drop_pressure_and_cancel_preserve_prior_custody_counts() {
    let mut b = Budget::new(limits(), Usage::default()).unwrap();
    let first = Usage {
        origins: 1,
        facts: 1,
        snapshots: 1,
        bytes: 40,
        ..Usage::default()
    };
    {
        let _reservation = b.reserve(first, 40).unwrap();
    }
    assert_eq!(b.used(), Usage::default());
    b.reserve(first, 40).unwrap().publish().unwrap();
    assert_eq!(b.used(), first);
    for proposed in [
        Usage {
            origins: 5,
            ..first
        },
        Usage { facts: 9, ..first },
        Usage {
            rebases: 4,
            ..first
        },
        Usage {
            snapshots: 9,
            ..first
        },
        Usage {
            commits: 5,
            ..first
        },
        Usage {
            bytes: 101,
            ..first
        },
        Usage {
            origins: 0,
            ..first
        },
    ] {
        assert!(matches!(b.reserve(proposed, 1), Err(Refusal::Pressure)));
        assert_eq!(b.used(), first);
    }
    assert!(matches!(b.reserve(first, 111), Err(Refusal::Pressure)));
    assert_eq!(b.used(), first);
    let cancellation = b.cancellation();
    let ticket = b.reserve(Usage { facts: 2, ..first }, 1).unwrap();
    cancellation.cancel();
    assert_eq!(ticket.publish(), Err(Refusal::Cancelled));
    assert_eq!(b.used(), first);
    assert!(matches!(b.reserve(first, 0), Err(Refusal::Cancelled)));
}
#[test]
fn exact_capacity_boundary_and_checked_peak_overflow() {
    let mut b = Budget::new(limits(), Usage::default()).unwrap();
    b.reserve(limits().retained, 150)
        .unwrap()
        .publish()
        .unwrap();
    assert_eq!(b.used(), limits().retained);
    let full = Usage {
        bytes: u64::MAX,
        ..Usage::default()
    };
    let mut b = Budget::new(
        Limits {
            retained: full,
            peak_bytes: u64::MAX,
        },
        full,
    )
    .unwrap();
    assert!(matches!(b.reserve(full, 1), Err(Refusal::Pressure)));
    assert_eq!(b.used(), full);
    assert!(matches!(
        Budget::new(
            Limits {
                retained: Usage {
                    origins: 5,
                    ..limits().retained
                },
                ..limits()
            },
            Usage::default()
        ),
        Err(Refusal::Limits)
    ));
}
