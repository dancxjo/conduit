//! Closed proposal-v2 Window8 Source ports. Original checked Source programs,
//! exact generated descriptors and complete Native readmission travel together.
//! This private selection is not a public snapshot/commit authority.
use crate::parser_session_pure_source::{
    PreparedParserPureSource, PureSourceLimits, PureSourceRefusal, PureSourceReservation,
};
use alloc::{rc::Rc, vec::Vec};
use conduit_plot::rust_binding::{
    NativeFamilyTypeDescriptor, PreparedNativeFamily, PreparedNativeRustBinding,
};
use core::cell::RefCell;

type Family = Rc<RefCell<PreparedNativeFamily>>;
type Prepare = fn(
    &'static str,
    &'static str,
    Family,
    Family,
    PureSourceLimits,
) -> Result<PreparedParserPureSource, PureSourceRefusal>;
type Reserve = fn(
    &'static str,
    &Family,
    &Family,
    PureSourceLimits,
) -> Result<PureSourceReservation, PureSourceRefusal>;
pub(crate) struct Window8PortSpec {
    pub(crate) name: &'static str,
    pub(crate) original_programs: &'static str,
    pub(crate) original_custody: &'static str,
    pub(crate) input: &'static NativeFamilyTypeDescriptor,
    pub(crate) output: &'static NativeFamilyTypeDescriptor,
    pub(crate) prepare: Prepare,
    pub(crate) reservation: Reserve,
}
/// Private orchestration locator only. Registry readiness and complete history
/// custody remain required; a selected name conveys no admission authority.
pub(crate) fn port_index(name: &str) -> Option<usize> {
    PORTS.iter().position(|port| port.name == name)
}
macro_rules! port {
    ($name:literal, $file:literal, $input:ident, $output:ident) => {
        Window8PortSpec {
            name: $name,
            original_programs: include_str!(concat!(env!("OUT_DIR"), "/", $file, ".hex")),
            original_custody: include_str!(concat!(env!("OUT_DIR"), "/", $file, ".custody")),
            input: <crate::generated::$input as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
            output: <crate::generated::$output as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
            prepare: PreparedParserPureSource::prepare_fixed::<
                crate::generated::$input,
                crate::generated::$output,
            >,
            reservation: PreparedParserPureSource::reservation::<
                crate::generated::$input,
                crate::generated::$output,
            >,
        }
    };
}
// Every group is a complete generated closure; runtime64/root16 unchanged.
// Text partition only until the selected checked preparation gate succeeds.
pub(crate) static FAMILY_ROOTS: &[&[&NativeFamilyTypeDescriptor]] = &[
    &[
        <crate::generated::LanguageParserWindow8QualifiedIndependentCommitRequest as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8QualifiedDependencyProposal as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8QualifiedLexicalAnalysis as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8QualifiedLexicalProposal as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8ReanalysisContext as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8GrowContext as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8QualifiedChoiceBranch as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8QualifiedLexicalRebase as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8WaitProposal as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8EmptyGrowContext as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserProposalWindow8OriginQuery as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8EmptySeedProposal as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
    ],
    &[
        <crate::generated::LanguageParserWindow8SessionSeedContext as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8EmptySeedRequest as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8QualifiedDependencyRebase as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8QualifiedLexicalAnchorContext as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8IndependentCommitRebase as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserProposalWindow8FeatureContext as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8DependencyCompatibilityDecision as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8ChoiceQuery as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8CodeQuery as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8RawClassContext as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8RawFeatureContext as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8RawMerge as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
    ],
    &[
        <crate::generated::LanguageParserWindow8RawRequest as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8RawAdvance as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8GrowProposal as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8IndependentCommitSetProposal as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8ReanalysisProposal as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8Begin as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8IndependentCommitInitialize as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserProposalWindow8V2Features as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8RawClassRelations as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8RawModelFeatures as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8ClassQuery as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8RawWalk as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserProposalWindow8V2ModelScores as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8Available as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8Completion as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8Ordinal as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
    ],
    &[
        <crate::generated::LanguageParserWindow8ProtectedChoiceDecision as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8RootCount as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
        <crate::generated::LanguageParserWindow8Selected as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR,
    ],
];
// Bare numerical aliases are deliberately excluded: the exact original model
// Source/Plan mixed owner admits those two boundaries separately.
pub(crate) static PORTS: &[Window8PortSpec] = &[
    port!(
        "language-proposal-window8-feature-context",
        "window8_session_00",
        LanguageParserProposalWindow8FeatureQuery,
        LanguageParserProposalWindow8FeatureContext
    ),
    port!(
        "language-proposal-window8-origins",
        "window8_session_01",
        LanguageParserProposalWindow8OriginQuery,
        LanguageParserProposalWindow8RawOrigins
    ),
    port!(
        "language-proposal-window8-v2-feature-values",
        "window8_session_03",
        LanguageParserProposalWindow8FeatureContext,
        LanguageParserProposalWindow8V2RawFeatures
    ),
    port!(
        "language-window8-available",
        "window8_session_05",
        LanguageLexicalTape,
        LanguageParserWindow8Available
    ),
    port!(
        "language-window8-choice-frontier",
        "window8_session_06",
        LanguageParserWindow8ChoiceQuery,
        LanguageParserWindow8Selected
    ),
    port!(
        "language-window8-class-context",
        "window8_session_07",
        LanguageParserWindow8RawClassContext,
        LanguageParserWindow8RawContext
    ),
    port!(
        "language-window8-class-index",
        "window8_session_08",
        LanguageParserWindow8ClassQuery,
        LanguageParserWindow8RawClassIndex
    ),
    port!(
        "language-window8-class-relation",
        "window8_session_09",
        LanguageParserWindow8RawClassRelations,
        LanguageParserWindow8RawClass
    ),
    port!(
        "language-window8-class-relations",
        "window8_session_10",
        LanguageParserWindow8RawClassIndex,
        LanguageParserWindow8RawClassRelations
    ),
    port!(
        "language-window8-complete",
        "window8_session_11",
        LanguageParserWindow8RawState,
        LanguageParserWindow8Completion
    ),
    port!(
        "language-window8-dependency-compatible",
        "window8_session_12",
        LanguageParserWindow8DependencyCompatibilityQuery,
        LanguageParserWindow8DependencyCompatibilityDecision
    ),
    port!(
        "language-window8-empty-codes",
        "window8_session_13",
        LanguageParserWindow8Ordinal,
        LanguageParserWindow8RawCodes
    ),
    port!(
        "language-window8-empty-grow",
        "window8_session_14",
        LanguageParserWindow8EmptyGrowContext,
        LanguageParserWindow8GrowProposal
    ),
    port!(
        "language-window8-feature-context",
        "window8_session_15",
        LanguageParserWindow8RawFeatureQuery,
        LanguageParserWindow8RawFeatureContext
    ),
    port!(
        "language-window8-feature-values",
        "window8_session_16",
        LanguageParserWindow8RawFeatureContext,
        LanguageParserWindow8RawModelFeatures
    ),
    port!(
        "language-window8-grow",
        "window8_session_17",
        LanguageParserWindow8GrowContext,
        LanguageParserWindow8GrowProposal
    ),
    port!(
        "language-window8-independent-commit-initialize",
        "window8_session_18",
        LanguageParserWindow8IndependentCommitInitialize,
        LanguageParserWindow8IndependentCommitSetProposal
    ),
    port!(
        "language-window8-independent-commit-rebase",
        "window8_session_19",
        LanguageParserWindow8IndependentCommitRebase,
        LanguageParserWindow8IndependentCommitSet
    ),
    port!(
        "language-window8-initialize",
        "window8_session_20",
        LanguageParserWindow8Begin,
        LanguageParserWindow8RawState
    ),
    port!(
        "language-window8-move-apply",
        "window8_session_21",
        LanguageParserWindow8RawContext,
        LanguageParserWindow8RawResult
    ),
    port!(
        "language-window8-move-context",
        "window8_session_22",
        LanguageParserWindow8RawRequest,
        LanguageParserWindow8RawContext
    ),
    port!(
        "language-window8-move-legal-left",
        "window8_session_23",
        LanguageParserWindow8RawContext,
        LanguageParserWindow8RawContext
    ),
    port!(
        "language-window8-move-legal-reduce",
        "window8_session_24",
        LanguageParserWindow8RawContext,
        LanguageParserWindow8RawContext
    ),
    port!(
        "language-window8-move-legal-right-nonroot",
        "window8_session_25",
        LanguageParserWindow8RawContext,
        LanguageParserWindow8RawContext
    ),
    port!(
        "language-window8-move-legal-right-root",
        "window8_session_26",
        LanguageParserWindow8RawContext,
        LanguageParserWindow8RawContext
    ),
    port!(
        "language-window8-move-legal-shift",
        "window8_session_27",
        LanguageParserWindow8RawContext,
        LanguageParserWindow8RawContext
    ),
    port!(
        "language-window8-qualified-dependency-analysis",
        "window8_session_28",
        LanguageParserWindow8QualifiedDependencyQuery,
        LanguageParserWindow8QualifiedDependencyProposal
    ),
    port!(
        "language-window8-qualified-dependency-anchor",
        "window8_session_29",
        LanguageParserWindow8QualifiedDependencyAnalysis,
        LanguageParserWindow8DependencyAnchorProposal
    ),
    port!(
        "language-window8-qualified-dependency-rebase",
        "window8_session_30",
        LanguageParserWindow8QualifiedDependencyRebase,
        LanguageParserWindow8DependencyAnchorProposal
    ),
    port!(
        "language-window8-qualified-independent-commit",
        "window8_session_31",
        LanguageParserWindow8QualifiedIndependentCommitRequest,
        LanguageParserWindow8IndependentCommitSetProposal
    ),
    port!(
        "language-window8-qualified-lexical-anchor",
        "window8_session_32",
        LanguageParserWindow8QualifiedLexicalAnchorContext,
        LanguageParserWindow8QualifiedLexicalAnchor
    ),
    port!(
        "language-window8-qualified-lexical-anchor-scalars",
        "window8_session_33",
        LanguageParserWindow8QualifiedLexicalAnalysis,
        LanguageParserWindow8QualifiedLexicalAnchorScalars
    ),
    port!(
        "language-window8-qualified-lexical-origin",
        "window8_session_34",
        LanguageParserWindow8QualifiedLexicalAnalysis,
        LanguageLexicalProposalOrigin
    ),
    port!(
        "language-window8-qualified-lexical-proposal",
        "window8_session_35",
        LanguageParserWindow8QualifiedLexicalQuery,
        LanguageParserWindow8QualifiedLexicalProposal
    ),
    port!(
        "language-window8-qualified-lexical-rebase-origin",
        "window8_session_36",
        LanguageParserWindow8QualifiedLexicalRebase,
        LanguageLexicalProposalOrigin
    ),
    port!(
        "language-window8-qualified-lexical-rebase-scalars",
        "window8_session_37",
        LanguageParserWindow8QualifiedLexicalRebase,
        LanguageParserWindow8QualifiedLexicalAnchorScalars
    ),
    port!(
        "language-window8-qualified-lexical-rebase-token",
        "window8_session_38",
        LanguageParserWindow8QualifiedLexicalRebase,
        LanguageLexicalToken
    ),
    port!(
        "language-window8-qualified-lexical-token",
        "window8_session_39",
        LanguageParserWindow8QualifiedLexicalAnalysis,
        LanguageLexicalToken
    ),
    port!(
        "language-window8-qualified-protected-choice",
        "window8_session_40",
        LanguageParserWindow8QualifiedChoiceBranch,
        LanguageParserWindow8ProtectedChoiceDecision
    ),
    port!(
        "language-window8-rank-0-1",
        "window8_session_41",
        LanguageParserWindow8RawBeam,
        LanguageParserWindow8RawBeam
    ),
    port!(
        "language-window8-rank-0-2",
        "window8_session_42",
        LanguageParserWindow8RawBeam,
        LanguageParserWindow8RawBeam
    ),
    port!(
        "language-window8-rank-1-2",
        "window8_session_43",
        LanguageParserWindow8RawBeam,
        LanguageParserWindow8RawBeam
    ),
    port!(
        "language-window8-rank-1-3",
        "window8_session_44",
        LanguageParserWindow8RawBeam,
        LanguageParserWindow8RawBeam
    ),
    port!(
        "language-window8-rank-2-3",
        "window8_session_45",
        LanguageParserWindow8RawBeam,
        LanguageParserWindow8RawBeam
    ),
    port!(
        "language-window8-rank-insert",
        "window8_session_46",
        LanguageParserWindow8RawMerge,
        LanguageParserWindow8RawBeam
    ),
    port!(
        "language-window8-reanalysis",
        "window8_session_47",
        LanguageParserWindow8ReanalysisContext,
        LanguageParserWindow8ReanalysisProposal
    ),
    port!(
        "language-window8-root-count",
        "window8_session_48",
        LanguageParserWindow8RawState,
        LanguageParserWindow8RootCount
    ),
    port!(
        "language-window8-score-advance",
        "window8_session_49",
        LanguageParserWindow8RawAdvance,
        LanguageParserWindow8RawHypothesis
    ),
    port!(
        "language-window8-session-empty-seed",
        "window8_session_50",
        LanguageParserWindow8EmptySeedRequest,
        LanguageParserWindow8EmptySeedProposal
    ),
    port!(
        "language-window8-session-seed",
        "window8_session_51",
        LanguageParserWindow8SessionSeedContext,
        LanguageParserWindow8RawBeam
    ),
    port!(
        "language-window8-token-codes",
        "window8_session_52",
        LanguageParserWindow8CodeQuery,
        LanguageParserWindow8RawCodes
    ),
    port!(
        "language-window8-wait",
        "window8_session_53",
        LanguageParserWindow8Continuation,
        LanguageParserWindow8WaitProposal
    ),
    port!(
        "language-window8-walk-follow",
        "window8_session_54",
        LanguageParserWindow8RawWalk,
        LanguageParserWindow8RawWalk
    ),
    port!(
        "language-window8-walk-initialize",
        "window8_session_55",
        LanguageParserWindow8WalkQuery,
        LanguageParserWindow8RawWalk
    ),
];

