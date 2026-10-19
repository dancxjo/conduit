//! Finite supplied-selection pronunciation preparation, NOT commitment authority.
//! The rich consumer must later require the Language-owned committed dependency
//! receipt. This intermediate seam retains the whole existing selection receipt.
use crate::{
    ipa_inventory::PreparedIpaInventory,
    ipa_phonemic::AdmittedPhonemicTranscription,
    ipa_phonetic::{phonetic_from_ipa, AdmittedPhoneticTranscription},
    semantic::*,
};
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_language::{
    pronunciation_selection::PreparedPronunciationSelection, LanguageLexicalCandidate,
    LanguageLexicalToken, LanguageVariety,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    rust_binding::NativeRustBinding, CheckedSyntaxDocument, PortableExpressionProgram,
    ProfileCatalog, StartupCatalog,
};
pub const RECORD_PROFILE_SOURCE: &str = include_str!("../record_pronunciation.conduit");
pub const RECORD_VARIETY_ID: &str = "speech/uk-nonrhotic-record@1";
#[derive(Debug)]
pub enum RecordPronunciationRefusal {
    Source(String),
    Native,
    UnsupportedProfile,
    Inventory,
    Phonemic,
    Phonetic,
    Output,
}
/// Immutable checked expression reused across preparations; no per-call checking.
pub struct PreparedRecordPronunciationProfile {
    checked: CheckedSyntaxDocument,
    program: PortableExpressionProgram,
    program_hex: String,
    source_id: String,
}
/// This type deliberately promises only supplied-selection compatibility.
/// Original source, token, exact arc and lexical profile remain inspectable.
pub struct SuppliedRecordPronunciation<'a, 'inventory> {
    selection: &'a PreparedPronunciationSelection,
    inventory: &'a PreparedIpaInventory<'inventory>,
    phonemic: AdmittedPhonemicTranscription<'inventory>,
    phonetic: AdmittedPhoneticTranscription,
    input: Vec<u8>,
    output: Vec<u8>,
    dictionary: String,
    first_vowel: String,
    stressed_syllable: u64,
    short: SpeechExactDuration,
    long: SpeechExactDuration,
}
impl SuppliedRecordPronunciation<'_, '_> {
    pub fn selection(&self) -> &PreparedPronunciationSelection {
        self.selection
    }
    pub fn inventory(&self) -> &PreparedIpaInventory<'_> {
        self.inventory
    }
    pub fn phonemic(&self) -> &AdmittedPhonemicTranscription<'_> {
        &self.phonemic
    }
    pub fn phonetic(&self) -> &AdmittedPhoneticTranscription {
        &self.phonetic
    }
    pub fn dictionary_ipa(&self) -> &str {
        &self.dictionary
    }
    pub fn first_vowel_ipa(&self) -> &str {
        &self.first_vowel
    }
    pub fn stressed_syllable(&self) -> u64 {
        self.stressed_syllable
    }
    pub fn short_duration(&self) -> &SpeechExactDuration {
        &self.short
    }
    pub fn long_duration(&self) -> &SpeechExactDuration {
        &self.long
    }
    pub fn input_canonical(&self) -> &[u8] {
        &self.input
    }
    pub fn output_canonical(&self) -> &[u8] {
        &self.output
    }
}
impl PreparedRecordPronunciationProfile {
    pub fn new() -> Result<Self, RecordPronunciationRefusal> {
        use RecordPronunciationRefusal::*;
        let mut catalog = StartupCatalog::new();
        catalog
            .insert_structured_type(
                "LanguageLexicalCandidate",
                LanguageLexicalCandidate::semantic_type().map_err(|_| Native)?,
            )
            .map_err(Source)?;
        catalog
            .insert_structured_type(
                "LanguageLexicalToken",
                LanguageLexicalToken::semantic_type().map_err(|_| Native)?,
            )
            .map_err(Source)?;
        catalog
            .insert_structured_type(
                "LanguageVariety",
                LanguageVariety::semantic_type().map_err(|_| Native)?,
            )
            .map_err(Source)?;
        let checked =
            check_syntax_document(&parse_syntax_document(RECORD_PROFILE_SOURCE), &catalog)
                .map_err(|e| Source(format!("{e:?}")))?;
        let expanded = expand_canonical_plot_for_authoring(
            &checked,
            "speech/record-pronunciation",
            &ProfileCatalog::new(),
        )
        .map_err(|e| Source(format!("{e:?}")))?;
        if expanded.expanded.gears.len() != 1 {
            return Err(Output);
        }
        let [entry] = expanded.expanded.gears[0].configuration.as_slice() else {
            return Err(Output);
        };
        let ConfigurationValue::Text(hex) = &entry.value else {
            return Err(Output);
        };
        Ok(Self {
            program: PortableExpressionProgram::from_canonical_hex(hex).map_err(|_| Output)?,
            program_hex: hex.clone(),
            source_id: checked.source_document_id.as_str().into(),
            checked,
        })
    }
    pub fn checked_source(&self) -> &CheckedSyntaxDocument {
        &self.checked
    }
    pub fn program_hex(&self) -> &str {
        &self.program_hex
    }
    pub fn source_id(&self) -> &str {
        &self.source_id
    }
    pub fn project_supplied_selection<'a, 'inventory>(
        &self,
        selection: &'a PreparedPronunciationSelection,
        inventory: &'a PreparedIpaInventory<'inventory>,
    ) -> Result<SuppliedRecordPronunciation<'a, 'inventory>, RecordPronunciationRefusal> {
        use RecordPronunciationRefusal::*;
        if inventory.basis().language() != selection.request().source().material().language() {
            return Err(Inventory);
        }
        let fields = vec![
            StructuredFieldValue::new(
                "candidate",
                selection
                    .candidate()
                    .clone()
                    .into_structured()
                    .map_err(|_| Native)?,
            )
            .map_err(|_| Native)?,
            StructuredFieldValue::new(
                "token",
                selection
                    .request()
                    .token()
                    .clone()
                    .into_structured()
                    .map_err(|_| Native)?,
            )
            .map_err(|_| Native)?,
            StructuredFieldValue::new(
                "variety",
                inventory
                    .basis()
                    .variety()
                    .clone()
                    .into_structured()
                    .map_err(|_| Native)?,
            )
            .map_err(|_| Native)?,
        ];
        let input = StructuredInfoValue::record(self.program.input_type.clone(), fields)
            .map_err(|_| Native)?
            .canonical_bytes()
            .map_err(|_| Native)?;
        let output = self
            .program
            .evaluate(&input)
            .map_err(|e| Source(format!("{e:?}")))?;
        let value = StructuredInfoValue::from_canonical_bytes(&output).map_err(|_| Output)?;
        if leaf(field(&value, "accepted")?)? != [1] {
            return Err(UnsupportedProfile);
        }
        let dictionary = text(field(&value, "dictionary_ipa")?)?;
        let provenance = SpeechEvidenceProvenance::new(
            "Cambridge UK record; broad-to-phone realization@1".into(),
            SpeechEvidenceSource::Manual,
            Some("1".into()),
        )
        .map_err(|_| Native)?;
        let phonemic = inventory
            .phonemic_from_ipa(dictionary.clone(), provenance.clone())
            .map_err(|_| Phonemic)?;
        let phonetic = phonetic_from_ipa(text(field(&value, "phonetic_ipa")?)?, provenance)
            .map_err(|_| Phonetic)?;
        let short = SpeechExactDuration::new(
            u64_field(&value, "short_denominator")?,
            u64_field(&value, "short_numerator_seconds")?,
        )
        .map_err(|_| Native)?;
        let long = SpeechExactDuration::new(
            u64_field(&value, "long_denominator")?,
            u64_field(&value, "long_numerator_seconds")?,
        )
        .map_err(|_| Native)?;
        Ok(SuppliedRecordPronunciation {
            selection,
            inventory,
            phonemic,
            phonetic,
            input,
            output: output.clone(),
            dictionary,
            first_vowel: text(field(&value, "first_vowel_ipa")?)?,
            stressed_syllable: u64_field(&value, "stressed_syllable")?,
            short,
            long,
        })
    }
}
fn field<'a>(
    v: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, RecordPronunciationRefusal> {
    let StructuredInfoValueShape::Record(fields) = v.shape() else {
        return Err(RecordPronunciationRefusal::Output);
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .map(|f| f.value())
        .ok_or(RecordPronunciationRefusal::Output)
}
fn leaf(v: &StructuredInfoValue) -> Result<&[u8], RecordPronunciationRefusal> {
    match v.shape() {
        StructuredInfoValueShape::Leaf(v) => Ok(v),
        _ => Err(RecordPronunciationRefusal::Output),
    }
}
fn text(v: &StructuredInfoValue) -> Result<String, RecordPronunciationRefusal> {
    String::from_utf8(leaf(v)?.into()).map_err(|_| RecordPronunciationRefusal::Output)
}
fn u64_field(v: &StructuredInfoValue, name: &str) -> Result<u64, RecordPronunciationRefusal> {
    Ok(u64::from_le_bytes(
        leaf(field(v, name)?)?
            .try_into()
            .map_err(|_| RecordPronunciationRefusal::Output)?,
    ))
}

/// Explicit reduced route. It carries no parser, POS or commitment claim.
pub struct ReducedHeuristicPronunciation<'a> {
    pronounced: crate::PronouncedText<'a>,
}
impl ReducedHeuristicPronunciation<'_> {
    pub fn pronounced(&self) -> &crate::PronouncedText<'_> {
        &self.pronounced
    }
    pub fn route(&self) -> &'static str {
        "reduced-heuristic-compact-text@1"
    }
}
pub fn prepare_reduced_heuristic<'a>(
    source: &'a str,
    storage: &'a mut [crate::VoiceEvent],
) -> Result<ReducedHeuristicPronunciation<'a>, crate::TextRefusal> {
    Ok(ReducedHeuristicPronunciation {
        pronounced: crate::pronounce(source, storage)?,
    })
}
