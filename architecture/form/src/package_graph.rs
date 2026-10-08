//! Exact lock-mediated checking across several source packages.

use crate::prelude::*;
use crate::{
    CheckedPackageBundle, CheckedSyntaxDocument, ConduitLock, PackageBundleError,
    PackageMemberSource, PackageResolutionError, PackageSyntax, StartupCatalog,
    SyntaxCheckDiagnostic, SyntaxDocument,
};
use alloc::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub struct PackageGraphSource<'a> {
    pub bundle: &'a CheckedPackageBundle,
    pub manifest_source: &'a str,
    pub manifest: &'a PackageSyntax,
    pub members: &'a [PackageMemberSource<'a>],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageGraphError {
    Lock(PackageResolutionError),
    Bundle {
        package: String,
        error: PackageBundleError,
    },
    DuplicatePackage(String),
    MissingLockedPackage(String),
    UnexpectedPackage(String),
    ConflictingSemanticForm(String),
    Syntax {
        package: String,
        module: String,
        diagnostic: SyntaxCheckDiagnostic,
    },
    AmbientCrossPackageReference {
        package: String,
        module: String,
        form: String,
    },
}

impl core::fmt::Display for PackageGraphError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl core::error::Error for PackageGraphError {}

/// Checks every source member selected by an exact lock.
///
/// Imports see the current package's explicit local modules and public exports
/// of direct requirements only. The lock does not create an ambient namespace.
pub fn check_package_graph(
    sources: &[PackageGraphSource<'_>],
    lock: &ConduitLock,
    catalog: &StartupCatalog,
) -> Result<CheckedSyntaxDocument, PackageGraphError> {
    let mut by_package = BTreeMap::new();
    for source in sources {
        source
            .bundle
            .validate_against(source.manifest_source, source.manifest, source.members)
            .map_err(|error| PackageGraphError::Bundle {
                package: source.bundle.package.path.clone(),
                error,
            })?;
        if by_package
            .insert(source.bundle.package.path.as_str(), *source)
            .is_some()
        {
            return Err(PackageGraphError::DuplicatePackage(
                source.bundle.package.path.clone(),
            ));
        }
    }
    let packages = sources
        .iter()
        .map(|source| source.bundle.package.clone())
        .collect::<Vec<_>>();
    lock.validate_against(&packages)
        .map_err(PackageGraphError::Lock)?;
    for package in &lock.packages {
        let Some(source) = by_package.get(package.path.as_str()) else {
            return Err(PackageGraphError::MissingLockedPackage(package.path.clone()));
        };
        if source.bundle.package.version != package.version
            || source.bundle.package.content_digest != package.content_digest
        {
            return Err(PackageGraphError::MissingLockedPackage(package.path.clone()));
        }
    }
    for source in sources {
        if !lock
            .packages
            .iter()
            .any(|package| package.path == source.bundle.package.path)
        {
            return Err(PackageGraphError::UnexpectedPackage(
                source.bundle.package.path.clone(),
            ));
        }
    }

    let mut documents = Vec::new();
    let mut all_forms = Vec::new();
    let mut owners = BTreeMap::new();
    for source in sources {
        let member_sources = source
            .members
            .iter()
            .map(|member| (member.path, member.source))
            .collect::<BTreeMap<_, _>>();
        for member in &source.bundle.members {
            let document = crate::parse_syntax_document(member_sources[member.path.as_str()]);
            let owner = format!("{}:{}", source.bundle.package.path, member.path);
            for form in &document.forms {
                if owners.insert(form.name.text.clone(), owner.clone()).is_some() {
                    return Err(PackageGraphError::ConflictingSemanticForm(
                        form.name.text.clone(),
                    ));
                }
                all_forms.push(form.clone());
            }
            documents.push((*source, member.path.clone(), owner, document));
        }
    }

    let signatures = crate::syntax_check::form_signatures(&all_forms).map_err(|diagnostic| {
        PackageGraphError::Syntax {
            package: "<graph>".into(),
            module: "<graph>".into(),
            diagnostic,
        }
    })?;
    let mut fronts = BTreeMap::new();
    for form in all_forms
        .iter()
        .filter(|form| form.front.type_parameters.is_empty())
    {
        fronts.insert(
            form.name.text.clone(),
            crate::value_type::checked_front(form, catalog).map_err(|diagnostic| {
                let owner = &owners[&form.name.text];
                let (package, module) = owner.split_once(':').unwrap_or((owner, "<member>"));
                PackageGraphError::Syntax {
                    package: package.into(),
                    module: module.into(),
                    diagnostic,
                }
            })?,
        );
    }

    let mut resolved_forms = Vec::new();
    for (source, module, owner, document) in &documents {
        crate::package_check::reject_ambient_references(owner, document, &owners).map_err(
            |error| match error {
                crate::PackageCheckError::AmbientCrossModuleReference { form, .. } => {
                    PackageGraphError::AmbientCrossPackageReference {
                        package: source.bundle.package.path.clone(),
                        module: module.clone(),
                        form,
                    }
                }
                _ => unreachable!("ambient reference audit has one refusal"),
            },
        )?;
        let mut source_paths = crate::package_check::local_source_paths(source.bundle, module);
        for requirement in &source.bundle.package.requirements {
            let dependency = by_package
                .get(requirement.path.as_str())
                .ok_or_else(|| PackageGraphError::MissingLockedPackage(requirement.path.clone()))?;
            for export in &dependency.bundle.package.exports {
                let public_path = format!("{}/{export}", dependency.bundle.package.path);
                let canonical = dependency
                    .bundle
                    .resolve_export(&public_path)
                    .expect("validated bundles retain declared exports");
                source_paths.insert(public_path, canonical.into());
            }
        }
        let mut resolved = crate::syntax_check::resolve_use_declarations(
            document,
            catalog,
            &source_paths,
            &signatures,
            &fronts,
        )
        .map_err(|diagnostic| PackageGraphError::Syntax {
            package: source.bundle.package.path.clone(),
            module: module.clone(),
            diagnostic,
        })?;
        resolved_forms.append(&mut resolved);
    }

    let synthetic = canonical_graph_source(lock);
    let document = SyntaxDocument::new(
        synthetic,
        Vec::new(),
        Vec::new(),
        false,
        (resolved_forms, Vec::new(), Vec::new()),
        Vec::new(),
    );
    crate::check_syntax_document(&document, catalog).map_err(|diagnostic| {
        PackageGraphError::Syntax {
            package: "<graph>".into(),
            module: "<graph>".into(),
            diagnostic,
        }
    })
}

fn canonical_graph_source(lock: &ConduitLock) -> String {
    let mut source = format!("{}\n", lock.schema);
    for package in &lock.packages {
        source.push_str(&package.path);
        source.push(':');
        for byte in package.content_digest {
            use core::fmt::Write as _;
            write!(source, "{byte:02x}").expect("writing to String cannot fail");
        }
        source.push('\n');
    }
    source
}
