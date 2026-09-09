//! Bounded preparation of the original cue, and its exact platform effect.
use conduit_synth::{StartupChime, STARTUP_CHIME_FRAMES, STARTUP_CHIME_SCORE_ID};
use serde::Serialize;
use std::cell::RefCell;
const FRAMES: usize = STARTUP_CHIME_FRAMES as usize;
thread_local! { static PCM: RefCell<Option<Box<[i16]>>> = const { RefCell::new(None) }; }

/// Pure finite preparation: this export never acquires or plays a device.
#[no_mangle]
pub extern "C" fn conduit_browser_startup_chime_prepare() -> usize {
    PCM.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let mut frames = vec![0; FRAMES].into_boxed_slice();
            let mut renderer = StartupChime::new();
            for block in frames.chunks_mut(256) {
                renderer.render(block).expect("bounded canonical cue block");
            }
            *slot = Some(frames);
        }
        slot.as_ref().unwrap().as_ptr() as usize
    })
}
#[no_mangle]
pub extern "C" fn conduit_browser_startup_chime_frames() -> usize {
    FRAMES
}

#[derive(Debug, Serialize)]
pub(super) struct AudioEffect {
    pub schema: &'static str,
    pub effect_kind: &'static str,
    pub active_play_id: String,
    pub placement_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub request_sequence: u32,
    pub score_id: &'static str,
    pub frames: u32,
    pub sample_rate: u32,
    pub channels: u8,
    pub source_interaction: Option<crate::source_interaction::SourceInteractionEvidence>,
}
pub(super) fn describe(
    session: &super::TourSession,
    placement: &conduit_core::PlannedGear,
    request: u32,
) -> AudioEffect {
    AudioEffect {
        schema: "conduit.browser/audio-cue-effect@1",
        effect_kind: "audio-cue",
        active_play_id: session.active_play_id.as_str().into(),
        placement_id: placement.placement_id.as_str().into(),
        host_id: session.host_id.as_str().into(),
        boot_id: session.boot_id.as_str().into(),
        request_sequence: request,
        score_id: STARTUP_CHIME_SCORE_ID,
        frames: STARTUP_CHIME_FRAMES,
        sample_rate: 48_000,
        channels: 1,
        source_interaction: session.source_interaction.clone(),
    }
}
