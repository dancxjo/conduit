use super::*;
use crate::preparation_backing::Backing;
impl Backing for AdmittedGlyphValues {
    fn backing(&self) -> Option<usize> {
        if !self.source_values.is_empty() || !self.values.is_empty() {
            return None;
        }
        // Empty BTreeMaps may retain an empty root. Account their maximal root
        // slots even though newly checked literal sources normally have none.
        crate::preparation_backing::sum([
            empty_root::<(usize, usize), (crate::Expression, CanonicalStructuredStartupValue)>(),
            empty_root::<(usize, usize), (TypedGlyphLiteralSyntax, CanonicalStructuredStartupValue)>(
            ),
        ])
    }
}
fn empty_root<K, V>() -> Option<usize> {
    use core::mem::{align_of, size_of};
    size_of::<K>()
        .checked_add(size_of::<V>())?
        .checked_add(align_of::<K>())?
        .checked_add(align_of::<V>())?
        .checked_mul(11)?
        .checked_add(16 * size_of::<usize>())
}
