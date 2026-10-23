//! Current Language-owned vocative commitment retained through speech preparation.
//! Authored acoustic bindings do not confer commitment or pronunciation authority.
use crate::{linguistic_prosody::*, semantic::SpeechLinguisticProsodyBinding};
use conduit_language::{
    analysis_session::CurrentCommittedDependency, prosody::PreparedRichProsody,
    prosody_commitment::PreparedCommittedProsody, LanguageProsodyDurationIntent,
    LanguageProsodyFocusIntent, LanguageProsodyRateIntent,
};

#[derive(Debug)]
pub enum CommittedSpeechProsodyRefusal {
    Dependency,
    Lexical,
    Rich,
    CarrierCommitment,
    CarrierLexical,
    CarrierAcoustic,
    /// Relative duration/rate/focus need an explicit acoustic profile of their own.
    UnsupportedSymbolicControl,
    Acoustic(LinguisticProsodyRefusal),
}

/// Borrows currentness from the owning analysis session. Advancing that session
/// requires ending this borrow; frozen carrier bytes alone cannot replace it.
pub struct CommittedSpeechProsody<'intent, 'authority, 'session> {
    committed: &'intent PreparedCommittedProsody,
    rich: &'intent PreparedRichProsody,
    authority: &'authority CurrentCommittedDependency<'authority, 'session>,
}
impl<'intent, 'authority, 'session> CommittedSpeechProsody<'intent, 'authority, 'session> {
    pub fn prepare(
        committed: &'intent PreparedCommittedProsody,
        rich: &'intent PreparedRichProsody,
        authority: &'authority CurrentCommittedDependency<'authority, 'session>,
    ) -> Result<Self, CommittedSpeechProsodyRefusal> {
        use CommittedSpeechProsodyRefusal::*;
        let dependency = authority.receipt();
        if committed.dependency().admission() != dependency.admission() {
            return Err(Dependency);
        }
        if committed.dependency().lexical() != dependency.lexical() {
            return Err(Lexical);
        }
        if committed.admission().rich() != rich.requested()
            || committed.intent().choice() != rich.accepted().choice()
        {
            return Err(Rich);
        }
        Ok(Self {
            committed,
            rich,
            authority,
        })
    }
    pub fn committed(&self) -> &PreparedCommittedProsody {
        self.committed
    }
    pub fn authority(&self) -> &CurrentCommittedDependency<'authority, 'session> {
        self.authority
    }
    pub fn rich(&self) -> &PreparedRichProsody {
        self.rich
    }
    /// This finite profile lowers choice only. Never silently erase additional
    /// requested symbolic controls when the installed acoustic seam lacks them.
    pub fn prepare_acoustics(
        &self,
        binding: &SpeechLinguisticProsodyBinding,
    ) -> Result<PreparedLinguisticProsody<'_>, CommittedSpeechProsodyRefusal> {
        let intent = self.committed.intent();
        if *intent.duration() != LanguageProsodyDurationIntent::Neutral
            || *intent.rate() != LanguageProsodyRateIntent::Neutral
            || *intent.focus() != LanguageProsodyFocusIntent::Neutral
        {
            return Err(CommittedSpeechProsodyRefusal::UnsupportedSymbolicControl);
        }
        prepare_linguistic_prosody(LinguisticProsodyBasis::Rich(self.rich), binding)
            .map_err(CommittedSpeechProsodyRefusal::Acoustic)
    }
}

/// Exact existing carrier paired with borrowed current commitment authority.
/// Its lifetime prevents advancing the session before installed admission/Play.
pub struct CommittedSpeechCarrier<'a, 'intent, 'authority, 'session, 'owner, 'inventory> {
    guard: &'a CommittedSpeechProsody<'intent, 'authority, 'session>,
    carrier: &'a crate::complete_carrier::PreparedCompleteSpeechCarrier<'a, 'owner, 'inventory>,
}
impl<'a, 'intent, 'authority, 'session, 'owner, 'inventory>
    CommittedSpeechCarrier<'a, 'intent, 'authority, 'session, 'owner, 'inventory>
{
    pub fn carrier(
        &self,
    ) -> &'a crate::complete_carrier::PreparedCompleteSpeechCarrier<'a, 'owner, 'inventory> {
        self.carrier
    }
    pub fn prosody(&self) -> &'a CommittedSpeechProsody<'intent, 'authority, 'session> {
        self.guard
    }
}
impl<'intent, 'authority, 'session> CommittedSpeechProsody<'intent, 'authority, 'session> {
    pub fn bind_carrier<'a, 'owner, 'inventory>(
        &'a self,
        carrier: &'a crate::complete_carrier::PreparedCompleteSpeechCarrier<'a, 'owner, 'inventory>,
        acoustic: &PreparedLinguisticProsody<'_>,
    ) -> Result<
        CommittedSpeechCarrier<'a, 'intent, 'authority, 'session, 'owner, 'inventory>,
        CommittedSpeechProsodyRefusal,
    > {
        use CommittedSpeechProsodyRefusal::*;
        if carrier.commitment_request() != self.committed.commitment().request()
            || carrier.commitment_result() != self.committed.commitment().result()
        {
            return Err(CarrierCommitment);
        }
        if carrier.lexical().tape() != self.authority.receipt().lexical()
            || carrier.correspondence().text() != self.rich.requested().source().material()
        {
            return Err(CarrierLexical);
        }
        let LinguisticProsodyBasis::Rich(basis) = acoustic.requested() else {
            return Err(CarrierAcoustic);
        };
        if basis.requested() != self.rich.requested() {
            return Err(CarrierAcoustic);
        }
        let checked = self
            .prepare_acoustics(acoustic.accepted().binding())
            .map_err(|_| CarrierAcoustic)?;
        if checked.selected_realization() != acoustic.selected_realization() {
            return Err(CarrierAcoustic);
        }
        let sources = acoustic.boundary().sources();
        let mut segments = 0;
        let mut boundaries = 0;
        for event in carrier.correspondence().intent().events().iter() {
            match event {
                crate::semantic::SpeechUtteranceIntentEvent::Segment(segment)
                    if segment
                        .sources()
                        .iter()
                        .any(|source| sources.contains(source)) =>
                {
                    if segment.sources() != sources
                        || segment.prosody() != acoustic.selected_realization().segment()
                    {
                        return Err(CarrierAcoustic);
                    }
                    segments += 1;
                }
                crate::semantic::SpeechUtteranceIntentEvent::Boundary(value)
                    if value
                        .sources()
                        .iter()
                        .any(|source| sources.contains(source)) =>
                {
                    if value.sources() != sources
                        || value.duration() != acoustic.boundary().duration()
                        || value.kind() != acoustic.boundary().kind()
                    {
                        return Err(CarrierAcoustic);
                    }
                    boundaries += 1;
                }
                _ => {}
            }
        }
        if segments == 0 || boundaries != 1 {
            return Err(CarrierAcoustic);
        }
        Ok(CommittedSpeechCarrier {
            guard: self,
            carrier,
        })
    }
}
