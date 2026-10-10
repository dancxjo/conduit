//! Preserve an unchanged installed catalog when Source adds no Native Types.
use super::*;
use alloc::borrow::Cow;

pub(crate) fn check<'a>(
    declarations: &[TypeSyntax],
    base: &'a StartupCatalog,
) -> Result<(Vec<CheckedNativeType>, Cow<'a, StartupCatalog>), SyntaxCheckDiagnostic> {
    // Imported families can install captured dependencies even with no local
    // declarations. They retain the original complete instantiation path.
    if declarations.is_empty() && base.native_families.is_empty() {
        return Ok((Vec::new(), Cow::Borrowed(base)));
    }
    super::check_native_types(declarations, base)
        .map(|(types, catalog)| (types, Cow::Owned(catalog)))
}
