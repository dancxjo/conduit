//! Admission of immutable text revisions. The caller publishes a validated
//! candidate atomically; pressure/cancellation cannot mutate the previous value.
use crate::LanguageTextRevision;

/// Revision-domain schemas are installed explicitly, independently of the
/// identity family consumed by speech/listening/translation native bindings.
pub fn text_revision_types() -> alloc::vec::Vec<(&'static str, conduit_core::StructuredInfoType)> {
    alloc::vec![
        (
            "LanguageTextFinality",
            crate::LanguageTextFinality::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageTextPriorRevision",
            crate::LanguageTextPriorRevision::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageTextRevision",
            crate::LanguageTextRevision::semantic_type().expect("checked Language Type")
        ),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRevisionRefusal {
    Identity,
    Language,
    PriorRevision,
    Sequence,
    StablePrefixRange,
    StablePrefixRegressed,
    StablePrefixChanged,
    CommittedPrefixRange,
    CommittedPrefixChanged,
    RevisableFrontier,
}

/// Validate one source epoch using a finite scalar frontier. Commitment is
/// supplied independently by the consumer; finality never advances it.
/// Rejected candidates leave all material/frontiers unchanged.
pub fn validate_text_revision(
    previous: Option<&LanguageTextRevision>,
    candidate: &LanguageTextRevision,
    committed_prefix: u32,
    maximum_revisable_scalars: u32,
) -> Result<(), TextRevisionRefusal> {
    use TextRevisionRefusal::*;
    let content = candidate.material().text().as_str();
    let scalars = content.chars().count() as u32;
    let stable = candidate.stable_prefix().unwrap_or(0);
    if stable > scalars {
        return Err(StablePrefixRange);
    }
    if committed_prefix > scalars || committed_prefix > stable {
        return Err(CommittedPrefixRange);
    }
    if maximum_revisable_scalars > 4096 || scalars - committed_prefix > maximum_revisable_scalars {
        return Err(RevisableFrontier);
    }
    let Some(previous) = previous else {
        if candidate.prior().is_some() || *candidate.sequence() != 0 {
            return Err(PriorRevision);
        }
        if committed_prefix != 0 {
            return Err(CommittedPrefixRange);
        }
        return Ok(());
    };
    if previous.material().identity() != candidate.material().identity() {
        return Err(Identity);
    }
    if previous.material().language() != candidate.material().language() {
        return Err(Language);
    }
    let Some(prior) = candidate.prior() else {
        return Err(PriorRevision);
    };
    if prior.revision() != previous.material().revision()
        || prior.sequence() != previous.sequence()
        || candidate.material().revision() == previous.material().revision()
    {
        return Err(PriorRevision);
    }
    if previous.sequence().checked_add(1) != Some(*candidate.sequence()) {
        return Err(Sequence);
    }
    let prior_stable = previous.stable_prefix().unwrap_or(0);
    if stable < prior_stable {
        return Err(StablePrefixRegressed);
    }
    let old = previous.material().text().as_str();
    if committed_prefix > old.chars().count() as u32 {
        return Err(CommittedPrefixRange);
    }
    if !same_prefix(old, content, committed_prefix) {
        return Err(CommittedPrefixChanged);
    }
    if !same_prefix(old, content, prior_stable) {
        return Err(StablePrefixChanged);
    }
    Ok(())
}

fn same_prefix(left: &str, right: &str, scalars: u32) -> bool {
    left.chars()
        .take(scalars as usize)
        .eq(right.chars().take(scalars as usize))
}
