//! Owner-captured Source families. Only checked package installation creates these.
use crate::prelude::*;
use crate::{CheckedNativeType, TypeSyntax};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeTypeFamily {
    pub(crate) root: String,
    pub(crate) templates: Vec<TypeSyntax>,
    pub(crate) dependencies: Vec<CheckedNativeType>,
    pub(crate) package_content_digest: [u8; 32],
}

pub(crate) fn install(
    catalog: &mut crate::StartupCatalog,
    path: &str,
    root: &TypeSyntax,
    declarations: &[TypeSyntax],
    owner: &crate::StartupCatalog,
    package_content_digest: [u8; 32],
) -> Result<(), crate::SyntaxCheckDiagnostic> {
    let family = NativeTypeFamily {
        root: root.name.text.clone(),
        templates: declarations
            .iter()
            .filter(|value| !value.parameters.is_empty())
            .cloned()
            .collect(),
        dependencies: owner.retained_native_types(),
        package_content_digest,
    };
    if catalog.structured_type(path).is_some()
        || catalog.get(path).is_some()
        || catalog.native_families.contains_key(path)
    {
        return Err(super::diagnostic(
            root.name.span,
            alloc::format!("duplicate or ambiguous shipped Type family '{path}'"),
        ));
    }
    catalog.native_families.insert(path.into(), family);
    Ok(())
}
