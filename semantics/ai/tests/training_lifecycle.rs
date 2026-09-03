use conduit_ai::{TrainingLifecycle, TrainingLifecyclePhase, TrainingRefusal};

#[test]
fn lifecycle_is_finite_and_terminal_paths_cleanly_unload() {
    for terminal in [
        TrainingLifecyclePhase::Cancelled,
        TrainingLifecyclePhase::ProviderLost,
        TrainingLifecyclePhase::Failed,
    ] {
        let mut lifecycle = TrainingLifecycle {
            session_identity: [5; 32],
            phase: TrainingLifecyclePhase::Unloaded,
        };
        lifecycle
            .transition(TrainingLifecyclePhase::Loading)
            .unwrap();
        lifecycle.transition(TrainingLifecyclePhase::Ready).unwrap();
        lifecycle
            .transition(TrainingLifecyclePhase::ActiveStep { step: 1 })
            .unwrap();
        lifecycle.transition(terminal).unwrap();
        lifecycle
            .transition(TrainingLifecyclePhase::Unloaded)
            .unwrap();
    }
    let mut lifecycle = TrainingLifecycle {
        session_identity: [5; 32],
        phase: TrainingLifecyclePhase::Ready,
    };
    assert_eq!(
        lifecycle.transition(TrainingLifecyclePhase::ActiveStep { step: 0 }),
        Err(TrainingRefusal::InvalidLifecycleTransition)
    );
}
