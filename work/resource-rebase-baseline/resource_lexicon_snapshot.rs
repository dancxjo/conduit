//! Initial-source-only resource lexical snapshot preparation. Source correlation
//! is replayed over actual legacy tokenization and complete borrowed resource
//! rows. No generic resource-reader Back, epoch/rebase or model proof is claimed.
use crate::{lexical::*, train_lexicon::*, *};
use alloc::{string::ToString, vec::Vec};
use conduit_plot::{
    rust_binding::{BoundedSequence, NativeBindingRefusal, NativeRustBinding},
    PortableExpressionProgram,
};

pub const SOURCE: &str = include_str!("../resource_lexicon_snapshot.conduit");
pub const PROFILE: &str = "language/resource-lexical-snapshot/initial-short4@1";
#[derive(Debug)]
pub enum ResourceSnapshotRefusal {
    PriorRequiresRebase,
    Language,
    Lexical(LexicalRefusal),
    Resource(TrainLexiconRefusal),
    Absent { ordinal: usize },
    Bound,
    Native(NativeBindingRefusal),
    Program,
    Correlation { ordinal: usize },
}
pub struct PreparedResourceSnapshotProgram {
    program: PortableExpressionProgram,
}
impl PreparedResourceSnapshotProgram {
    pub fn prepare() -> Result<Self, ResourceSnapshotRefusal> {
        PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
            env!("OUT_DIR"),
            "/resource_lexical_binding.hex"
        )))
        .map(|program| Self { program })
        .map_err(|_| ResourceSnapshotRefusal::Program)
    }
}
pub struct ResourceSnapshotBinding<'a> {
    ordinal: usize,
    row: TrainLexiconLookup<'a>,
    query: crate::generated::LanguageResourceLexicalBinding,
    input: Vec<u8>,
    output: Vec<u8>,
}
impl<'a> ResourceSnapshotBinding<'a> {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn row(&self) -> &TrainLexiconLookup<'a> {
        &self.row
    }
    pub fn query(&self) -> &crate::generated::LanguageResourceLexicalBinding {
        &self.query
    }
    pub fn input_canonical(&self) -> &[u8] {
        &self.input
    }
    pub fn output_canonical(&self) -> &[u8] {
        &self.output
    }
}
pub struct PreparedResourceLexicalSnapshot<'a> {
    lexicon: &'a PreparedTrainLexicon,
    lexical: PreparedLexicalTape,
    bindings: Vec<ResourceSnapshotBinding<'a>>,
}
impl<'a> PreparedResourceLexicalSnapshot<'a> {
    pub fn prepare(
        source: &LanguageTextRevision,
        lexicon: &'a PreparedTrainLexicon,
        program: &PreparedResourceSnapshotProgram,
    ) -> Result<Self, ResourceSnapshotRefusal> {
        use ResourceSnapshotRefusal::*;
        // A new owner must implement checked resource-backed lineage/rebase;
        // never bypass the legacy exact-profile/prior correspondence contract.
        if source.material().language().get() != "language/en" {
            return Err(Language);
        }
        if source.prior().is_some() {
            return Err(PriorRequiresRebase);
        }
        let provenance = LinguisticDerivationProvenance::library(
            PROFILE.to_string(),
            digest_hex(lexicon.reference().semantic_digest().map_err(|_| Bound)?),
        )
        .map_err(Native)?;
        let empty = LanguageLexicalProfile::new(
            BoundedSequence::new(),
            PROFILE.to_string(),
            source.material().language().clone(),
            provenance.clone(),
        )
        .map_err(Native)?;
        let skeleton = prepare_lexical_tape(source, &empty, None).map_err(Lexical)?;
        if skeleton.tape().tokens().is_empty() || skeleton.tape().tokens().len() > 4 {
            return Err(Bound);
        }
        let mut entries: Vec<LanguageLexicalEntry> = Vec::new();
        let mut rows = Vec::new();
        for (ordinal, token) in skeleton.tape().tokens().iter().enumerate() {
            if !matches!(token.completeness(), LanguageLexicalCompleteness::Complete) {
                continue;
            }
            let row = lexicon
                .lookup(token.surface())
                .map_err(Resource)?
                .ok_or(Absent { ordinal })?;
            let entry = entry(&row)?;
            if !entries
                .iter()
                .any(|prior| prior.surface() == entry.surface())
            {
                entries.push(entry.clone());
            }
            rows.push((ordinal, row, entry));
        }
        // Snapshot identity is content-derived, never reused as if changing
        // active entries were the same legacy full profile.
        let encoded = BoundedSequence::<_, 64>::try_from_iter(entries).map_err(|_| Bound)?;
        let temporary = LanguageLexicalProfile::new(
            encoded,
            PROFILE.to_string(),
            source.material().language().clone(),
            provenance,
        )
        .map_err(Native)?;
        let identity = digest_hex(conduit_core::semantic_digest(
            PROFILE,
            &temporary.clone().encode().map_err(Native)?,
        ));
        let profile = LanguageLexicalProfile::new(
            temporary.entries().clone(),
            identity,
            temporary.language().clone(),
            temporary.provenance().clone(),
        )
        .map_err(Native)?;
        let lexical = prepare_lexical_tape(source, &profile, None).map_err(Lexical)?;
        let mut bindings = Vec::with_capacity(rows.len());
        for (ordinal, row, entry) in rows {
            let token = lexical.tape().tokens()[ordinal].clone();
            let query = crate::generated::LanguageResourceLexicalBinding::new(
                ordinal as u64,
                lexicon.reference().clone(),
                entry,
                row.owner().reference().clone(),
                source.material().clone(),
                token,
            )
            .map_err(Native)?;
            let input = query.clone().encode().map_err(Native)?;
            if input.len() > 65536 {
                return Err(Bound);
            }
            let output = program.program.evaluate(&input).map_err(|_| Program)?;
            if output.as_slice() != [1] {
                return Err(Correlation { ordinal });
            }
            bindings.push(ResourceSnapshotBinding {
                ordinal,
                row,
                query,
                input,
                output,
            });
        }
        Ok(Self {
            lexicon,
            lexical,
            bindings,
        })
    }
    pub fn lexicon(&self) -> &'a PreparedTrainLexicon {
        self.lexicon
    }
    pub fn lexical(&self) -> &PreparedLexicalTape {
        &self.lexical
    }
    pub fn bindings(&self) -> &[ResourceSnapshotBinding<'a>] {
        &self.bindings
    }
    pub fn source_id(&self) -> &'static str {
        crate::parser_commitment::COMMITMENT_SOURCE_ID
    }
}
fn digest_hex(bytes: [u8; 32]) -> alloc::string::String {
    use core::fmt::Write;
    let mut result = alloc::string::String::new();
    for byte in bytes {
        write!(result, "{byte:02x}").expect("String");
    }
    result
}
fn entry(row: &TrainLexiconLookup<'_>) -> Result<LanguageLexicalEntry, ResourceSnapshotRefusal> {
    use ResourceSnapshotRefusal::{Bound, Native};
    let mut candidates = Vec::new();
    for code in row.pos_codes() {
        use LanguageLexicalPos::*;
        let pos = match code {
            0 => Adjective,
            1 => Adposition,
            2 => Adverb,
            3 => Auxiliary,
            4 => CoordinatingConjunction,
            5 => Determiner,
            6 => Interjection,
            7 => Noun,
            8 => Numeral,
            9 => Particle,
            10 => Pronoun,
            11 => ProperNoun,
            12 => Punctuation,
            13 => SubordinatingConjunction,
            14 => Symbol,
            15 => Verb,
            16 => Other,
            _ => return Err(Bound),
        };
        candidates.push(
            LanguageLexicalCandidate::new(row.surface().to_string(), BoundedSequence::new(), pos)
                .map_err(Native)?,
        );
    }
    LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter(candidates).map_err(|_| Bound)?,
        row.surface().to_string(),
    )
    .map_err(Native)
}

#[cfg(test)]
#[path = "resource_lexicon_snapshot_tests.rs"]
mod tests;
