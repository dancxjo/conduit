//! Exact symbolic-language intent to explicit rate-independent speech profile.
//! These are preparation receipts, not playback or grammar inference.
use crate::semantic::*;
use conduit_language::{
    prosody::*, LanguageLexicalToken, LanguageProsodyChoice, LanguageProsodyProfile,
    LanguageTextRevision,
};
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal, NativeRustBinding};
#[derive(Debug)]
pub enum LinguisticProsodyRefusal {
    Native(NativeBindingRefusal),
    Admission {
        requested: LanguageProsodyChoice,
        source: alloc::boxed::Box<conduit_language::LanguageTextSegmentRef>,
        profile_id: alloc::string::String,
        reason: NativeBindingRefusal,
    },
    Program,
    Source(conduit_language::LinguisticRefusal),
}
pub enum LinguisticProsodyBasis<'a> {
    Rich(&'a PreparedRichProsody),
    Fallback(&'a PreparedFallbackProsody),
}
impl LinguisticProsodyBasis<'_> {
    pub fn source(&self) -> &LanguageTextRevision {
        match self {
            Self::Rich(value) => value.requested().source(),
            Self::Fallback(value) => value.requested().source(),
        }
    }
    pub fn token(&self) -> &LanguageLexicalToken {
        match self {
            Self::Rich(value) => value.requested().token(),
            Self::Fallback(value) => value.requested().token(),
        }
    }
    pub fn profile(&self) -> &LanguageProsodyProfile {
        match self {
            Self::Rich(value) => value.requested().profile(),
            Self::Fallback(value) => value.requested().profile(),
        }
    }
    pub fn choice(&self) -> &LanguageProsodyChoice {
        match self {
            Self::Rich(value) => value.accepted().choice(),
            Self::Fallback(value) => value.accepted().choice(),
        }
    }
}
pub struct PreparedLinguisticProsody<'a> {
    basis: LinguisticProsodyBasis<'a>,
    accepted: SpeechLinguisticProsodyAdmission,
    realization: SpeechLinguisticProsodyRealization,
    segments: [SpeechSegmentProsodyIntent; 1],
    boundary: SpeechPlannedBoundaryIntent,
}
impl PreparedLinguisticProsody<'_> {
    pub fn requested(&self) -> &LinguisticProsodyBasis<'_> {
        &self.basis
    }
    pub fn accepted(&self) -> &SpeechLinguisticProsodyAdmission {
        &self.accepted
    }
    pub fn selected_realization(&self) -> &SpeechLinguisticProsodyRealization {
        &self.realization
    }
    pub fn boundary(&self) -> &SpeechPlannedBoundaryIntent {
        &self.boundary
    }
    pub fn prepare_formant_segment(
        &self,
    ) -> Result<
        crate::intent_prosody::PreparedSegmentProsody<'_>,
        crate::intent_prosody::IntentProsodyRefusal,
    > {
        crate::intent_prosody::prepare_segment_prosody(&self.segments)
    }
}
pub fn prepare_linguistic_prosody<'a>(
    basis: LinguisticProsodyBasis<'a>,
    binding: &SpeechLinguisticProsodyBinding,
) -> Result<PreparedLinguisticProsody<'a>, LinguisticProsodyRefusal> {
    let source_reference = conduit_language::language_source_occurrence(
        basis.source().material(),
        basis.token().span(),
        LanguageTextSegmentKind::Word,
    )
    .map_err(LinguisticProsodyRefusal::Source)?;
    let accepted = SpeechLinguisticProsodyAdmission::new(
        binding.clone(),
        basis.source().material().language().clone(),
        basis.profile().identity().clone(),
        basis.choice().clone(),
    )
    .map_err(|reason| LinguisticProsodyRefusal::Admission {
        requested: basis.choice().clone(),
        source: alloc::boxed::Box::new(source_reference.clone()),
        profile_id: basis.profile().identity().clone(),
        reason,
    })?;
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(include_str!(
        concat!(env!("OUT_DIR"), "/linguistic_prosody_program.hex")
    ))
    .map_err(|_| LinguisticProsodyRefusal::Program)?;
    let bytes = program
        .evaluate(
            &accepted
                .clone()
                .encode()
                .map_err(LinguisticProsodyRefusal::Native)?,
        )
        .map_err(|_| LinguisticProsodyRefusal::Program)?;
    let realization = SpeechLinguisticProsodyRealization::decode(&bytes)
        .map_err(LinguisticProsodyRefusal::Native)?;
    let reference = source_reference;
    let reference = LanguageSegmentRef::text(
        *reference.kind(),
        reference.language().clone(),
        reference.range().clone(),
        reference.revision_id().clone(),
        reference.text_id().clone(),
    )
    .map_err(LinguisticProsodyRefusal::Native)?;
    let sources =
        BoundedSequence::try_from_iter([reference]).expect("one finite source occurrence");
    let boundary = SpeechPlannedBoundaryIntent::new(
        realization.boundary_duration().clone(),
        realization.boundary_kind().clone(),
        binding.provenance().clone(),
        sources,
    )
    .map_err(LinguisticProsodyRefusal::Native)?;
    let segments = [realization.segment().clone()];
    Ok(PreparedLinguisticProsody {
        basis,
        accepted,
        realization,
        segments,
        boundary,
    })
}
