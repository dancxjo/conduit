//! Exact declared syllable density over one complete origin-zero Audio span.
//! Source performs all numeric projection. No clock mapping, gap filling,
//! schedule interpolation, acoustic-frequency alias, or measurement is inferred.
use crate::{
    correspondence::PreparedSpeechCorrespondence, semantic::*,
    syllable_structure::PreparedSyllableStructure,
};
use alloc::vec::Vec;
use conduit_audio::AudioTrajectoryAnchor;
use conduit_plot::{
    rust_binding::{NativeBindingRefusal, NativeRustBinding},
    PortableExpressionEvaluationRefusal, PortableExpressionProgram,
};

const PROGRAM: &str = include_str!(concat!(env!("OUT_DIR"), "/linguistic_rate_program.hex"));

#[derive(Debug)]
pub enum LinguisticRateRefusal {
    Native(NativeBindingRefusal),
    Program,
    Evaluation(PortableExpressionEvaluationRefusal),
    Basis,
    ForeignStructure,
    InventoryBasis,
    Source,
    ForeignAnchor,
    Capacity,
}

/// Target-only execution. The opaque original structure remains available with
/// syllables, phones, boundaries, exact text and original acoustic intent.
pub struct PreparedLinguisticRate<'a, 'carrier, 'inventory> {
    original: &'a PreparedSpeechCorrespondence<'carrier, 'inventory>,
    structure: &'a PreparedSyllableStructure<'carrier>,
    admission: SpeechLinguisticRateAdmission,
    result: SpeechSyllablesPerSecond,
    input: Vec<u8>,
    output: Vec<u8>,
}
impl<'a, 'carrier, 'inventory> PreparedLinguisticRate<'a, 'carrier, 'inventory> {
    pub fn original(&self) -> &'a PreparedSpeechCorrespondence<'carrier, 'inventory> {
        self.original
    }
    pub fn structure(&self) -> &'a PreparedSyllableStructure<'carrier> {
        self.structure
    }
    pub fn admission(&self) -> &SpeechLinguisticRateAdmission {
        &self.admission
    }
    pub fn result(&self) -> &SpeechSyllablesPerSecond {
        &self.result
    }
    pub fn source_program_hex(&self) -> &'static str {
        PROGRAM
    }
    pub fn input_canonical(&self) -> &[u8] {
        &self.input
    }
    pub fn output_canonical(&self) -> &[u8] {
        &self.output
    }
}

/// Prepares the explicitly named constant full-utterance subset. A caller must
/// provide the complete covered duration; this function does not assert that a
/// physical observation or a separate acoustic trajectory covers that duration.
/// Multiple spans/overlap/gaps require another admitted profile and cannot be
/// passed to this one-span entrance. Original fractions are never reduced.
pub fn prepare_linguistic_rate<'a, 'carrier, 'inventory>(
    original: &'a PreparedSpeechCorrespondence<'carrier, 'inventory>,
    structure: &'a PreparedSyllableStructure<'carrier>,
    anchor: &AudioTrajectoryAnchor,
    target: &SpeechLinguisticRateTarget,
) -> Result<PreparedLinguisticRate<'a, 'carrier, 'inventory>, LinguisticRateRefusal> {
    use LinguisticRateRefusal::*;
    if !core::ptr::eq(original.intent(), structure.intent())
        || !core::ptr::eq(original.phones(), structure.phones())
        || !core::ptr::eq(original.text(), structure.text())
    {
        return Err(ForeignStructure);
    }
    if target.scope().inventory_basis() != original.inventory().basis() {
        return Err(InventoryBasis);
    }
    if target.scope().source() != original.alignment().source() {
        return Err(Source);
    }
    let intent = original.intent();
    let basis = target.scope().basis();
    if basis != original.phones().basis()
        || basis.utterance_id() != intent.utterance_id()
        || basis.revision_id() != intent.revision_id()
        || basis.inventory_id() != intent.inventory_id()
        || basis.language() != intent.language()
    {
        return Err(Basis);
    }
    if target.scope().anchor() != anchor {
        return Err(ForeignAnchor);
    }
    let scalars = u32::try_from(original.text().text().chars().count()).map_err(|_| Capacity)?;
    let source = target.scope().source();
    LanguageTextReferenceMatch::new(original.text().clone(), source.clone(), scalars)
        .map_err(Native)?;
    if *source.range().start() != 0 || *source.range().end() != scalars {
        return Err(Source);
    }
    let count = u64::try_from(structure.syllables().len()).map_err(|_| Capacity)?;
    let admission = SpeechLinguisticRateAdmission::new(count, target.clone()).map_err(Native)?;
    let input = admission.clone().encode().map_err(Native)?;
    let program = PortableExpressionProgram::from_canonical_hex(PROGRAM).map_err(|_| Program)?;
    let output = program.evaluate(&input).map_err(Evaluation)?;
    let raw = SpeechLinguisticRateRawProjection::decode(&output).map_err(Native)?;
    // Source owns the arithmetic; the generated public constructor owns exact
    // output refinements. A raw expression cannot bypass that admission.
    let result =
        SpeechSyllablesPerSecond::new(*raw.denominator_seconds(), *raw.numerator_syllables())
            .map_err(Native)?;
    Ok(PreparedLinguisticRate {
        original,
        structure,
        admission,
        result,
        input,
        output,
    })
}
