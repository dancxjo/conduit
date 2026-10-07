//! Exact accepted alignment and independently admitted target discourse.
//! Supplied discourse facts and an accepted alignment snapshot are checked together.
//! The receipt does not establish current provider acceptance or infer target syntax.
//! This receipt derives no translation and grants no playback authority.
use crate::semantic::*;
use conduit_language::{discourse::PreparedVocativeFact, lexical::PreparedLexicalTape, *};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

pub struct TranslationVocativeRequest<'a> {
    pub candidate: &'a TranslationCandidate,
    pub accepted: &'a TranslationAccepted,
    pub source: &'a PreparedVocativeFact,
    pub target: &'a PreparedVocativeFact,
    pub source_lexical: &'a PreparedLexicalTape,
    pub target_lexical: &'a PreparedLexicalTape,
    pub alignment_index: usize,
    pub source_slot: u64,
    pub target_slot: u64,
}
#[derive(Debug)]
pub enum TranslationStructureRefusal {
    MissingAlignment,
    NonTextAlignment,
    MissingOccurrence,
    LexicalBasis,
    Native(NativeBindingRefusal),
    Source(LinguisticRefusal),
    Program {
        stage: &'static str,
        detail: alloc::string::String,
    },
    Unaligned,
}
pub struct PreparedTranslationVocative<'a> {
    candidate: &'a TranslationCandidate,
    witness: TranslationVocativeAdmission,
    source_fact: &'a PreparedVocativeFact,
    target_fact: &'a PreparedVocativeFact,
    alignment_index: usize,
}
impl PreparedTranslationVocative<'_> {
    pub fn candidate(&self) -> &TranslationCandidate {
        self.candidate
    }
    pub fn source_fact(&self) -> &PreparedVocativeFact {
        self.source_fact
    }
    pub fn target_fact(&self) -> &PreparedVocativeFact {
        self.target_fact
    }
    pub fn alignment(&self) -> &TranslationAlignment {
        &self.candidate.alignments().as_slice()[self.alignment_index]
    }
    pub fn witness(&self) -> &TranslationVocativeAdmission {
        &self.witness
    }
}
pub fn prepare_translation_vocative(
    request: TranslationVocativeRequest<'_>,
) -> Result<PreparedTranslationVocative<'_>, TranslationStructureRefusal> {
    use TranslationStructureRefusal::*;
    let source = request.source.fact();
    let target = request.target.fact();
    if request.source_lexical.tape().source() != source.source()
        || request.target_lexical.tape().source() != target.source()
    {
        return Err(LexicalBasis);
    }
    let occurrence = |tape: &PreparedLexicalTape, identity: &LinguisticTokenIdentity| {
        tape.tape()
            .tokens()
            .as_slice()
            .iter()
            .find(|token| token.identity() == identity)
            .cloned()
    };
    let source_token = occurrence(request.source_lexical, source.basis().dependent().token())
        .ok_or(MissingOccurrence)?;
    let target_token = occurrence(request.target_lexical, target.basis().dependent().token())
        .ok_or(MissingOccurrence)?;
    let source_reference = language_source_occurrence(
        source.source().material(),
        source_token.span(),
        LanguageTextSegmentKind::Word,
    )
    .map_err(Source)?;
    let target_reference = language_source_occurrence(
        target.source().material(),
        target_token.span(),
        LanguageTextSegmentKind::Word,
    )
    .map_err(Source)?;
    let alignment = request
        .candidate
        .alignments()
        .as_slice()
        .get(request.alignment_index)
        .ok_or(MissingAlignment)?;
    let TranslationCorrespondence::Aligned(group) = alignment.correspondence() else {
        return Err(NonTextAlignment);
    };
    let selected = |values: &[LanguageSegmentRef], slot: u64| {
        let slot = usize::try_from(slot).map_err(|_| MissingAlignment)?;
        let Some(LanguageSegmentRef::Text(reference)) = values.get(slot) else {
            return Err(NonTextAlignment);
        };
        LanguageTextSegmentRef::new(
            *reference.kind(),
            reference.language().clone(),
            reference.range().clone(),
            reference.revision_id().clone(),
            reference.text_id().clone(),
        )
        .map_err(Native)
    };
    let aligned_source = selected(group.sources().as_slice(), request.source_slot)?;
    let aligned_target = selected(group.targets().as_slice(), request.target_slot)?;
    let anchor_program = conduit_plot::PortableExpressionProgram::from_canonical_hex(include_str!(
        concat!(env!("OUT_DIR"), "/translation_anchor_program.hex")
    ))
    .map_err(|error| Program {
        stage: "decode portable program",
        detail: alloc::format!("{error:?}"),
    })?;
    let mut anchor_evaluator = conduit_plot::PreparedPortableExpressionEvaluator::new(
        &anchor_program,
    )
    .map_err(|error| Program {
        stage: "prepare discourse anchor",
        detail: alloc::format!("{error:?}"),
    })?;
    let mut anchor = |fact: &LanguageVocativeDiscourseFact| {
        let input = fact.clone().encode().map_err(Native)?;
        let bytes = anchor_evaluator.evaluate(&input).map_err(|error| Program {
            stage: "evaluate discourse anchor",
            detail: alloc::format!("{error:?}"),
        })?;
        TranslationDiscourseAnchor::decode(bytes).map_err(Native)
    };
    let witness = TranslationVocativeAdmission::new(
        request.accepted.clone(),
        aligned_source,
        aligned_target,
        request.candidate.identity().clone(),
        anchor(source)?,
        request.candidate.source().clone(),
        source_reference,
        source_token.span().clone(),
        anchor(target)?,
        request.candidate.target().clone(),
        target_reference,
        target_token.span().clone(),
    )
    .map_err(Native)?;
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(include_str!(
        concat!(env!("OUT_DIR"), "/translation_vocative_program.hex")
    ))
    .map_err(|error| Program {
        stage: "decode alignment program",
        detail: alloc::format!("{error:?}"),
    })?;
    let bytes = program
        .evaluate(&witness.clone().encode().map_err(Native)?)
        .map_err(|error| Program {
            stage: "evaluate alignment",
            detail: alloc::format!("{error:?}"),
        })?;
    // Boolean uses the exact native primitive encoding.
    if bytes.as_slice() != [1] {
        return Err(Unaligned);
    }
    Ok(PreparedTranslationVocative {
        candidate: request.candidate,
        witness,
        source_fact: request.source,
        target_fact: request.target,
        alignment_index: request.alignment_index,
    })
}
