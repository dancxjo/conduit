//! Run the ordinary startup Plot after USB-powered peripherals are initialized.
use super::super::refuse;
use conduitos::{
    allocation::BOOT_ARENA, arch, identity::BootIdentities, make::EMBEDDED_MAKE, offer::HostOffer,
};
pub(super) fn play(ids: &BootIdentities, offer: &HostOffer<'_>) -> Option<arch::PwmAudio> {
    let mut audio = arch::PwmAudio::prepare().ok();
    let mut startup = {
        use conduitos::pcm_output::PcmOutput;
        arch::present(b"CONDUIT_BOOT_STAGE startup-plot-prepare\n");
        conduitos::startup_plot::StartupPlot::prepare(
            ids,
            offer,
            EMBEDDED_MAKE.build_id,
            audio.as_ref().map(PcmOutput::provider),
        )
        .unwrap_or_else(|reason| refuse(reason))
    };
    {
        use conduitos::pcm_output::PcmOutput;
        let before = BOOT_ARENA.attempts();
        let receipt = startup
            .run(audio.as_mut().map(|device| device as &mut dyn PcmOutput))
            .unwrap_or_else(|reason| refuse(reason));
        let allocation_stable = BOOT_ARENA.attempts() == before;
        let evidence = serde_json::to_vec(&serde_json::json!({
            "source":"plots/startup-chime/main.conduit",
            "body_id":receipt.body_id.as_str(),"plan_id":receipt.plan_id.as_str(),
            "active_play_id":receipt.active_play_id.as_str(),"outcome":receipt.outcome,
            "frames":receipt.frames,"allocation_stable_during_play":allocation_stable,
            "speaker_heard_claimed":false
        }))
        .unwrap_or_else(|_| refuse("startup-evidence-encoding"));
        if evidence.len() > 2048 {
            refuse("startup-evidence-bound");
        }
        arch::present(b"CONDUIT_STARTUP_PLOT ");
        arch::present(&evidence);
        arch::present(b"\n");
    }
    if let Some(observation) = audio.as_ref().and_then(arch::PwmAudio::last_playback) {
        let evidence = serde_json::to_vec(&serde_json::json!({
            "frames_queued":observation.frames_queued,"elapsed_us":observation.elapsed_us,
            "pwm_status":observation.pwm_status,"completed":observation.completed,
            "failure":observation.failure.map(|error|error.as_str())
        }))
        .unwrap_or_else(|_| refuse("pcm-evidence-encoding"));
        arch::present(b"CONDUIT_PCM_PLAYBACK ");
        arch::present(&evidence);
        arch::present(b"\n");
    }
    drop(startup);
    audio
}
