//! Checked package members and exact package content identity.

use crate::prelude::*;
use crate::{
    CheckedPackageSource, FormSyntax, PackageResolutionError, PackageSyntax, SourceDocumentId,
    TypeSyntax, MAXIMUM_PACKAGE_CONTENT_BYTES, MAXIMUM_PACKAGE_MEMBERS,
};
use alloc::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageMemberSource<'a> {
    pub path: &'a str,
    pub source: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedPackageMember {
    pub path: String,
    pub source_document_id: SourceDocumentId,
    pub content_digest: [u8; 32],
    pub forms: Vec<String>,
    pub types: Vec<String>,
    pub local_requirements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedPackageBundle {
    pub package: CheckedPackageSource,
    pub members: Vec<CheckedPackageMember>,
}

/// In-memory source lookup derived from an exact checked bundle.
///
/// This is deliberately not serialized: callers must rebuild it from the
/// lossless member sources and revalidate the bundle before checking imports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageExportCatalog {
    package_content_digest: [u8; 32],
    exports: BTreeMap<String, FormSyntax>,
    type_exports: BTreeMap<String, TypeSyntax>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageBundleError {
    SourceMismatch,
    InvalidManifest(PackageResolutionError),
    MemberLimitExceeded,
    ContentLimitExceeded,
    InvalidMemberPath(String),
    DuplicateMemberPath(String),
    InvalidMemberSource(String),
    DuplicateForm(String),
    DuplicateType(String),
    MissingLocalMember(String),
    MissingLocalForm(String),
    AmbiguousLocalForm(String),
    DependencyCycle(Vec<String>),
    MissingExport(String),
    AmbiguousExport(String),
}

impl core::fmt::Display for PackageBundleError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl core::error::Error for PackageBundleError {}

impl CheckedPackageBundle {
    pub fn from_sources(
        manifest_source: &str,
        manifest: &PackageSyntax,
        member_sources: &[PackageMemberSource<'_>],
    ) -> Result<Self, PackageBundleError> {
        if member_sources.len() > MAXIMUM_PACKAGE_MEMBERS {
            return Err(PackageBundleError::MemberLimitExceeded);
        }
        let content_bytes = member_sources
            .iter()
            .try_fold(manifest_source.len(), |total, member| {
                total
                    .checked_add(member.path.len())
                    .and_then(|total| total.checked_add(member.source.len()))
            })
            .ok_or(PackageBundleError::ContentLimitExceeded)?;
        if content_bytes > MAXIMUM_PACKAGE_CONTENT_BYTES {
            return Err(PackageBundleError::ContentLimitExceeded);
        }

        let mut package = CheckedPackageSource::from_syntax(manifest_source, manifest)
            .map_err(PackageBundleError::InvalidManifest)?;
        let mut sources = member_sources.to_vec();
        sources.sort_by(|left, right| left.path.cmp(right.path));
        let mut members = Vec::with_capacity(sources.len());
        let mut member_forms = BTreeMap::<String, BTreeSet<String>>::new();
        let mut all_forms = BTreeSet::new();
        let mut all_types = BTreeSet::new();
        for member in sources {
            if !crate::surface_lex::is_operation(member.path) {
                return Err(PackageBundleError::InvalidMemberPath(member.path.into()));
            }
            if members
                .last()
                .is_some_and(|prior: &CheckedPackageMember| prior.path == member.path)
            {
                return Err(PackageBundleError::DuplicateMemberPath(member.path.into()));
            }
            let document = crate::parse_syntax_document(member.source);
            if !document.diagnostics.is_empty()
                || !document.packages.is_empty()
                || !document.constructions.is_empty()
            {
                return Err(PackageBundleError::InvalidMemberSource(member.path.into()));
            }
            let mut forms = document
                .forms
                .iter()
                .map(|form| form.name.text.clone())
                .collect::<Vec<_>>();
            forms.sort();
            for form in &forms {
                if !all_forms.insert(form.clone()) {
                    return Err(PackageBundleError::DuplicateForm(form.clone()));
                }
            }
            let mut types = document
                .types
                .iter()
                .map(|value_type| value_type.name.text.clone())
                .collect::<Vec<_>>();
            types.sort();
            for value_type in &types {
                if !all_types.insert(value_type.clone()) {
                    return Err(PackageBundleError::DuplicateType(value_type.clone()));
                }
            }
            member_forms.insert(member.path.into(), forms.iter().cloned().collect());
            let mut local_requirements = document
                .uses
                .iter()
                .filter_map(|declaration| declaration.path.strip_prefix("./"))
                .map(str::to_string)
                .collect::<Vec<_>>();
            local_requirements.sort();
            local_requirements.dedup();
            members.push(CheckedPackageMember {
                path: member.path.into(),
                source_document_id: SourceDocumentId::from(crate::hash_string(&format!(
                    "canonical-source:{}",
                    member.source
                ))),
                content_digest: Sha256::digest(member.source.as_bytes()).into(),
                forms,
                types,
                local_requirements,
            });
        }
        validate_local_requirements(&members, &member_forms)?;
        reject_member_cycles(&members)?;
        for export in &package.exports {
            exported_member_name(&members, export)?;
        }
        package.content_digest = bundle_digest(manifest_source, &members);
        Ok(Self { package, members })
    }

    /// Rebuilds the bundle from its lossless sources and requires exact
    /// equality. Deserialized bundle truth is not trusted on shape alone.
    pub fn validate_against(
        &self,
        manifest_source: &str,
        manifest: &PackageSyntax,
        member_sources: &[PackageMemberSource<'_>],
    ) -> Result<(), PackageBundleError> {
        let expected = Self::from_sources(manifest_source, manifest, member_sources)?;
        if self != &expected {
            return Err(PackageBundleError::SourceMismatch);
        }
        Ok(())
    }

    /// Resolves one public source path from the manifest-owned export list.
    /// The returned Form name is authored semantic identity; the package path
    /// remains source provenance and never becomes the Form's Kind identity.
    pub fn resolve_export(&self, source_path: &str) -> Option<&str> {
        let export = source_path
            .strip_prefix(&self.package.path)?
            .strip_prefix('/')?;
        if export.contains('/')
            || self
                .package
                .exports
                .binary_search_by(|candidate| candidate.as_str().cmp(export))
                .is_err()
        {
            return None;
        }
        exported_form_name(&self.members, export).ok()
    }

    /// Resolves one shipped semantic Type without folding package identity into
    /// the Type's own checked semantic identity.
    pub fn resolve_type_export(&self, source_path: &str) -> Option<&str> {
        let export = source_path
            .strip_prefix(&self.package.path)?
            .strip_prefix('/')?;
        if export.contains('/')
            || self
                .package
                .exports
                .binary_search_by(|candidate| candidate.as_str().cmp(export))
                .is_err()
        {
            return None;
        }
        exported_type_name(&self.members, export).ok()
    }

    pub fn member_owning_form(&self, form: &str) -> Option<&str> {
        self.members
            .iter()
            .find(|member| {
                member
                    .forms
                    .binary_search_by(|name| name.as_str().cmp(form))
                    .is_ok()
            })
            .map(|member| member.path.as_str())
    }

    pub fn resolve_local_requirement(&self, requirement: &str) -> Option<&str> {
        let (module, export) = requirement.rsplit_once('/')?;
        let member = self.members.iter().find(|member| member.path == module)?;
        unique_form_leaf(&member.forms, export).ok()
    }
}

impl PackageExportCatalog {
    pub fn from_bundle(
        bundle: &CheckedPackageBundle,
        manifest_source: &str,
        manifest: &PackageSyntax,
        member_sources: &[PackageMemberSource<'_>],
    ) -> Result<Self, PackageBundleError> {
        bundle.validate_against(manifest_source, manifest, member_sources)?;
        let mut forms = BTreeMap::new();
        let mut types = BTreeMap::new();
        for member in member_sources {
            let document = crate::parse_syntax_document(member.source);
            for form in document.forms {
                forms.insert(form.name.text.clone(), form);
            }
            for value_type in document.types {
                types.insert(value_type.name.text.clone(), value_type);
            }
        }
        let mut exports = BTreeMap::new();
        let mut type_exports = BTreeMap::new();
        for export in &bundle.package.exports {
            let source_path = format!("{}/{export}", bundle.package.path);
            if let Some(canonical) = bundle.resolve_export(&source_path) {
                let form = forms
                    .remove(canonical)
                    .ok_or_else(|| PackageBundleError::MissingExport(export.clone()))?;
                exports.insert(source_path, form);
            } else if let Some(canonical) = bundle.resolve_type_export(&source_path) {
                let value_type = types
                    .remove(canonical)
                    .ok_or_else(|| PackageBundleError::MissingExport(export.clone()))?;
                type_exports.insert(source_path, value_type);
            } else {
                return Err(PackageBundleError::MissingExport(export.clone()));
            }
        }
        Ok(Self {
            package_content_digest: bundle.package.content_digest,
            exports,
            type_exports,
        })
    }

    pub fn resolve(&self, source_path: &str) -> Option<&FormSyntax> {
        self.exports.get(source_path)
    }

    pub fn resolve_type(&self, source_path: &str) -> Option<&TypeSyntax> {
        self.type_exports.get(source_path)
    }

    /// Installs shipped Type paths for downstream `with ... as ...` checking.
    /// The package path is a source lookup name only; the checked Type keeps
    /// the same semantic identity it had in its defining pack.
    pub fn install_shipped_types(
        &self,
        catalog: &mut crate::StartupCatalog,
    ) -> Result<Vec<crate::CheckedNativeType>, crate::SyntaxCheckDiagnostic> {
        let declarations = self.type_exports.values().cloned().collect::<Vec<_>>();
        let (checked, _) = crate::native_type::check_native_types(&declarations, catalog)?;
        for (source_path, syntax) in &self.type_exports {
            let value_type = checked
                .iter()
                .find(|candidate| candidate.name == syntax.name.text)
                .expect("every shipped Type was checked");
            catalog
                .insert_native_type(
                    source_path.clone(),
                    value_type.value_type.clone(),
                    value_type.value_contracts.clone(),
                    value_type.invariants.clone(),
                )
                .map_err(|message| crate::SyntaxCheckDiagnostic {
                    code: "CND-FRM-058",
                    span: syntax.name.span,
                    message,
                })?;
        }
        Ok(checked)
    }

    pub fn package_content_digest(&self) -> [u8; 32] {
        self.package_content_digest
    }
}

fn exported_form_name<'a>(
    members: &'a [CheckedPackageMember],
    export: &str,
) -> Result<&'a str, PackageBundleError> {
    match unique_form_leaf(members.iter().flat_map(|member| &member.forms), export) {
        Ok(form) => Ok(form),
        Err(FormLeafError::Missing) => Err(PackageBundleError::MissingExport(export.into())),
        Err(FormLeafError::Ambiguous) => Err(PackageBundleError::AmbiguousExport(export.into())),
    }
}

fn exported_type_name<'a>(
    members: &'a [CheckedPackageMember],
    export: &str,
) -> Result<&'a str, PackageBundleError> {
    match unique_form_leaf(members.iter().flat_map(|member| &member.types), export) {
        Ok(value_type) => Ok(value_type),
        Err(FormLeafError::Missing) => Err(PackageBundleError::MissingExport(export.into())),
        Err(FormLeafError::Ambiguous) => Err(PackageBundleError::AmbiguousExport(export.into())),
    }
}

