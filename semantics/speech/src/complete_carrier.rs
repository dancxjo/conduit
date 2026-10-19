//! One custody receipt composing existing admitted linguistic/acoustic owners.
//! No new linguistic IR, inference, normalization, playback grant or model is
//! introduced. Absence in an optional acoustic slot means explicitly not supplied.
use crate::{
    correspondence::PreparedSpeechCorrespondence,
    intent_frequency_trajectory::{
        AdmittedFrequencyObservation, FrequencyGestureRefusal, PreparedFrequencyGestures,
    },
    linguistic_rate::PreparedLinguisticRate,
    scoped_energy::{AdmittedScopedEnergyObservation, PreparedScopedEnergy},
    semantic::*,
    syllable_structure::PreparedSyllableStructure,
};
use alloc::vec::Vec;
use conduit_language::{
    lexical::PreparedLexicalTape,
    parser_commitment::{ParserCommitmentRefusal, PreparedParserCommitment},
    LanguageLexicalCompleteness, LanguageParserBeamCommitRequest, LanguageParserBeamCommitResult,
    LanguageTextSegmentKind,
};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
mod words;
#[derive(Debug)]
pub enum CompleteCarrierRefusal {
    Native(NativeBindingRefusal),
    ForeignStructure,
    ForeignLexical,
    Commitment(ParserCommitmentRefusal),
    CommitmentBasis,
    IncompleteCommitment,
    Capacity,
    WordRelation,
    UnsupportedWordRelation,
    ForeignAcoustic,
    Observation(FrequencyGestureRefusal),
    MissingObservationTarget,
}
impl From<NativeBindingRefusal> for CompleteCarrierRefusal {
    fn from(e: NativeBindingRefusal) -> Self {
        Self::Native(e)
    }
}
pub struct CompleteCarrierAcoustics<'a, 'owner, 'inventory> {
    pub frequency: Option<&'a PreparedFrequencyGestures<'a, 'owner, 'inventory>>,
    pub energy: Option<&'a PreparedScopedEnergy<'a, 'owner, 'inventory>>,
    pub rate: Option<&'a PreparedLinguisticRate<'a, 'owner, 'inventory>>,
    pub frequency_observations: &'a [(usize, SpeechFrequencyObservation)],
    pub energy_observations: &'a [(usize, SpeechScopedEnergyObservation)],
}
impl CompleteCarrierAcoustics<'_, '_, '_> {
    pub fn not_supplied() -> Self {
        Self {
            frequency: None,
            energy: None,
            rate: None,
            frequency_observations: &[],
            energy_observations: &[],
        }
    }
}
pub struct PreparedCompleteSpeechCarrier<'a, 'owner, 'inventory> {
    correspondence: &'a PreparedSpeechCorrespondence<'owner, 'inventory>,
    structure: &'a PreparedSyllableStructure<'owner>,
    lexical: &'a PreparedLexicalTape,
    word_relations: &'a [TranslationCorrespondence],
    commitment: PreparedParserCommitment,
    acoustics: CompleteCarrierAcoustics<'a, 'owner, 'inventory>,
    frequency_observations: Vec<AdmittedFrequencyObservation<'a, 'a, 'owner, 'inventory>>,
    energy_observations: Vec<AdmittedScopedEnergyObservation<'a, 'a, 'owner, 'inventory>>,
    words: words::WordAdmissions<'a>,
    canonical: Vec<u8>,
    identity: [u8; 32],
}
impl<'a, 'owner, 'inventory> PreparedCompleteSpeechCarrier<'a, 'owner, 'inventory> {
    pub fn prepare(
        correspondence: &'a PreparedSpeechCorrespondence<'owner, 'inventory>,
        structure: &'a PreparedSyllableStructure<'owner>,
        lexical: &'a PreparedLexicalTape,
        word_relations: &'a [TranslationCorrespondence],
        commitment_request: &LanguageParserBeamCommitRequest,
        commitment_result: &LanguageParserBeamCommitResult,
        acoustics: CompleteCarrierAcoustics<'a, 'owner, 'inventory>,
    ) -> Result<Self, CompleteCarrierRefusal> {
        use CompleteCarrierRefusal::*;
        if !core::ptr::eq(correspondence.intent(), structure.intent())
            || !core::ptr::eq(correspondence.phones(), structure.phones())
            || !core::ptr::eq(correspondence.text(), structure.text())
        {
            return Err(ForeignStructure);
        }
        if lexical.tape().source().material() != correspondence.text() {
            return Err(ForeignLexical);
        }
        let basis = commitment_request.beam().basis();
        if basis.text() != correspondence.text().identity()
            || basis.source_revision() != correspondence.text().revision()
            || commitment_request.frontier().basis() != basis
        {
            return Err(CommitmentBasis);
        }
        let count = lexical.tape().tokens().len();
        if usize::try_from(*commitment_request.through()).ok() != Some(count) {
            return Err(IncompleteCommitment);
        }
        for hypothesis in [
            commitment_request.beam().candidate0(),
            commitment_request.beam().candidate1(),
            commitment_request.beam().candidate2(),
            commitment_request.beam().candidate3(),
        ] {
            if *hypothesis.active()
                && usize::try_from(*hypothesis.state().token_count()).ok() != Some(count)
            {
                return Err(CommitmentBasis);
            }
        }
        for token in lexical.tape().tokens().as_slice() {
            if !matches!(token.completeness(), LanguageLexicalCompleteness::Complete)
                || *token.span().end()
                    > u64::from(lexical.tape().source().stable_prefix().unwrap_or(0))
            {
                return Err(IncompleteCommitment);
            }
        }
        let commitment = PreparedParserCommitment::prepare(commitment_request, commitment_result)
            .map_err(Commitment)?;
        if commitment.result().frontier().committed() != commitment_request.through() {
            return Err(IncompleteCommitment);
        }
        let words = words::prepare(correspondence, lexical, structure, word_relations)?;
        if acoustics.frequency_observations.len() > 16 || acoustics.energy_observations.len() > 16 {
            return Err(Capacity);
        }
        if let Some(f) = acoustics.frequency {
            if !core::ptr::eq(f.original(), correspondence)
                || !core::ptr::eq(f.structure(), structure)
            {
                return Err(ForeignAcoustic);
            }
        }
        if let Some(e) = acoustics.energy {
            if !core::ptr::eq(e.original(), correspondence)
                || !core::ptr::eq(e.structure(), structure)
            {
                return Err(ForeignAcoustic);
            }
        }
        if let Some(r) = acoustics.rate {
            if !core::ptr::eq(r.original(), correspondence)
                || !core::ptr::eq(r.structure(), structure)
            {
                return Err(ForeignAcoustic);
            }
        }
        let mut frequency_observations = Vec::new();
        for (index, observation) in acoustics.frequency_observations {
            frequency_observations.push(
                acoustics
                    .frequency
                    .ok_or(MissingObservationTarget)?
                    .observe(*index, observation)
                    .map_err(Observation)?,
            );
        }
        let mut energy_observations = Vec::new();
        for (index, observation) in acoustics.energy_observations {
            energy_observations.push(
                acoustics
                    .energy
                    .ok_or(MissingObservationTarget)?
                    .observe(*index, observation)
                    .map_err(Observation)?,
            );
        }
        let mut value = Self {
            correspondence,
            structure,
            lexical,
            word_relations,
            commitment,
            acoustics,
            frequency_observations,
            energy_observations,
            words,
            canonical: Vec::new(),
            identity: [0; 32],
        };
        value.canonical = value.encode_original()?;
        value.identity =
            conduit_core::semantic_digest("conduit.speech/complete-carrier@1", &value.canonical);
        Ok(value)
    }
    pub fn correspondence(&self) -> &'a PreparedSpeechCorrespondence<'owner, 'inventory> {
        self.correspondence
    }
    pub fn structure(&self) -> &'a PreparedSyllableStructure<'owner> {
        self.structure
    }
    pub fn lexical(&self) -> &'a PreparedLexicalTape {
        self.lexical
    }
    pub fn word_relations(&self) -> &'a [TranslationCorrespondence] {
        self.word_relations
    }
    pub fn commitment_request(&self) -> &LanguageParserBeamCommitRequest {
        self.commitment.request()
    }
    pub fn commitment_result(&self) -> &LanguageParserBeamCommitResult {
        self.commitment.result()
    }
    pub fn commitment(&self) -> &PreparedParserCommitment {
        &self.commitment
    }
    pub fn acoustics(&self) -> &CompleteCarrierAcoustics<'a, 'owner, 'inventory> {
        &self.acoustics
    }
    pub fn frequency_observations(
        &self,
    ) -> &[AdmittedFrequencyObservation<'a, 'a, 'owner, 'inventory>] {
        &self.frequency_observations
    }
    pub fn energy_observations(
        &self,
    ) -> &[AdmittedScopedEnergyObservation<'a, 'a, 'owner, 'inventory>] {
        &self.energy_observations
    }
    pub fn word_text_admissions(&self) -> &[crate::text_admission::ResolvedText<'a>] {
        &self.words.text
    }
    /// Exact punctuation occurrences explicitly omitted from phonemic realization.
    /// They remain in the lexical tape and its fully covered parser commitment.
    pub fn omitted_text_admissions(&self) -> &[crate::text_admission::ResolvedText<'a>] {
        &self.words.omitted
    }
    pub fn word_phoneme_admissions(&self) -> &[crate::reference_admission::ResolvedPhoneme<'a>] {
        &self.words.phonemes
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn identity(&self) -> [u8; 32] {
        self.identity
    }
    fn encode_original(&self) -> Result<Vec<u8>, CompleteCarrierRefusal> {
        let mut out = Vec::new();
        packet(&mut out, b"complete-carrier@1")?;
        packet(
            &mut out,
            &crate::carrier_identity::canonical_carrier_bytes(self.correspondence)?,
        )?;
        native(&mut out, self.lexical.tape().clone())?;
        many(&mut out, self.structure.sources())?;
        many(&mut out, self.structure.boundaries())?;
        many(&mut out, self.word_relations)?;
        native(&mut out, self.commitment.request().clone())?;
        native(&mut out, self.commitment.result().clone())?;
        packet(&mut out, self.commitment.source_id().as_bytes())?;
        for execution in self.commitment.executions() {
            // Programs retain their full bytes on the opaque owner receipt.
            // Bind their exact identities here without duplicating schema-rich
            // expression programs in each common-carrier evidence packet.
            packet(
                &mut out,
                &conduit_core::semantic_digest(
                    "conduit.language/commitment-program@1",
                    execution.program_hex().as_bytes(),
                ),
            )?;
            packet(&mut out, execution.input_canonical())?;
            packet(&mut out, execution.output_canonical())?;
        }
        match self.acoustics.frequency {
            Some(value) => {
                packet(&mut out, b"frequency/supplied")?;
                native(&mut out, value.targets().clone())?;
                packet(&mut out, value.audio().original_canonical())?;
                audio_executions(&mut out, value.audio().admission_executions())?;
            }
            None => packet(&mut out, b"frequency/not-supplied")?,
        }
        match self.acoustics.energy {
            Some(value) => {
                packet(&mut out, b"energy/supplied")?;
                native(&mut out, value.supplied().clone())?;
                packet(&mut out, value.audio().original_canonical())?;
                audio_executions(&mut out, value.audio().admission_executions())?;
            }
            None => packet(&mut out, b"energy/not-supplied")?,
        }
        match self.acoustics.rate {
            Some(value) => {
                packet(&mut out, b"rate/supplied")?;
                native(&mut out, value.admission().clone())?;
                native(&mut out, value.result().clone())?;
                packet(&mut out, value.source_program_hex().as_bytes())?;
                packet(&mut out, value.input_canonical())?;
                packet(&mut out, value.output_canonical())?;
            }
            None => packet(&mut out, b"rate/not-supplied")?,
        }
        packet(
            &mut out,
            &(self.acoustics.frequency_observations.len() as u64).to_le_bytes(),
        )?;
        for (index, observation) in self.acoustics.frequency_observations {
            packet(&mut out, &(*index as u64).to_le_bytes())?;
            native(&mut out, observation.clone())?;
        }
        packet(
            &mut out,
            &(self.acoustics.energy_observations.len() as u64).to_le_bytes(),
        )?;
        for (index, observation) in self.acoustics.energy_observations {
            packet(&mut out, &(*index as u64).to_le_bytes())?;
            native(&mut out, observation.clone())?;
        }
        Ok(out)
    }
}
fn native<T: NativeRustBinding>(out: &mut Vec<u8>, value: T) -> Result<(), CompleteCarrierRefusal> {
    packet(out, &value.encode()?)
}
fn many<T: NativeRustBinding + Clone>(
    out: &mut Vec<u8>,
    values: &[T],
) -> Result<(), CompleteCarrierRefusal> {
    packet(out, &(values.len() as u64).to_le_bytes())?;
    for value in values {
        native(out, value.clone())?;
    }
    Ok(())
}
fn packet(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), CompleteCarrierRefusal> {
    if bytes.len() > 2_000_000
        || out
            .len()
            .checked_add(bytes.len())
            .and_then(|v| v.checked_add(8))
            .is_none_or(|v| v > 2_000_000)
    {
        return Err(CompleteCarrierRefusal::Capacity);
    }
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

fn audio_executions(
    out: &mut Vec<u8>,
    executions: &[conduit_audio::AudioSourceExecution],
) -> Result<(), CompleteCarrierRefusal> {
    packet(out, &(executions.len() as u64).to_le_bytes())?;
    for execution in executions {
        packet(out, execution.source_program_hex().as_bytes())?;
        packet(out, execution.input_canonical())?;
        packet(out, execution.output_canonical())?;
    }
    Ok(())
}
