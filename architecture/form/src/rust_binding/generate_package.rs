use super::{
    ExternalNativeRustBinding, RustBindingGenerationError, RustBindingModule, RustBindingOptions,
};
use crate::prelude::*;
use crate::{
    check_package_bundle, CheckedPackageBundle, ConduitLock, PackageBundleError, PackageCheckError,
    PackageExportCatalog, PackageMemberSource, PackageResolutionError, PackageSyntax,
    StartupCatalog, SyntaxCheckDiagnostic,
};
use alloc::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockedRustBindingGenerationError {
    InvalidLock(PackageResolutionError),
    SourceNotLocked,
    MissingLockedSource(String),
    DuplicateLockedSource(String),
    UnexpectedLockedSource(String),
    MissingExternalBinding(String),
    DuplicateExternalBinding(String),
    UnusedExternalBinding(String),
    ExternalBindingIdentityDrift(String),
    InvalidExternalRustPath(String),
    Bundle(PackageBundleError),
    NativeType(SyntaxCheckDiagnostic),
    Package(PackageCheckError),
    Generation(RustBindingGenerationError),
}

#[derive(Clone, Copy)]
pub struct LockedPackageBindingSource<'a> {
    pub bundle: &'a CheckedPackageBundle,
    pub manifest_source: &'a str,
    pub manifest: &'a PackageSyntax,
    pub member_sources: &'a [PackageMemberSource<'a>],
}

pub struct LockedPackageRustBindingInput<'a> {
    pub root: LockedPackageBindingSource<'a>,
    /// Exact source for every package named by `lock.packages`, including root.
    pub locked_sources: &'a [LockedPackageBindingSource<'a>],
    pub lock: &'a ConduitLock,
    /// Exact foreign semantic-Type identities and the Rust paths whose types
    /// implement `NativeRustBinding` for them. Root-owned Types are never
    /// accepted here and dependency bindings are never generated again.
    pub external_bindings: &'a [ExternalNativeRustBinding<'a>],
}

