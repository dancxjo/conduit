//! Owned custody of an explicit effect-owner assertion after native queue validation.
//! This preserves an utterance interpretation epoch, not a completed-token frontier.
use crate::{
    native_playback_back::{NativeSpeechPlaybackBack, PlaybackAcknowledgementRefusal},
    playback_basis::PreparedSpeechPlaybackTape,
    semantic::{SpeechPlaybackAcknowledgement, SpeechUtteranceIntent},
};
use alloc::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackEffectCoverage {
    /// The first irreversible frame protects the original interpretation epoch.
    /// No exact occurrence-to-frame coverage or played-token frontier is implied.
    EpochOnly,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnedPlaybackAcknowledgementRefusal {
    ForeignTapeOwner,
    Acknowledgement(PlaybackAcknowledgementRefusal),
}
/// Strong owners retain the exact tape and full assertion. The tape's source,
/// timing and linguistic owners remain borrowed for this same lifetime.
/// Device authority is still the responsibility of the supplied effect owner.
pub struct OwnedPlayedTapeEvidence<'a> {
    tape: Rc<PreparedSpeechPlaybackTape<'a>>,
    acknowledgement: Rc<SpeechPlaybackAcknowledgement>,
}
impl<'a> OwnedPlayedTapeEvidence<'a> {
    pub fn tape(&self) -> &PreparedSpeechPlaybackTape<'a> {
        &self.tape
    }
    pub fn acknowledgement(&self) -> &SpeechPlaybackAcknowledgement {
        &self.acknowledgement
    }
    pub fn source(&self) -> &'a SpeechUtteranceIntent {
        self.tape.source()
    }
    pub fn first_frame(&self) -> u64 {
        *self.acknowledgement.first_frame()
    }
    pub fn through_frame(&self) -> u64 {
        *self.acknowledgement.through_frame()
    }
    pub fn coverage(&self) -> PlaybackEffectCoverage {
        PlaybackEffectCoverage::EpochOnly
    }
}
/// Queued assertions return no played evidence. An equal-valued foreign tape
/// refuses before the native acknowledgment state can change. Cancellation
/// cannot erase an effect owner's subsequent assertion about queued audio.
pub fn acknowledge_owned<'a>(
    back: &mut NativeSpeechPlaybackBack<'a>,
    tape: Rc<PreparedSpeechPlaybackTape<'a>>,
    acknowledgement: Rc<SpeechPlaybackAcknowledgement>,
) -> Result<Option<OwnedPlayedTapeEvidence<'a>>, OwnedPlaybackAcknowledgementRefusal> {
    if !core::ptr::eq(back.tape(), tape.as_ref()) {
        return Err(OwnedPlaybackAcknowledgementRefusal::ForeignTapeOwner);
    }
    let played = back
        .acknowledge(acknowledgement.as_ref())
        .map_err(OwnedPlaybackAcknowledgementRefusal::Acknowledgement)?
        .is_some();
    Ok(if played {
        Some(OwnedPlayedTapeEvidence {
            tape,
            acknowledgement,
        })
    } else {
        None
    })
}
