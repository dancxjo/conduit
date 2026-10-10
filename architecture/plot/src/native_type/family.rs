//! Owner-captured Source families. Only checked package installation creates these.
use crate::prelude::*;
use crate::{CheckedNativeType, TypeSyntax};
use alloc::collections::BTreeMap;
pub(crate) mod budget;
mod capture;
pub(crate) mod source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeTypeFamily {
    pub(crate) root: String,
    pub(crate) templates: Vec<TypeSyntax>,
    pub(crate) dependencies: Vec<CheckedNativeType>,
    pub(crate) origins: BTreeMap<String, TypeSyntax>,
    pub(crate) source_origins: BTreeMap<String, source::NativeTypeSourceOrigin>,
    pub(crate) package_content_digest: [u8; 32],
}

pub(crate) fn install(
    catalog: &mut crate::StartupCatalog,
    path: &str,
    root: &TypeSyntax,
    declarations: &[TypeSyntax],
    owner: &crate::StartupCatalog,
    package_content_digest: [u8; 32],
    owner_origins: &BTreeMap<String, source::NativeTypeSourceOrigin>,
) -> Result<(), crate::SyntaxCheckDiagnostic> {
    if catalog.structured_type(path).is_some()
        || catalog.get(path).is_some()
        || catalog.value_kind_alias(path).is_some()
        || catalog.native_families.contains_key(path)
    {
        return Err(super::diagnostic(
            root.name.span,
            alloc::format!("duplicate or ambiguous shipped Type family '{path}'"),
        ));
    }
    if catalog.native_families.len() >= budget::MAXIMUM_FAMILIES {
        return Err(super::diagnostic(
            root.name.span,
            "native family registry exceeds its 128-entry profile".into(),
        ));
    }
    budget::validate(catalog, None, root.name.span)?;
    let (selected, owner) = capture::select(root, declarations, owner)?;
    let imports = super::generic::imports::Imports::prepare(&owner)?;
    let mut templates = Vec::new();
    let mut origins = imports.origins;
    let mut source_origins = imports.source_origins;
    for declaration in &selected {
        let origin = owner_origins.get(&declaration.name.text).ok_or_else(|| {
            super::diagnostic(
                declaration.name.span,
                "checked family owner module provenance is missing".into(),
            )
        })?;
        source_origins.insert(declaration.name.text.clone(), origin.clone());
    }
    for original in selected.iter().filter(|value| !value.parameters.is_empty()) {
        let mut template = original.clone();
        super::generic::imports::rewrite(&mut template, &imports.aliases);
        origins.insert(template.name.text.clone(), original.clone());
        templates.push(template);
    }
    templates.extend(imports.templates);
    let dependencies = capture::dependencies(&templates, &selected, &imports.catalog)?;
    for dependency in &dependencies {
        if let Some(origin) = imports.catalog.native_type_source(&dependency.name) {
            source_origins.insert(dependency.name.clone(), origin.clone());
        }
    }
    let family = NativeTypeFamily {
        root: root.name.text.clone(),
        templates,
        dependencies,
        origins,
        package_content_digest,
        source_origins,
    };
    budget::validate(catalog, Some((path, &family)), root.name.span)?;
    catalog.native_families.insert(path.into(), family);
    Ok(())
}
