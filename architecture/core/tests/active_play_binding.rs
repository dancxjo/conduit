use conduit_core::{bind_active_play, BootId, HostId, PlanId};

#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[test]
fn borrowed_verification_matches_canonical_owned_binding_without_allocation() {
    for (plan, host, boot) in [
        ("plan/a", "host/b", "boot/c"),
        ("", "", ""),
        ("🌱", "h/é", "boot/温室"),
    ] {
        let (plan, host, boot) = (PlanId::from(plan), HostId::from(host), BootId::from(boot));
        for sequence in [0, 1, u64::MAX] {
            let active = bind_active_play(&plan, &host, &boot, sequence);
            let (valid, cost) =
                allocation_probe::observe(|| active.is_bound_to(&plan, &host, &boot, sequence));
            assert!(valid);
            assert_eq!(cost.allocations, 0);
            assert_eq!(cost.reallocations, 0);
            assert_eq!(cost.peak_bytes, 0);
        }
    }
}

#[test]
fn borrowed_verification_refuses_changed_scope_and_each_digest_digit() {
    let (plan, host, boot) = (
        PlanId::from("plan/a"),
        HostId::from("host/b"),
        BootId::from("boot/c"),
    );
    let active = bind_active_play(&plan, &host, &boot, 7);
    assert!(!active.is_bound_to(&"other-plan".into(), &host, &boot, 7));
    assert!(!active.is_bound_to(&plan, &"other-host".into(), &boot, 7));
    assert!(!active.is_bound_to(&plan, &host, &"other-boot".into(), 7));
    assert!(!active.is_bound_to(&plan, &host, &boot, 8));
    for index in 0..64 {
        let mut changed = active.clone();
        let mut digits = changed.active_play_id.as_str().as_bytes().to_vec();
        digits[index] = if digits[index] == b'0' { b'1' } else { b'0' };
        changed.active_play_id = String::from_utf8(digits).unwrap().into();
        assert!(
            !changed.is_bound_to(&plan, &host, &boot, 7),
            "digit {index}"
        );
    }
    for malformed in [
        "".to_owned(),
        "0".repeat(63),
        "0".repeat(65),
        active.active_play_id.as_str().to_uppercase(),
    ] {
        let mut changed = active.clone();
        changed.active_play_id = malformed.into();
        assert!(!changed.is_bound_to(&plan, &host, &boot, 7));
    }
    // Matching the digest alone does not excuse substituted descriptive fields.
    for field in 0..4 {
        let mut changed = active.clone();
        match field {
            0 => changed.plan_id = "other-plan".into(),
            1 => changed.host_id = "other-host".into(),
            2 => changed.boot_id = "other-boot".into(),
            _ => changed.play_sequence = 8,
        }
        assert!(!changed.is_bound_to(&plan, &host, &boot, 7));
    }
}
