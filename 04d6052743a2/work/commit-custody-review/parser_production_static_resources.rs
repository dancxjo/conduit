//! Read-only canonical artifacts are separate from prepared metadata heap.
//! Complete byte lengths are charged; pointers only deduplicate storage owners.
use super::definitions::ALL_FAMILIES;
use conduit_plot::rust_binding::{
    NativeFamilyConstraintDescriptor as Constraint, NativeFamilyTypeDescriptor,
};
use core::mem::size_of;
#[derive(Clone, Copy, Debug)]
pub(crate) struct ParserStaticResourceReceipt {
    pub descriptors: usize,
    pub canonical_bytes_bound: usize,
    pub descriptor_storage_bytes_bound: usize,
    pub source_canonical_bytes_bound: usize,
}
pub(crate) fn descriptor_resources() -> Option<ParserStaticResourceReceipt> {
    // A generation union can have 192 Types. Actual runtime owners still have
    // the independent 64-Type/16-root limits. This scan allocates no heap.
    let mut seen: [Option<&'static NativeFamilyTypeDescriptor>; 192] = [None; 192];
    let mut count = 0usize;
    let mut storage = 0usize;
    let mut canonical = 0usize;
    fn insert(
        seen: &mut [Option<&'static NativeFamilyTypeDescriptor>; 192],
        count: &mut usize,
        descriptor: &'static NativeFamilyTypeDescriptor,
    ) -> Option<()> {
        if seen[..*count]
            .iter()
            .flatten()
            .any(|previous| core::ptr::eq(*previous, descriptor))
        {
            return Some(());
        }
        *seen.get_mut(*count)? = Some(descriptor);
        *count += 1;
        Some(())
    }
    for kind in ALL_FAMILIES {
        storage = storage.checked_add(
            kind.roots()
                .len()
                .checked_mul(size_of::<&NativeFamilyTypeDescriptor>())?,
        )?;
        for descriptor in kind.roots() {
            insert(&mut seen, &mut count, descriptor)?;
        }
    }
    let mut index = 0;
    while index < count {
        let descriptor = seen[index]?;
        index += 1;
        storage = storage
            .checked_add(size_of::<NativeFamilyTypeDescriptor>())?
            .checked_add(
                descriptor
                    .children
                    .len()
                    .checked_mul(size_of::<&NativeFamilyTypeDescriptor>())?,
            )?
            .checked_add(descriptor.laws.len().checked_mul(size_of::<&[u8]>())?)?
            .checked_add(descriptor.contracts.len().checked_mul(size_of::<
                conduit_plot::rust_binding::NativeFamilyContractDescriptor,
            >())?)?;
        canonical = canonical.checked_add(descriptor.type_bytes.len())?;
        for law in descriptor.laws {
            canonical = canonical.checked_add(law.len())?;
        }
        for recipe in descriptor.contracts {
            canonical = canonical
                .checked_add(recipe.representation_path.len())?
                .checked_add(recipe.value_kind.len())?;
            storage = storage.checked_add(
                recipe
                    .constraints
                    .len()
                    .checked_mul(size_of::<Constraint>())?,
            )?;
            for constraint in recipe.constraints {
                match constraint {
                    Constraint::CanonicalMembership { members, .. } => {
                        storage =
                            storage.checked_add(members.len().checked_mul(size_of::<&[u8]>())?)?;
                        for member in *members {
                            canonical = canonical.checked_add(member.len())?;
                        }
                    }
                    Constraint::FixedIntegerRange {
                        minimum, maximum, ..
                    } => {
                        canonical = canonical
                            .checked_add(minimum.map_or(0, <[u8]>::len))?
                            .checked_add(maximum.map_or(0, <[u8]>::len))?;
                    }
                }
            }
        }
        for child in descriptor.children {
            insert(&mut seen, &mut count, child)?;
        }
    }
    let mut source = 0usize;
    use crate::parser_session_execution::ParserSessionEntry as E;
    for entry in [
        E::Availability,
        E::DecodeComplete,
        E::Completion,
        E::V2FeatureIndices,
        E::V2ScoreObservation,
        E::IndependentBranch,
        E::IndependentCommit,
        E::IndependentCommitRebase,
        E::IndependentCommitRebaseSets,
        E::IndependentMask,
        E::JointBranch,
        E::Commit,
        E::JointConsensus,
        E::Expansion,
        E::Rebase,
        E::Merge,
        E::JointScoreBand1000,
        E::StableFact,
        E::LegalMask,
        E::ProtectedOriginEdge,
        E::Initialize,
        E::ProtectedInsert,
        E::ProtectedRebase,
        E::ProtectionForestProjection,
        E::RetainedCommitAnchor,
        E::RevisionReset,
        E::ScoreProposal,
        E::Seed,
        E::Transition,
        E::V2ModelFeatures,
        E::V2Pos,
        E::WaitState,
    ] {
        source = source
            .checked_add(entry.name().len())?
            .checked_add(entry.program_hex().len())?
            .checked_add(entry.chain_custody().len())?;
    }
    Some(ParserStaticResourceReceipt {
        descriptors: count,
        canonical_bytes_bound: canonical,
        descriptor_storage_bytes_bound: storage,
        source_canonical_bytes_bound: source,
    })
}
