//! Source-owned exact revision metadata; no stability or playback commitment.
use crate::{LanguageTextRevision, LanguageTextRevisionLineage};
use conduit_plot::rust_binding::NativeBindingRefusal;
/// Preserve both complete revisions and admit their Source lineage law.
/// Consumers separately validate scalar prefixes and their finite frontier
/// with `validate_text_revision` before publishing changed text.
pub fn prepare_text_revision_lineage(
    previous: &LanguageTextRevision,
    next: &LanguageTextRevision,
) -> Result<LanguageTextRevisionLineage, NativeBindingRefusal> {
    LanguageTextRevisionLineage::new(next.clone(), previous.clone())
}
pub fn revision_lineage_types() -> alloc::vec::Vec<(&'static str, conduit_core::StructuredInfoType)>
{
    alloc::vec![(
        "LanguageTextRevisionLineage",
        LanguageTextRevisionLineage::semantic_type().expect("checked Language Type")
    )]
}
