//! Fixed retained epoch provenance and the successful next-epoch preparation bundle.
use super::*;
// All four survivor receipts stay inline and move together on success/refusal.
// Indirection would add storage allocation solely to change enum/stack layout.
#[allow(clippy::large_enum_variant)]
pub(super) enum Material {
    Initial(PreparedAnalysisInitialization),
    #[cfg(target_has_atomic = "ptr")]
    ResourceRebased {
        receipts: [PreparedResourceParserRebaseReplay; 4],
        envelopes: Vec<Vec<u8>>,
    },
    Rebased {
        lexical: PreparedLexicalTape,
        receipts: [PreparedParserRebaseReplay; 4],
        envelopes: Vec<Vec<u8>>,
    },
}
pub(super) type PreparedNextEpoch<'a> = (
    AnalysisSession<'a>,
    PreparedLexicalTape,
    LanguageParserScorerBeam,
    u64,
    Vec<Vec<u8>>,
);
