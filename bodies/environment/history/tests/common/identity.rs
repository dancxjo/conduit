use conduit_core::{ActivePlayId, BootId, HostId, PlanId};
use conduit_environment_runtime::recording::RecordingContext;
use conduit_face::FaceContributionBasis;
pub fn context() -> RecordingContext<'static> {
    static HOST: std::sync::LazyLock<HostId> = std::sync::LazyLock::new(|| "sensor/test".into());
    static BOOT: std::sync::LazyLock<BootId> = std::sync::LazyLock::new(|| "boot/test".into());
    static PLAN: std::sync::LazyLock<PlanId> = std::sync::LazyLock::new(|| "plan/sensor".into());
    static PLAY: std::sync::LazyLock<ActivePlayId> =
        std::sync::LazyLock::new(|| "play/sensor".into());
    RecordingContext {
        host: &HOST,
        boot: &BOOT,
        plan: &PLAN,
        play: &PLAY,
    }
}
pub fn basis() -> FaceContributionBasis {
    FaceContributionBasis {
        checked_plot_id: "checked/history".into(),
        plan_id: "plan/reader".into(),
        active_play_id: "play/reader".into(),
        required_interaction_context: None,
    }
}
