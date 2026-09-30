//! Semantic checking across one exact package's explicit source modules.

use crate::prelude::*;
use crate::{
    BackStatement, CheckedPackageBundle, CheckedSyntaxDocument, CordStage, PackageBundleError,
    PackageMemberSource, PackageSyntax, StartupCatalog, SyntaxCheckDiagnostic, SyntaxDefinitions,
    SyntaxDocument,
};
use alloc::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCheckError {
    Bundle(PackageBundleError),
    MissingMemberSource(String),
    AmbientCrossModuleReference {
        module: String,
        form: String,
    },
    Syntax {
        module: String,
        diagnostic: SyntaxCheckDiagnostic,
    },
}

impl core::fmt::Display for PackageCheckError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl core::error::Error for PackageCheckError {}

/// Checks every member as one semantic package after resolving only explicit
/// local module imports. There is no ambient package-wide Form namespace.
pub fn check_package_bundle(
    bundle: &CheckedPackageBundle,
    manifest_source: &str,
    manifest: &PackageSyntax,
    member_sources: &[PackageMemberSource<'_>],
    catalog: &StartupCatalog,
) -> Result<CheckedSyntaxDocument, PackageCheckError> {
    bundle
        .validate_against(manifest_source, manifest, member_sources)
        .map_err(PackageCheckError::Bundle)?;
    let source_by_path = member_sources
        .iter()
        .map(|member| (member.path, member.source))
        .collect::<BTreeMap<_, _>>();
    let mut documents = Vec::with_capacity(bundle.members.len());
    let mut all_forms = Vec::new();
    let mut all_types = Vec::new();
    let mut all_representations = Vec::new();
    let mut owners = BTreeMap::new();
    for member in &bundle.members {
        let source = source_by_path
            .get(member.path.as_str())
            .ok_or_else(|| PackageCheckError::MissingMemberSource(member.path.clone()))?;
        let document = crate::parse_syntax_document(source);
        for form in &document.forms {
            owners.insert(form.name.text.clone(), member.path.clone());
            all_forms.push(form.clone());
        }
        all_types.extend(document.types.iter().cloned());
        all_representations.extend(document.representations.iter().cloned());
        documents.push((member.path.clone(), document));
    }

    let (native_types, package_catalog) =
        crate::native_type::check_native_types(&all_types, catalog).map_err(|diagnostic| {
            PackageCheckError::Syntax {
                module: "<package>".into(),
                diagnostic,
            }
        })?;
    let catalog = &package_catalog;
    let representations = crate::representation::check_representations(
        &all_representations,
        &all_types,
        &native_types,
    )
    .map_err(|diagnostic| PackageCheckError::Syntax {
        module: "<package>".into(),
        diagnostic,
    })?;

    let signatures = crate::syntax_check::form_signatures(&all_forms).map_err(|diagnostic| {
        PackageCheckError::Syntax {
            module: "<package>".into(),
            diagnostic,
        }
    })?;
    let mut fronts = BTreeMap::new();
    for form in all_forms.iter().filter(|form| {
        form.front.type_parameters.is_empty() && form.front.kind_parameters.is_empty()
    }) {
        fronts.insert(
            form.name.text.clone(),
            crate::value_type::checked_front(form, catalog).map_err(|diagnostic| {
                PackageCheckError::Syntax {
                    module: owners[&form.name.text].clone(),
                    diagnostic,
                }
            })?,
        );
    }

    let mut resolved_forms = Vec::new();
    for (module, document) in &documents {
        reject_ambient_references(module, document, &owners)?;
        let source_paths = local_source_paths(bundle, module);
        let (mut resolved, _) = crate::syntax_check::resolve_use_declarations(
            document,
            catalog,
            &source_paths,
            &signatures,
            &fronts,
        )
        .map_err(|diagnostic| PackageCheckError::Syntax {
            module: module.clone(),
            diagnostic,
        })?;
        resolved_forms.append(&mut resolved);
    }

    let synthetic_source = canonical_package_source(bundle, member_sources);
    let document = SyntaxDocument::new(
        synthetic_source,
        Vec::new(),
        Vec::new(),
        false,
        SyntaxDefinitions {
            forms: resolved_forms,
            ..SyntaxDefinitions::default()
        },
        Vec::new(),
    );
    let mut checked = crate::check_syntax_document(&document, catalog).map_err(|diagnostic| {
        PackageCheckError::Syntax {
            module: "<package>".into(),
            diagnostic,
        }
    })?;
    checked.native_types = native_types;
    checked.representations = representations;
    Ok(checked)
}

fn local_source_paths(bundle: &CheckedPackageBundle, current: &str) -> BTreeMap<String, String> {
    let Some(member) = bundle.members.iter().find(|member| member.path == current) else {
        return BTreeMap::new();
    };
    member
        .local_requirements
        .iter()
        .filter_map(|requirement| {
            bundle
                .resolve_local_requirement(requirement)
                .map(|form| (format!("./{requirement}"), form.to_string()))
        })
        .collect()
}

fn reject_ambient_references(
    module: &str,
    document: &SyntaxDocument,
    owners: &BTreeMap<String, String>,
) -> Result<(), PackageCheckError> {
    let aliases = document
        .uses
        .iter()
        .map(|declaration| declaration.alias.text.as_str())
        .collect::<BTreeSet<_>>();
    for form in &document.forms {
        let bindings = form_bindings(form);
        for statement in &form.back {
            match statement {
                BackStatement::NamedGear(gear) => {
                    reject_name(module, &gear.invocation.kind.text, &aliases, owners)?
                }
                BackStatement::Cord(cord) => {
                    reject_stages(module, &cord.stages, &bindings, &aliases, owners)?
                }
                BackStatement::MatchedRoute(route) => {
                    for arm in &route.arms {
                        reject_stages(module, &arm.stages, &bindings, &aliases, owners)?;
                    }
                }
                BackStatement::Pool(pool) => {
                    reject_name(module, &pool.member_form.text, &aliases, owners)?
                }
                BackStatement::LocalValue(_) => {}
            }
        }
    }
    Ok(())
}

fn form_bindings(form: &crate::FormSyntax) -> BTreeSet<&str> {
    form.front
        .startup_parameters
        .iter()
        .map(|parameter| parameter.name.text.as_str())
        .chain(
            form.front
                .runtime_ports
                .iter()
                .map(|port| port.name.text.as_str()),
        )
        .chain(form.back.iter().filter_map(|statement| match statement {
            BackStatement::NamedGear(gear) => Some(gear.name.text.as_str()),
            BackStatement::Pool(pool) => Some(pool.name.text.as_str()),
            BackStatement::LocalValue(local) => Some(local.name.text.as_str()),
            BackStatement::Cord(_) | BackStatement::MatchedRoute(_) => None,
        }))
        .collect()
}

fn reject_stages(
    module: &str,
    stages: &[CordStage],
    bindings: &BTreeSet<&str>,
    aliases: &BTreeSet<&str>,
    owners: &BTreeMap<String, String>,
) -> Result<(), PackageCheckError> {
    for stage in stages {
        match stage {
            CordStage::Reference(reference) | CordStage::Glyph(reference) => {
                if !bindings.contains(reference.text.as_str()) {
                    reject_name(module, &reference.text, aliases, owners)?;
                }
            }
            CordStage::InlineGear(invocation) | CordStage::RelationalGear { invocation, .. } => {
                reject_name(module, &invocation.kind.text, aliases, owners)?
            }
            CordStage::RelationalGlyph { glyph, .. } => {
                reject_name(module, &glyph.text, aliases, owners)?
            }
            CordStage::TerminalProjection { .. }
            | CordStage::Cancellation { .. }
            | CordStage::When(_)
            | CordStage::Literal(_)
            | CordStage::PureExpression(_)
            | CordStage::StructuredSelector(_) => {}
        }
    }
    Ok(())
}

fn reject_name(
    module: &str,
    name: &str,
    aliases: &BTreeSet<&str>,
    owners: &BTreeMap<String, String>,
) -> Result<(), PackageCheckError> {
    if !aliases.contains(name) && owners.get(name).is_some_and(|owner| owner != module) {
        return Err(PackageCheckError::AmbientCrossModuleReference {
            module: module.into(),
            form: name.into(),
        });
    }
    Ok(())
}

fn canonical_package_source(
    bundle: &CheckedPackageBundle,
    sources: &[PackageMemberSource<'_>],
) -> String {
    let by_path = sources
        .iter()
        .map(|source| (source.path, source.source))
        .collect::<BTreeMap<_, _>>();
    let mut canonical = format!("package-content:{}\n", bundle.package.path);
    for member in &bundle.members {
        canonical.push_str(&format!(
            "member:{}:{}\n",
            member.path,
            by_path[member.path.as_str()]
        ));
    }
    canonical
}
