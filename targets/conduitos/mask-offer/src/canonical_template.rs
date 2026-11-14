//! Immutable standard Mask compiler output. The dummy Host and grants here are
//! data only. A consumer must select actual providers, freshly authorized grants
//! and Lines and ordinarily admit the resulting exact Plan before execution.
use alloc::vec::Vec;
use conduit_core::*;
use conduit_face::{MaskPlot, MaskPlotBoundary, PlannedMaskPlot};
#[path = "canonical_template/data.rs"]
mod data;
#[path = "canonical_template/source.rs"]
mod source;

/// Opaque generated data. Construction never runs a parser, checker or planner.
/// No mutable template access, membership, Show or execution authority is exposed.
/// Consumers must separately fund any clone made from the borrowed planned data.
pub struct CanonicalNativeGraphicalTemplate {
    planned: PlannedMaskPlot,
}
impl CanonicalNativeGraphicalTemplate {
    pub fn planned_mask(&self) -> &PlannedMaskPlot {
        &self.planned
    }
}

/// Closed constructor receipt, not a memory grant. Root must reserve these bytes
/// on its original counter BEFORE calling construct, and retain that charge.
/// Borrowed immutable code/data and caller stack are not owned heap backing.
#[derive(Clone, Copy)]
pub struct CanonicalNativeGraphicalTemplatePreparation {
    _private: (),
}
impl CanonicalNativeGraphicalTemplatePreparation {
    /// Every String literal's actual UTF-8 bytes plus every nonempty Vec's exact
    /// element count times the target's real element size, and the inline owner.
    pub const fn retained_bytes(self) -> usize {
        data::BACKING_BYTES + core::mem::size_of::<CanonicalNativeGraphicalTemplate>()
    }
    /// Generated constructors use only String::from and Vec::from(array): each
    /// allocation moves directly into its sole result owner, with no heap scratch,
    /// cloning, compression, codec, verifier, asynchronous work or shared backing.
    pub const fn preparation_bytes(self) -> usize {
        self.retained_bytes()
    }
    pub fn construct(self) -> CanonicalNativeGraphicalTemplate {
        CanonicalNativeGraphicalTemplate {
            planned: data::construct(),
        }
    }
}
pub const fn canonical_native_graphical_template_preparation()
-> CanonicalNativeGraphicalTemplatePreparation {
    CanonicalNativeGraphicalTemplatePreparation { _private: () }
}

const fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}
// Source changes cannot silently keep old compiled identities or source spans.
const _: () = assert!(same_bytes(
    source::SOURCE,
    include_bytes!("../../../../plots/native-graphical-mask/main.conduit")
));

#[cfg(test)]
#[path = "canonical_template/tests.rs"]
mod tests;