/// Finds an actual complete owner of the exact generated descriptor; equal Type
/// bytes or a foreign descriptor never establish readiness.
pub(crate) fn family_for(
    families: &[Family],
    descriptor: &'static NativeFamilyTypeDescriptor,
) -> Result<usize, PureSourceRefusal> {
    for (index, family) in families.iter().enumerate() {
        if family
            .try_borrow()
            .map_err(|_| PureSourceRefusal::Closed)?
            .contains_descriptor(descriptor)
        {
            return Ok(index);
        }
    }
    Err(PureSourceRefusal::Descriptor)
}

/// Checks every fixed port and sums Source/history costs without allocating.
/// Family construction and original model/target storage remain enclosing
/// whole-profile obligations. Shared families are charged by that owner once.
pub(crate) fn reservations(
    families: &[Family],
    limits: &[PureSourceLimits],
    maximum_source_and_history_bytes: usize,
) -> Result<Window8PortReservation, PureSourceRefusal> {
    if limits.len() != PORTS.len() {
        return Err(PureSourceRefusal::Pressure);
    }
    let mut receipt = Window8PortReservation {
        retained_source_bytes: 0,
        history_bytes: 0,
        sequential_source_preparation_bytes: 0,
        sequential_native_bytes: 0,
    };
    for (port, limits) in PORTS.iter().zip(limits) {
        let input = family_for(families, port.input)?;
        let output = family_for(families, port.output)?;
        let r = (port.reservation)(
            port.original_programs,
            &families[input],
            &families[output],
            *limits,
        )?;
        receipt.retained_source_bytes = receipt
            .retained_source_bytes
            .checked_add(r.source_retained_bytes_bound)
            .ok_or(PureSourceRefusal::Pressure)?;
        receipt.history_bytes = receipt
            .history_bytes
            .checked_add(r.history_bytes_bound)
            .ok_or(PureSourceRefusal::Pressure)?;
        receipt.sequential_source_preparation_bytes = receipt
            .sequential_source_preparation_bytes
            .max(r.source_preparation_bytes_bound);
        receipt.sequential_native_bytes = receipt
            .sequential_native_bytes
            .max(r.active_native_bytes_bound);
    }
    let combined = receipt
        .retained_source_bytes
        .checked_add(receipt.history_bytes)
        .and_then(|n| n.checked_add(receipt.sequential_source_preparation_bytes))
        .and_then(|n| n.checked_add(receipt.sequential_native_bytes))
        .ok_or(PureSourceRefusal::Pressure)?;
    if combined > maximum_source_and_history_bytes {
        return Err(PureSourceRefusal::Pressure);
    }
    Ok(receipt)
}
pub(crate) struct Window8PortReservation {
    pub(crate) retained_source_bytes: usize,
    pub(crate) history_bytes: usize,
    pub(crate) sequential_source_preparation_bytes: usize,
    pub(crate) sequential_native_bytes: usize,
}

/// Constructs only the fixed complete selection after the whole-bank Source
/// reservation. The enclosing driver must admit family/model/target/preparation
/// costs before creating those owners, then retain/cancel the resulting ports.
pub(crate) fn prepare(
    families: &[Family],
    limits: &[PureSourceLimits],
    maximum_source_and_history_bytes: usize,
) -> Result<Vec<PreparedParserPureSource>, PureSourceRefusal> {
    reservations(families, limits, maximum_source_and_history_bytes)?;
    let mut owners = Vec::new();
    owners
        .try_reserve_exact(PORTS.len())
        .map_err(|_| PureSourceRefusal::Pressure)?;
    if owners.capacity() != PORTS.len() {
        return Err(PureSourceRefusal::Pressure);
    }
    for (port, limits) in PORTS.iter().zip(limits) {
        let input = family_for(families, port.input)?;
        let output = family_for(families, port.output)?;
        owners.push((port.prepare)(
            port.original_programs,
            port.original_custody,
            Rc::clone(&families[input]),
            Rc::clone(&families[output]),
            *limits,
        )?);
    }
    Ok(owners)
}
