//! Requested-allocation term for canonical expected-offer construction only.
//! One closed preparation calls wake validation twice (budget and driver
//! preparation), and chime validation three times (budget, driver preparation,
//! and prepare_startup_outputs). Sum every request, including dropped catalog
//! presentation Strings. Refusal stops at a prefix of these constructors.
//! Caller error formatting, PCM/gates/stores/drivers, provider preparation and
//! all Root/IMAGE or effect authority belong to separate owners.
//!
//! Audited producers: semantics/catalog/src/{body_startup.rs,contract.rs};
//! core/capability_offer.rs::{Kind::validate,Kind::semantic_contract,
//! BackOfferBuilder::new,BackOfferBuilder::build}; std/offers/src/startup_chime.rs.
//! Exact public constants prevent silent duplicate identity definitions.

use conduit_core::{
    AuthorityRequirement, HostCallRequirement, KindSemanticLaw, PortDescriptor,
    ResourceRequirement, BOOL_INFO_ID,
};
use conduit_semantic_catalog::{
    BODY_WAKE_EXAMPLE, BODY_WAKE_KIND, BODY_WAKE_NAME, BODY_WAKE_REVISION, BODY_WAKE_SUMMARY,
    STARTUP_CHIME_EXAMPLE, STARTUP_CHIME_KIND, STARTUP_CHIME_NAME, STARTUP_CHIME_REVISION,
    STARTUP_CHIME_SUMMARY, STARTUP_PULSE_PORT,
};
use conduit_std_offers::{
    AUDIO_PLAYBACK_AUTHORITY_CONTRACT, AUDIO_PLAYBACK_RESOURCE_CLASS, BODY_WAKE_ARTIFACT,
    BODY_WAKE_CAPABILITY, BODY_WAKE_IMPLEMENTATION, BODY_WAKE_PROFILE, STARTUP_CHIME_ARTIFACT,
    STARTUP_CHIME_CAPABILITY, STARTUP_CHIME_IMPLEMENTATION, STARTUP_CHIME_OPERATION,
    STARTUP_CHIME_PROFILE,
};
use std::mem::{align_of, size_of};

fn add(a: usize, b: usize) -> Option<usize> {
    a.checked_add(b)
}
fn mul(a: usize, b: usize) -> Option<usize> {
    a.checked_mul(b)
}
fn strings(values: &[&str]) -> Option<usize> {
    values
        .iter()
        .try_fold(0usize, |sum, value| add(sum, value.len()))
}
fn round(n: usize, alignment: usize) -> Option<usize> {
    Some(add(n, alignment - 1)? / alignment * alignment)
}

// Pinned alloc BTree node.rs:43-66,86: B=6, capacity11. Kind::validate inserts
// one borrowed port symbol, creating exactly one leaf and no owned key String.
// Its fields: pointer, two u16 fields, [K;11], [V;11], here K=&str,V=().
// Round each field to maximum alignment for a conservative padding envelope.
fn port_symbol_leaf() -> Option<usize> {
    let alignment = align_of::<usize>().max(align_of::<&str>());
    add(
        add(
            round(size_of::<usize>(), alignment)?,
            mul(2, round(size_of::<u16>(), alignment)?)?,
        )?,
        round(mul(11, size_of::<&str>())?, alignment)?,
    )
}

fn common_records() -> Option<usize> {
    // One vec![pulse_port(...)], one vec![Terminal(...)] before validation,
    // and one semantic_laws clone in BackOfferBuilder::build. Terminal is
    // inline and has no further owned payload in either selected contract.
    add(
        add(
            size_of::<PortDescriptor>(),
            mul(2, size_of::<KindSemanticLaw>())?,
        )?,
        port_symbol_leaf()?,
    )
}

fn wake_offer() -> Option<usize> {
    let payload = strings(&[
        BODY_WAKE_KIND,
        BODY_WAKE_NAME,
        BODY_WAKE_SUMMARY,
        BODY_WAKE_REVISION,
        STARTUP_PULSE_PORT,
        BOOL_INFO_ID,
        BODY_WAKE_CAPABILITY,
        BODY_WAKE_PROFILE,
        BODY_WAKE_IMPLEMENTATION,
        BODY_WAKE_ARTIFACT,
    ])?;
    // Shared producer example now owns one exact borrowed-str conversion.
    add(add(common_records()?, payload)?, BODY_WAKE_EXAMPLE.len())
}

fn chime_offer() -> Option<usize> {
    let payload = strings(&[
        STARTUP_CHIME_KIND,
        STARTUP_CHIME_NAME,
        STARTUP_CHIME_SUMMARY,
        STARTUP_CHIME_EXAMPLE,
        STARTUP_CHIME_REVISION,
        STARTUP_PULSE_PORT,
        BOOL_INFO_ID,
        STARTUP_CHIME_CAPABILITY,
        STARTUP_CHIME_PROFILE,
        STARTUP_CHIME_IMPLEMENTATION,
        STARTUP_CHIME_ARTIFACT,
        // Host call, resource requirement, and authority requirement each own
        // their String leaves independently, including repeated operation/Bool.
        STARTUP_CHIME_OPERATION,
        BOOL_INFO_ID,
        AUDIO_PLAYBACK_RESOURCE_CLASS,
        AUDIO_PLAYBACK_AUTHORITY_CONTRACT,
        STARTUP_CHIME_OPERATION,
        BOOL_INFO_ID,
    ])?;
    let requirements = add(
        add(
            size_of::<HostCallRequirement>(),
            size_of::<ResourceRequirement>(),
        )?,
        size_of::<AuthorityRequirement>(),
    )?;
    add(add(common_records()?, payload)?, requirements)
}

/// Allocation-free partial term for the exact two-operation constructor path.
/// Shared producer constants preserve one source of truth for text and IDs.
/// Existing complete semantic validation remains unchanged.
pub(super) fn derive() -> Option<usize> {
    if !cfg!(conduit_pinned_collection_layout) {
        return None;
    }
    // For these Bool ports, portable_value_bounds yields None. Its filtered
    // collection is empty; no CheckedValueContract is constructed. Startup
    // configuration is empty. Kind validation's realization/config/terminal
    // sets stay empty, initialized-state validation returns before cloning,
    // and the single Terminal law adds no further validation allocation.
    let total = add(mul(2, wake_offer()?)?, mul(3, chime_offer()?)?)?;
    if total > isize::MAX as usize {
        None
    } else {
        Some(total)
    }
}
