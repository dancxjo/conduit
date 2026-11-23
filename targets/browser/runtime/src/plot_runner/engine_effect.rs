//! Internal admitted browser Host Call effects.
use super::*;

pub(in crate::plot_runner) struct PendingHostEffect {
    pub request: HostCallRequest,
    pub effect: BrowserHostEffect,
}

pub(in crate::plot_runner) enum BrowserHostEffect {
    WombPrepare,
    AudioCue,
    AudioCapture,
    PcmPlayback { frame: Option<Vec<u8>> },
    PitchTone { hertz: u32 },
    ClockObservation,
    CivilClockObservation,
    Timer { duration_millis: u64 },
    Snapshot { publish: bool },
    KeyEvent,
    PointerEvent,
    ButtonTransition,
    ApplicationEvent,
    TextEditor(crate::installed_browser::text_editor::EditorDocument),
    TutorialPresenterRequest,
    Manifestation(BrowserManifestation),
}