fn exported_member_name<'a>(
    members: &'a [CheckedPackageMember],
    export: &str,
) -> Result<&'a str, PackageBundleError> {
    match (
        exported_form_name(members, export),
        exported_type_name(members, export),
    ) {
        (Ok(name), Err(PackageBundleError::MissingExport(_)))
        | (Err(PackageBundleError::MissingExport(_)), Ok(name)) => Ok(name),
        (Err(PackageBundleError::MissingExport(_)), Err(PackageBundleError::MissingExport(_))) => {
            Err(PackageBundleError::MissingExport(export.into()))
        }
        _ => Err(PackageBundleError::AmbiguousExport(export.into())),
    }
}

fn validate_local_requirements(
    members: &[CheckedPackageMember],
    member_forms: &BTreeMap<String, BTreeSet<String>>,
) -> Result<(), PackageBundleError> {
    for member in members {
        for requirement in &member.local_requirements {
            let (module, form) = requirement
                .rsplit_once('/')
                .ok_or_else(|| PackageBundleError::MissingLocalForm(requirement.clone()))?;
            let forms = member_forms
                .get(module)
                .ok_or_else(|| PackageBundleError::MissingLocalMember(module.into()))?;
            unique_form_leaf(forms, form).map_err(|error| match error {
                FormLeafError::Missing => PackageBundleError::MissingLocalForm(requirement.clone()),
                FormLeafError::Ambiguous => {
                    PackageBundleError::AmbiguousLocalForm(requirement.clone())
                }
            })?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormLeafError {
    Missing,
    Ambiguous,
}

fn unique_form_leaf<'a, I>(forms: I, leaf: &str) -> Result<&'a str, FormLeafError>
where
    I: IntoIterator<Item = &'a String>,
{
    let mut matches = forms
        .into_iter()
        .filter(|form| form.rsplit('/').next().is_some_and(|name| name == leaf));
    let found = matches.next().ok_or(FormLeafError::Missing)?;
    if matches.next().is_some() {
        return Err(FormLeafError::Ambiguous);
    }
    Ok(found)
}

fn reject_member_cycles(members: &[CheckedPackageMember]) -> Result<(), PackageBundleError> {
    let by_path = members
        .iter()
        .map(|member| (member.path.as_str(), member))
        .collect::<BTreeMap<_, _>>();
    let mut complete = BTreeSet::new();
    for member in members {
        visit_member(&member.path, &by_path, &mut complete, &mut Vec::new())?;
    }
    Ok(())
}

fn visit_member(
    path: &str,
    members: &BTreeMap<&str, &CheckedPackageMember>,
    complete: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
) -> Result<(), PackageBundleError> {
    if let Some(position) = stack.iter().position(|active| active == path) {
        let mut cycle = stack[position..].to_vec();
        cycle.push(path.into());
        return Err(PackageBundleError::DependencyCycle(cycle));
    }
    if complete.contains(path) {
        return Ok(());
    }
    stack.push(path.into());
    for requirement in &members[path].local_requirements {
        let (dependency, _) = requirement
            .rsplit_once('/')
            .expect("validated local import");
        visit_member(dependency, members, complete, stack)?;
    }
    stack.pop();
    complete.insert(path.into());
    Ok(())
}

fn bundle_digest(manifest: &str, members: &[CheckedPackageMember]) -> [u8; 32] {
    let mut digest = Sha256::new();
    append_digest_part(&mut digest, b"conduit-package-content/v1");
    append_digest_part(&mut digest, manifest.as_bytes());
    for member in members {
        append_digest_part(&mut digest, member.path.as_bytes());
        append_digest_part(&mut digest, &member.content_digest);
    }
    digest.finalize().into()
}

fn append_digest_part(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}
