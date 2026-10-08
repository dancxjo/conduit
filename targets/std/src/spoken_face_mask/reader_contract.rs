//! Commands, outcomes, and refusals for the mechanical Face reader.

use conduit_presentation::{
    FaceInteraction, FaceInteractionRefusal, FaceReadingRefusal, FaceUtterancePlanError,
    ManifestationLifecycle, MaskShow, Presentation, PresentationRole,
};

use super::SpokenTurnReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReaderCommand {
    Help,
    ReadAll,
    /// Read the currently primary Item subjects without selected-detail spill.
    ReadCurrentItems,
    Next,
    Previous,
    Repeat,
    NextSubject,
    PreviousSubject,
    NextAction,
    PreviousAction,
    NextRole(PresentationRole),
    PreviousRole(PresentationRole),
    FocusSubject(String),
    /// Move to an action offered by this exact Face without invoking it.
    FocusAction(String),
    Stop,
    /// Set one complete typed value on the action currently in focus.
    Edit {
        argument: String,
        value: Vec<u8>,
    },
    Activate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpokenFaceRefusal {
    InvalidFace(FaceUtterancePlanError),
    StaleFace,
    StaleShow,
    UnavailableShow,
    EmptyFace,
    NoActionInFocus,
    UnknownSubject,
    UnknownAction,
    UnknownArgument,
    UnsupportedValueKind,
    InvalidValue,
    VoiceBound,
    SpeechPressure,
    SpeechReceipt,
    Interaction(FaceInteractionRefusal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaderResult {
    pub interaction: Option<FaceInteraction>,
    pub reading: bool,
    pub focused_clause: usize,
    pub cancel_stream_identity: Option<String>,
    pub interrupted: Option<SpokenTurnReceipt>,
}

/// A deterministic text readout for an attached screen reader or terminal.
/// No synthesis, audio output, or spoken Show completion is implied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpokenTextReadout {
    pub face_id: String,
    pub face_revision: u64,
    pub show_id: String,
    pub clauses: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) enum Reading {
    Clauses {
        current: Option<usize>,
        offset: usize,
    },
    Message {
        text: String,
        offset: usize,
    },
}

pub(super) fn reading_refusal(refusal: FaceReadingRefusal) -> SpokenFaceRefusal {
    match refusal {
        FaceReadingRefusal::InvalidFace(error) => SpokenFaceRefusal::InvalidFace(error),
        FaceReadingRefusal::EmptyFace => SpokenFaceRefusal::EmptyFace,
        FaceReadingRefusal::StaleFace => SpokenFaceRefusal::StaleFace,
        FaceReadingRefusal::UnknownAction => SpokenFaceRefusal::UnknownAction,
        FaceReadingRefusal::UnknownSubject => SpokenFaceRefusal::UnknownSubject,
    }
}

pub(super) fn check_show(face: &Presentation, show: &MaskShow) -> Result<(), SpokenFaceRefusal> {
    show.validate(face)
        .map_err(|_| SpokenFaceRefusal::StaleShow)?;
    if show.show.lifecycle != ManifestationLifecycle::Available {
        return Err(SpokenFaceRefusal::UnavailableShow);
    }
    Ok(())
}
