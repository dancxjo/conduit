//! Identity of an ordered installed work recipe, never execution permission.
#[cfg(feature = "authenticated-admission")]
use crate::VerifiedContinuationIntent;
use sha2::{Digest, Sha256};

/// Exact immutable entry pins emitted by checked fabrication. Dynamic Boot
/// bindings are intentionally absent; their typed slots are covered by recipe.
#[derive(Clone, Copy)]
pub struct ContinuationBundleEntry<'a> {
    pub name: &'a str,
    pub source_document: &'a str,
    pub checked_plot: &'a str,
    pub expanded_plot: &'a str,
    pub projection_digest: [u8; 32],
    pub recipe_digest: [u8; 32],
    /// Exact ordered canonical-placement to lowered-node mapping.
    pub placement_ids: &'a [&'a str],
    pub placement_nodes: &'a [u16],
}
#[derive(Debug, PartialEq, Eq)]
pub enum ContinuationBundleRefusal {
    Bounds,
    Duplicate,
    Source,
    Workset,
}
/// Signed identity match only. Does not certify media, resource possession,
/// canonical fresh Plans, committed Wake sequence, membership or Start.
pub struct CheckedContinuationBundle {
    digest: [u8; 32],
}
impl CheckedContinuationBundle {
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
}
/// Deterministic bounded construction transcript. Images/configurations and
/// execution ceilings are independently bound by the containing signed intent.
pub fn continuation_bundle_digest(
    source_digest: [u8; 32],
    entries: &[ContinuationBundleEntry<'_>],
) -> Result<[u8; 32], ContinuationBundleRefusal> {
    use ContinuationBundleRefusal as E;
    if source_digest == [0; 32] || entries.is_empty() || entries.len() > 16 {
        return Err(E::Bounds);
    }
    let mut hash = Sha256::new();
    hash.update(b"conduit.body/continuation-workset@1\0");
    hash.update(source_digest);
    hash.update((entries.len() as u16).to_le_bytes());
    for (index, entry) in entries.iter().enumerate() {
        if entries[..index]
            .iter()
            .any(|prior| prior.name == entry.name)
        {
            return Err(E::Duplicate);
        }
        for text in [
            entry.name,
            entry.source_document,
            entry.checked_plot,
            entry.expanded_plot,
        ] {
            if text.is_empty() || text.len() > crate::MAX_LIFECYCLE_ID_BYTES {
                return Err(E::Bounds);
            }
            hash.update((text.len() as u16).to_le_bytes());
            hash.update(text.as_bytes());
        }
        if entry.projection_digest == [0; 32]
            || entry.recipe_digest == [0; 32]
            || entry.placement_ids.len() != entry.placement_nodes.len()
            || entry.placement_nodes.is_empty()
            || entry.placement_nodes.len() > 256
        {
            return Err(E::Bounds);
        }
        for (n, placement) in entry.placement_ids.iter().enumerate() {
            if placement.is_empty() || placement.len() > crate::MAX_LIFECYCLE_ID_BYTES {
                return Err(E::Bounds);
            }
            if entry.placement_ids[..n].contains(placement) {
                return Err(E::Duplicate);
            }
            hash.update((placement.len() as u16).to_le_bytes());
            hash.update(placement.as_bytes());
        }
        for (n, node) in entry.placement_nodes.iter().enumerate() {
            if entry.placement_nodes[..n].contains(node) {
                return Err(E::Duplicate);
            }
        }
        hash.update(entry.projection_digest);
        hash.update(entry.recipe_digest);
        hash.update((entry.placement_nodes.len() as u16).to_le_bytes());
        for node in entry.placement_nodes {
            hash.update(node.to_le_bytes());
        }
    }
    Ok(hash.finalize().into())
}
/// Root supplies installed bytes/pins independently of candidate network input.
/// A match permits further canonical reconstruction checks, never effects.
#[cfg(feature = "authenticated-admission")]
pub fn check_continuation_bundle(
    intent: &VerifiedContinuationIntent<'_>,
    installed_source: &[u8],
    entries: &[ContinuationBundleEntry<'_>],
) -> Result<CheckedContinuationBundle, ContinuationBundleRefusal> {
    let source: [u8; 32] = Sha256::digest(installed_source).into();
    check_installed_continuation_bundle(intent, source, entries)
}

/// MCU specialization for an independently Make-pinned Source digest. Root
/// must supply the compiled installed pin, never copy the candidate intent.
/// Avoids retaining Source text on a Moon; proves the same recipe identity only.
#[cfg(feature = "authenticated-admission")]
pub fn check_installed_continuation_bundle(
    intent: &VerifiedContinuationIntent<'_>,
    source: [u8; 32],
    entries: &[ContinuationBundleEntry<'_>],
) -> Result<CheckedContinuationBundle, ContinuationBundleRefusal> {
    if source != intent.intent().source_digest {
        return Err(ContinuationBundleRefusal::Source);
    }
    let digest = continuation_bundle_digest(source, entries)?;
    if digest != intent.intent().workset_digest {
        return Err(ContinuationBundleRefusal::Workset);
    }
    Ok(CheckedContinuationBundle { digest })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transcript_binds_order_slots_and_identity_boundaries() {
        let a = ContinuationBundleEntry {
            name: "a",
            source_document: "source",
            checked_plot: "checked",
            expanded_plot: "expanded",
            projection_digest: [1; 32],
            recipe_digest: [2; 32],
            placement_ids: &["placement/a", "placement/b"],
            placement_nodes: &[0, 1],
        };
        let b = ContinuationBundleEntry { name: "b", ..a };
        let digest =
            |entries: &[ContinuationBundleEntry<'_>]| continuation_bundle_digest([3; 32], entries);
        assert_ne!(digest(&[a, b]), digest(&[b, a]));
        assert_ne!(
            digest(&[a]),
            digest(&[ContinuationBundleEntry {
                placement_nodes: &[1, 0],
                ..a
            }])
        );
        assert_ne!(
            digest(&[a]),
            digest(&[ContinuationBundleEntry {
                recipe_digest: [4; 32],
                ..a
            }])
        );
        assert_ne!(
            digest(&[a]),
            digest(&[ContinuationBundleEntry {
                placement_ids: &["placement/c", "placement/b"],
                ..a
            }])
        );
        assert_eq!(digest(&[a, a]), Err(ContinuationBundleRefusal::Duplicate));
        assert_eq!(
            digest(&[ContinuationBundleEntry {
                placement_nodes: &[0, 0],
                ..a
            }]),
            Err(ContinuationBundleRefusal::Duplicate)
        );
        assert_eq!(digest(&[]), Err(ContinuationBundleRefusal::Bounds));
    }
}
