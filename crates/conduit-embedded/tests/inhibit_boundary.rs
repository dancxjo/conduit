use conduit_core::{
    HazardControlPhase, HazardControlState, HostLifecycleChange, InhibitCause, SemanticHash,
    inhibit_hazardous_host, recover_after_host_change,
};

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

#[test]
fn constrained_local_failure_hook_only_reduces_capability() {
    let armed = HazardControlState {
        phase: HazardControlPhase::Armed,
        profile_identity: hash(5),
        safe_state_identity: hash(6),
        plan: hash(1),
        epoch: 4,
        command_authority: hash(2),
        next_sequence: 8,
        active_until_tick: 30,
        latch_generation: 2,
        latch_identity: hash(3),
    };
    let inhibited = inhibit_hazardous_host(armed, hash(4), InhibitCause::ImplementationFailed);
    assert_eq!(inhibited.phase, HazardControlPhase::Inhibited);
    assert_eq!(inhibited.plan, SemanticHash::from_bytes([0; 32]));
    assert_eq!(inhibited.active_until_tick, 0);
    assert_eq!(inhibited.safe_state_identity, hash(6));

    let after_reboot = recover_after_host_change(inhibited, HostLifecycleChange::Reboot);
    assert_eq!(after_reboot, inhibited);
}
