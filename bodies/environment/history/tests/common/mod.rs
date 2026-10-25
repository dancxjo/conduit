use conduit_core::{ActivePlayId, BootId, HostId, PlanId};
use conduit_environment_history::*;
use conduit_environment_runtime::{
    observation::{ObservationContract, PreparedObservationQuantityForms},
    recording::{PreparedRecordingChunk, RecordingContext, RecordingDisposition as D},
};
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
pub fn chunk(events: &[(u64, u64, bool)]) -> String {
    let contract = ObservationContract::prepare().unwrap();
    let mut forms = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap().to_vec();
    let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
    for &(seq, time, available) in events {
        chunk
            .append(
                seq,
                time,
                if available { D::Available } else { D::Checksum },
                available.then_some(observation.as_slice()),
            )
            .unwrap();
    }
    chunk.seal().unwrap().into()
}
// Some integration targets use committed publication custody instead.
#[allow(dead_code)]
pub fn prepared(chunks: u16) -> PreparedHistoryFace {
    PreparedHistoryFace::prepare(context(), &ObservationContract::prepare().unwrap(), chunks)
        .unwrap()
}