/// Generates bindings only after every source bundle named by the lock has
/// been revalidated. Dependency Types are installed from those checked sources
/// in dependency order; callers cannot inject ambient semantic meaning.
pub fn generate_locked_package_rust_bindings(
    input: LockedPackageRustBindingInput<'_>,
    options: &RustBindingOptions,
) -> Result<RustBindingModule, LockedRustBindingGenerationError> {
    let mut sources = BTreeMap::new();
    let mut package_catalog = Vec::with_capacity(input.locked_sources.len());
    for source in input.locked_sources {
        let path = source.bundle.package.path.clone();
        if sources.insert(path.clone(), *source).is_some() {
            return Err(LockedRustBindingGenerationError::DuplicateLockedSource(
                path,
            ));
        }
        source
            .bundle
            .validate_against(
                source.manifest_source,
                source.manifest,
                source.member_sources,
            )
            .map_err(LockedRustBindingGenerationError::Bundle)?;
        package_catalog.push(source.bundle.package.clone());
    }
    if !input.lock.roots.iter().any(|root| {
        root.path == input.root.bundle.package.path
            && root.version == input.root.bundle.package.version
            && root.content_digest == input.root.bundle.package.content_digest
    }) {
        return Err(LockedRustBindingGenerationError::SourceNotLocked);
    }
    for package in &input.lock.packages {
        if !sources.contains_key(&package.path) {
            return Err(LockedRustBindingGenerationError::MissingLockedSource(
                package.path.clone(),
            ));
        }
    }
    for path in sources.keys() {
        if !input
            .lock
            .packages
            .iter()
            .any(|package| &package.path == path)
        {
            return Err(LockedRustBindingGenerationError::UnexpectedLockedSource(
                path.clone(),
            ));
        }
    }
    input
        .lock
        .validate_against(&package_catalog)
        .map_err(LockedRustBindingGenerationError::InvalidLock)?;
    let locked_packages = input
        .lock
        .packages
        .iter()
        .map(|package| (package.path.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let mut semantic_catalog = StartupCatalog::new();
    let mut installed = BTreeSet::new();
    let mut foreign_types = Vec::new();
    install_dependencies(
        &input.root.bundle.package.path,
        &sources,
        &locked_packages,
        &mut installed,
        &mut semantic_catalog,
        &mut foreign_types,
        false,
    )?;
    let checked = check_package_bundle(
        input.root.bundle,
        input.root.manifest_source,
        input.root.manifest,
        input.root.member_sources,
        &semantic_catalog,
    )
    .map_err(LockedRustBindingGenerationError::Package)?;
    let foreign_by_identity = foreign_types
        .iter()
        .map(|value_type| (value_type.identity.as_str(), value_type))
        .collect::<BTreeMap<_, _>>();
    let required = checked
        .native_types
        .iter()
        .flat_map(|value_type| referenced_schema_identities(&value_type.value_type))
        .filter(|identity| foreign_by_identity.contains_key(identity.as_str()))
        .collect::<BTreeSet<_>>();
    let mut external_names = BTreeMap::new();
    for binding in input.external_bindings {
        if !valid_rust_type_path(binding.rust_type_path) {
            return Err(LockedRustBindingGenerationError::InvalidExternalRustPath(
                binding.rust_type_path.into(),
            ));
        }
        if external_names
            .insert(
                binding.semantic_identity.into(),
                binding.rust_type_path.into(),
            )
            .is_some()
        {
            return Err(LockedRustBindingGenerationError::DuplicateExternalBinding(
                binding.semantic_identity.into(),
            ));
        }
        if !foreign_by_identity.contains_key(binding.semantic_identity) {
            return Err(
                LockedRustBindingGenerationError::ExternalBindingIdentityDrift(
                    binding.semantic_identity.into(),
                ),
            );
        }
        if !required.contains(binding.semantic_identity) {
            return Err(LockedRustBindingGenerationError::UnusedExternalBinding(
                binding.semantic_identity.into(),
            ));
        }
    }
    if let Some(missing) = required
        .iter()
        .find(|identity| !external_names.contains_key(identity.as_str()))
    {
        return Err(LockedRustBindingGenerationError::MissingExternalBinding(
            missing.clone(),
        ));
    }
    super::generate::generate_rust_bindings_with_external_names(
        &checked.native_types,
        &[],
        options,
        &external_names,
    )
    .map_err(LockedRustBindingGenerationError::Generation)
}

fn referenced_schema_identities(value_type: &conduit_core::StructuredInfoType) -> BTreeSet<String> {
    use conduit_core::StructuredInfoTypeShape;
    let mut identities = BTreeSet::new();
    fn visit(value_type: &conduit_core::StructuredInfoType, identities: &mut BTreeSet<String>) {
        match value_type.shape() {
            StructuredInfoTypeShape::Nominal {
                schema,
                representation,
            } => {
                identities.insert(schema.as_str().into());
                visit(representation, identities);
            }
            StructuredInfoTypeShape::Record { schema, fields } => {
                identities.insert(schema.as_str().into());
                for field in fields {
                    visit(field.value_type(), identities);
                }
            }
            StructuredInfoTypeShape::Variant { schema, cases } => {
                identities.insert(schema.as_str().into());
                for case in cases {
                    visit(case.payload_type(), identities);
                }
            }
            StructuredInfoTypeShape::Sequence { element, .. }
            | StructuredInfoTypeShape::Collection { element, .. } => visit(element, identities),
            StructuredInfoTypeShape::Leaf(_) => {}
        }
    }
    visit(value_type, &mut identities);
    identities
}

pub(super) fn valid_rust_type_path(path: &str) -> bool {
    let path = path.strip_prefix("::").unwrap_or(path);
    !path.is_empty()
        && path.split("::").all(|segment| {
            !segment.is_empty()
                && (matches!(segment, "crate" | "self" | "super")
                    || (segment.as_bytes()[0].is_ascii_alphabetic()
                        || segment.as_bytes()[0] == b'_')
                        && segment
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'))
        })
}

fn install_dependencies<'a>(
    path: &str,
    sources: &BTreeMap<String, LockedPackageBindingSource<'a>>,
    locked_packages: &BTreeMap<&str, &crate::LockedPackage>,
    installed: &mut BTreeSet<String>,
    catalog: &mut StartupCatalog,
    foreign_types: &mut Vec<crate::CheckedNativeType>,
    install_current: bool,
) -> Result<(), LockedRustBindingGenerationError> {
    if installed.contains(path) {
        return Ok(());
    }
    let package = locked_packages
        .get(path)
        .ok_or_else(|| LockedRustBindingGenerationError::MissingLockedSource(path.into()))?;
    for requirement in &package.requirements {
        install_dependencies(
            &requirement.path,
            sources,
            locked_packages,
            installed,
            catalog,
            foreign_types,
            true,
        )?;
    }
    if install_current {
        let source = sources
            .get(path)
            .ok_or_else(|| LockedRustBindingGenerationError::MissingLockedSource(path.into()))?;
        let shipped = PackageExportCatalog::from_bundle(
            source.bundle,
            source.manifest_source,
            source.manifest,
            source.member_sources,
        )
        .map_err(LockedRustBindingGenerationError::Bundle)?
        .install_shipped_types(catalog)
        .map_err(LockedRustBindingGenerationError::NativeType)?;
        foreign_types.extend(shipped);
    }
    installed.insert(path.into());
    Ok(())
}
