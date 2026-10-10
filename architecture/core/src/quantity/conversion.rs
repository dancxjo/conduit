//! Reviewed angle-family distinction shared by exact quantity arithmetic.
use super::CatalogUnit;
pub(super) const fn is_radian(unit: CatalogUnit) -> bool {
    matches!(
        unit,
        CatalogUnit::Microradian | CatalogUnit::Milliradian | CatalogUnit::Radian
    )
}
