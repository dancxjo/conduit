use super::*;
use conduit_core::{BootId, HostId, PlanId};

#[test]
fn borrowed_active_play_check_matches_existing_owned_binder_and_rejects_mutations() {
    for (plan, host, boot) in [("p", "h", "b"), ("計画\n", "Host/🦀", "Boot\n二")] {
        for sequence in [0, 1, 9, 10, u64::MAX] {
            let owned = bind_active_play(
                &PlanId::from(plan),
                &HostId::from(host),
                &BootId::from(boot),
                sequence,
            );
            let id = owned.active_play_id.as_str();
            assert!(active_play_matches(id, plan, host, boot, sequence));
            assert!(!active_play_matches(id, "different", host, boot, sequence));
            assert!(!active_play_matches(id, plan, "different", boot, sequence));
            assert!(!active_play_matches(id, plan, host, "different", sequence));
            assert!(!active_play_matches(
                id,
                plan,
                host,
                boot,
                sequence.wrapping_add(1)
            ));
            assert!(!active_play_matches(&id[..63], plan, host, boot, sequence));
            assert!(!active_play_matches(
                &alloc::format!("{id}0"),
                plan,
                host,
                boot,
                sequence
            ));
            assert!(!active_play_matches(
                &id.to_uppercase(),
                plan,
                host,
                boot,
                sequence
            ));
        }
    }
}
