use super::*;
use crate::preparation_backing::{Backing, sum};
impl Backing for DimensionDefinition {
    fn backing(&self) -> Option<usize> {
        Some(0)
    }
}
impl Backing for UnitDefinition {
    fn backing(&self) -> Option<usize> {
        Some(0)
    }
}
impl Backing for Span {
    fn backing(&self) -> Option<usize> {
        Some(0)
    }
}
impl Backing for CheckedQuantityRole {
    fn backing(&self) -> Option<usize> {
        self.leaf_kind.backing()
    }
}
impl CheckedPhysicalCatalogue {
    pub(crate) fn owned_preparation_backing_bytes(&self) -> Option<usize> {
        // Full immutable snapshot is charged even if an external owner shares it.
        #[cfg(target_has_atomic = "ptr")]
        let header = 2 * core::mem::size_of::<usize>();
        #[cfg(not(target_has_atomic = "ptr"))]
        let header = 0;
        sum([
            Some(core::mem::size_of::<CheckedPhysicalCatalogueData>())
                .and_then(|v| v.checked_add(header)),
            self.dimensions.backing(),
            self.quantities.backing(),
            self.units.backing(),
            self.aliases.backing(),
            self.unit_spans.backing(),
        ])
    }
}
