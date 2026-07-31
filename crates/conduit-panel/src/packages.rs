use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::{InterfaceClaim, ModuleGraph, PackageImportSelection, Panel, SourceSpan};

/// Single current pre-release contract-package draft marker.
pub const CONTRACT_PACKAGE_DRAFT: u16 = 0;
/// Maximum packages or artifacts in one explicit lock closure.
pub const MAXIMUM_CONTRACT_PACKAGES: usize = 256;
/// Maximum bytes in one supplied immutable contract-package artifact.
pub const MAXIMUM_CONTRACT_PACKAGE_BYTES: usize = 4 * 1024 * 1024;
/// Maximum aggregate artifact bytes admitted by one resolution.
pub const MAXIMUM_CONTRACT_PACKAGE_CLOSURE_BYTES: usize = 16 * 1024 * 1024;
/// Maximum exports retained by one contract package.
pub const MAXIMUM_CONTRACT_PACKAGE_EXPORTS: usize = 4_096;
/// Maximum transitive dependency pins retained by one contract package.
pub const MAXIMUM_CONTRACT_PACKAGE_DEPENDENCIES: usize = 256;

/// Semantic export kind. This is contract metadata, never an implementation kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContractExportKind {
    Type,
    Node,
    Composite,
    Interface,
    Adapter,
}

/// One dependency that must already be present in the supplied lock and artifact set.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPackageDependency {
    pub package_id: String,
    pub artifact_digest: String,
}

/// One exported immutable semantic descriptor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPackageExport {
    pub name: String,
    pub canonical_id: String,
    pub kind: ContractExportKind,
    pub descriptor_hash: String,
    /// Complete semantic descriptor body supplied to the checker. It contains
    /// contract facts only, never implementation/acquisition instructions.
    pub descriptor: serde_json::Value,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub structural_facets: Vec<String>,
    #[serde(default)]
    pub directional_obligations: Vec<String>,
    #[serde(default)]
    pub conformance_fixtures: Vec<String>,
    #[serde(default)]
    pub lessons: Vec<String>,
    #[serde(default)]
    pub successor: Option<String>,
    #[serde(default)]
    pub deprecated: bool,
}

/// Immutable semantic package manifest carried inside a supplied artifact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPackageManifest {
    pub schema: String,
    pub draft: u16,
    pub package_id: String,
    pub owner: String,
    pub provenance: String,
    pub license: String,
    #[serde(default)]
    pub dependencies: Vec<ContractPackageDependency>,
    pub exports: Vec<ContractPackageExport>,
}

/// Exact export pin stored in checked lock data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LockedExport {
    pub name: String,
    pub canonical_id: String,
    pub descriptor_hash: String,
}

/// One exact package artifact pin.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LockedContractPackage {
    pub package_id: String,
    pub artifact_digest: String,
    pub source: String,
    pub provenance_policy: String,
    pub exports: Vec<LockedExport>,
}

/// Checked input to import resolution. It contains no location or fetch instruction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPackageLock {
    pub schema: String,
    pub draft: u16,
    pub packages: Vec<LockedContractPackage>,
}

/// Caller-supplied immutable bytes. `mirror` is provenance-only and is never
/// opened or dereferenced by the resolver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContractPackageArtifact<'a> {
    pub bytes: &'a [u8],
    pub mirror: Option<&'a str>,
}

/// One validated package and its exact byte identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedContractPackage {
    pub package_id: String,
    pub artifact_digest: String,
    pub mirror: Option<String>,
    pub manifest: ContractPackageManifest,
}

/// One source name bound to an immutable public semantic descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageImportBinding {
    pub local_name: String,
    pub package_id: String,
    pub canonical_id: String,
    pub descriptor_hash: String,
    pub kind: ContractExportKind,
    pub source_span: SourceSpan,
}

/// Effect-free resolution output. The rewritten panel retains import
/// declarations while node/interface references use canonical identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageImportResolution {
    authored_panel: Panel,
    panel: Panel,
    packages: Vec<ResolvedContractPackage>,
    bindings: Vec<PackageImportBinding>,
}

/// One package binding attributed to its exact source module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModulePackageImportBinding {
    pub module_uri: String,
    pub binding: PackageImportBinding,
}

/// A local-module closure after every package import was resolved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageResolvedModuleGraph {
    pub graph: ModuleGraph,
    pub bindings: Vec<ModulePackageImportBinding>,
}

impl PackageImportResolution {
    /// Authored AST before local import names are resolved.
    #[must_use]
    pub const fn authored_panel(&self) -> &Panel {
        &self.authored_panel
    }

    /// Semantic AST with imported uses rewritten to canonical identities.
    #[must_use]
    pub const fn panel(&self) -> &Panel {
        &self.panel
    }

    /// Exact validated package artifacts.
    #[must_use]
    pub fn packages(&self) -> &[ResolvedContractPackage] {
        &self.packages
    }

    /// Checked alias-to-descriptor bindings.
    #[must_use]
    pub fn bindings(&self) -> &[PackageImportBinding] {
        &self.bindings
    }
}

/// Deterministic package/import failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageResolutionError {
    pub code: &'static str,
    pub source_span: Option<SourceSpan>,
    pub message: String,
}

impl std::fmt::Display for PackageResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for PackageResolutionError {}

/// Reusable resolver over caller-owned immutable inputs.
pub struct ContractPackageResolver<'a> {
    lock: &'a ContractPackageLock,
    artifacts: &'a [ContractPackageArtifact<'a>],
}

impl<'a> ContractPackageResolver<'a> {
    #[must_use]
    pub const fn new(
        lock: &'a ContractPackageLock,
        artifacts: &'a [ContractPackageArtifact<'a>],
    ) -> Self {
        Self { lock, artifacts }
    }

    pub fn resolve(
        &self,
        panel: &Panel,
    ) -> Result<PackageImportResolution, PackageResolutionError> {
        resolve_package_imports(panel, self.lock, self.artifacts)
    }
}

/// Resolves package imports using only supplied lock data and immutable bytes.
///
/// This function has no loader, network, filesystem, process, authority,
/// enrollment, grant, prompt, installation, or provider-registry capability.
pub fn resolve_package_imports(
    panel: &Panel,
    lock: &ContractPackageLock,
    artifacts: &[ContractPackageArtifact<'_>],
) -> Result<PackageImportResolution, PackageResolutionError> {
    let closure_bytes = artifacts.iter().try_fold(0_usize, |total, artifact| {
        total.checked_add(artifact.bytes.len())
    });
    if lock.packages.len() > MAXIMUM_CONTRACT_PACKAGES
        || artifacts.len() > MAXIMUM_CONTRACT_PACKAGES
        || artifacts
            .iter()
            .any(|artifact| artifact.bytes.len() > MAXIMUM_CONTRACT_PACKAGE_BYTES)
        || closure_bytes.is_none_or(|total| total > MAXIMUM_CONTRACT_PACKAGE_CLOSURE_BYTES)
    {
        return Err(error(
            "CND-IPK-008",
            None,
            "contract-package input exceeds a finite resolver bound",
        ));
    }
    validate_lock(lock)?;
    let mut resolved = BTreeMap::new();
    for locked in &lock.packages {
        let mut matches = artifacts.iter().filter_map(|artifact| {
            let digest = digest(artifact.bytes);
            (digest == locked.artifact_digest).then_some((*artifact, digest))
        });
        let Some((artifact, artifact_digest)) = matches.next() else {
            return Err(error(
                "CND-IPK-003",
                None,
                format!(
                    "locked package `{}` has no supplied artifact with digest `{}`",
                    locked.package_id, locked.artifact_digest
                ),
            ));
        };
        let manifest: ContractPackageManifest =
            serde_json::from_slice(artifact.bytes).map_err(|failure| {
                error(
                    "CND-IPK-001",
                    None,
                    format!(
                        "package `{}` artifact is not a current manifest: {failure}",
                        locked.package_id
                    ),
                )
            })?;
        validate_manifest(&manifest, locked)?;
        resolved.insert(
            locked.package_id.as_str(),
            ResolvedContractPackage {
                package_id: locked.package_id.clone(),
                artifact_digest,
                mirror: artifact.mirror.map(str::to_owned),
                manifest,
            },
        );
    }
    for package in resolved.values() {
        for dependency in &package.manifest.dependencies {
            let Some(found) = resolved.get(dependency.package_id.as_str()) else {
                return Err(error(
                    "CND-IPK-004",
                    None,
                    format!(
                        "package `{}` requires missing transitive package `{}`",
                        package.package_id, dependency.package_id
                    ),
                ));
            };
            if found.artifact_digest != dependency.artifact_digest {
                return Err(error(
                    "CND-IPK-005",
                    None,
                    format!(
                        "package `{}` pins descriptor package `{}` at `{}`, found `{}`",
                        package.package_id,
                        dependency.package_id,
                        dependency.artifact_digest,
                        found.artifact_digest
                    ),
                ));
            }
        }
    }

    let mut bindings = Vec::new();
    let mut local_names = BTreeSet::new();
    for import in &panel.package_imports {
        match &import.selection {
            PackageImportSelection::Named(names) => {
                let package = resolved.get(import.target.as_str()).ok_or_else(|| {
                    error(
                        "CND-IPK-004",
                        Some(import.source_span),
                        format!(
                            "package `{}` is absent from supplied lock data",
                            import.target
                        ),
                    )
                })?;
                for name in names {
                    bind_export(
                        package,
                        &name.export,
                        &name.local,
                        name.source_span,
                        &mut local_names,
                        &mut bindings,
                    )?;
                }
            }
            PackageImportSelection::Alias { local, source_span } => {
                let exact_package = resolved.get(import.target.as_str());
                let split_export =
                    import
                        .target
                        .rsplit_once('/')
                        .and_then(|(package_id, export)| {
                            resolved.get(package_id).map(|package| (package, export))
                        });
                match (exact_package, split_export) {
                    (Some(_), Some(_)) => {
                        return Err(error(
                            "CND-IPK-002",
                            Some(import.source_span),
                            format!(
                                "import target `{}` is both a package and an export path",
                                import.target
                            ),
                        ));
                    }
                    (Some(package), None) => {
                        for export in package.manifest.exports.iter().filter(|item| item.public) {
                            bind_export(
                                package,
                                &export.name,
                                &format!("{local}.{}", export.name),
                                *source_span,
                                &mut local_names,
                                &mut bindings,
                            )?;
                        }
                    }
                    (None, Some((package, export))) => bind_export(
                        package,
                        export,
                        local,
                        *source_span,
                        &mut local_names,
                        &mut bindings,
                    )?,
                    (None, None) => {
                        return Err(error(
                            "CND-IPK-004",
                            Some(import.source_span),
                            format!(
                                "import target `{}` is absent from supplied lock data",
                                import.target
                            ),
                        ));
                    }
                }
            }
        }
    }

    let binding_map = bindings
        .iter()
        .map(|binding| (binding.local_name.as_str(), binding))
        .collect::<BTreeMap<_, _>>();
    reject_local_declaration_collisions(panel, &bindings)?;
    let mut rewritten = panel.clone();
    rewrite_nodes(&mut rewritten.nodes, &binding_map)?;
    for definition in &mut rewritten.definitions {
        rewrite_nodes(&mut definition.nodes, &binding_map)?;
        rewrite_claims(&mut definition.implements, &binding_map)?;
        rewrite_type_references(&mut definition.parameters, &binding_map)?;
    }
    for node in &mut rewritten.nodes {
        rewrite_claims(&mut node.implements, &binding_map)?;
    }

    Ok(PackageImportResolution {
        authored_panel: panel.clone(),
        panel: rewritten,
        packages: resolved.into_values().collect(),
        bindings,
    })
}

fn reject_local_declaration_collisions(
    panel: &Panel,
    bindings: &[PackageImportBinding],
) -> Result<(), PackageResolutionError> {
    for binding in bindings {
        let collision = match binding.kind {
            ContractExportKind::Node
            | ContractExportKind::Composite
            | ContractExportKind::Adapter => panel
                .definitions
                .iter()
                .any(|definition| definition.id == binding.local_name),
            ContractExportKind::Interface => panel
                .interfaces
                .iter()
                .any(|interface| interface.id == binding.local_name),
            ContractExportKind::Type => false,
        };
        if collision {
            return Err(error(
                "CND-IPK-002",
                Some(binding.source_span),
                format!(
                    "import name `{}` collides with a local declaration",
                    binding.local_name
                ),
            ));
        }
    }
    Ok(())
}

/// Resolves package imports in every module of an already explicit local
/// module closure. No new module or package source can be loaded.
pub fn resolve_module_package_imports(
    graph: &ModuleGraph,
    lock: &ContractPackageLock,
    artifacts: &[ContractPackageArtifact<'_>],
) -> Result<PackageResolvedModuleGraph, PackageResolutionError> {
    let mut rewritten = graph.clone();
    let mut bindings = Vec::new();
    for module in &mut rewritten.modules {
        let resolution = resolve_package_imports(&module.panel, lock, artifacts)?;
        bindings.extend(resolution.bindings().iter().cloned().map(|binding| {
            ModulePackageImportBinding {
                module_uri: module.canonical_uri.clone(),
                binding,
            }
        }));
        module.panel = resolution.panel().clone();
    }
    Ok(PackageResolvedModuleGraph {
        graph: rewritten,
        bindings,
    })
}

fn validate_lock(lock: &ContractPackageLock) -> Result<(), PackageResolutionError> {
    if lock.schema != "conduit.contract-package-lock" || lock.draft != CONTRACT_PACKAGE_DRAFT {
        return Err(error(
            "CND-IPK-001",
            None,
            "lock does not use the single current contract-package draft",
        ));
    }
    let mut packages = BTreeSet::new();
    for package in &lock.packages {
        if package.exports.len() > MAXIMUM_CONTRACT_PACKAGE_EXPORTS {
            return Err(error(
                "CND-IPK-008",
                None,
                format!(
                    "locked package `{}` exceeds the export-pin bound",
                    package.package_id
                ),
            ));
        }
        if !valid_package_id(&package.package_id) || !packages.insert(package.package_id.as_str()) {
            return Err(error(
                "CND-IPK-002",
                None,
                format!(
                    "invalid or duplicate locked package `{}`",
                    package.package_id
                ),
            ));
        }
        if !valid_hash(&package.artifact_digest)
            || package.source.trim().is_empty()
            || package.provenance_policy.trim().is_empty()
        {
            return Err(error(
                "CND-IPK-001",
                None,
                format!(
                    "locked package `{}` has incomplete exact facts",
                    package.package_id
                ),
            ));
        }
    }
    Ok(())
}

fn validate_manifest(
    manifest: &ContractPackageManifest,
    locked: &LockedContractPackage,
) -> Result<(), PackageResolutionError> {
    if manifest.schema != "conduit.contract-package"
        || manifest.draft != CONTRACT_PACKAGE_DRAFT
        || manifest.package_id != locked.package_id
        || !valid_package_id(&manifest.package_id)
        || manifest.owner.trim().is_empty()
        || manifest.provenance.trim().is_empty()
        || manifest.license.trim().is_empty()
        || manifest.exports.len() > MAXIMUM_CONTRACT_PACKAGE_EXPORTS
        || manifest.dependencies.len() > MAXIMUM_CONTRACT_PACKAGE_DEPENDENCIES
    {
        return Err(error(
            "CND-IPK-001",
            None,
            format!(
                "package `{}` manifest has invalid current-draft identity or metadata",
                locked.package_id
            ),
        ));
    }
    let locked_exports = locked
        .exports
        .iter()
        .map(|export| (export.name.as_str(), export))
        .collect::<BTreeMap<_, _>>();
    if locked_exports.len() != locked.exports.len() {
        return Err(error(
            "CND-IPK-002",
            None,
            format!("package `{}` lock repeats an export", locked.package_id),
        ));
    }
    let mut seen = BTreeSet::new();
    for export in &manifest.exports {
        if !seen.insert(export.name.as_str())
            || export.name.contains('/')
            || export.name.contains('.')
            || export.name.is_empty()
            || export.canonical_id != format!("{}/{}", manifest.package_id, export.name)
            || !valid_hash(&export.descriptor_hash)
            || !valid_descriptor(export)
        {
            return Err(error(
                "CND-IPK-001",
                None,
                format!(
                    "package `{}` has invalid export `{}`",
                    manifest.package_id, export.name
                ),
            ));
        }
        let Some(pin) = locked_exports.get(export.name.as_str()) else {
            return Err(error(
                "CND-IPK-005",
                None,
                format!(
                    "package `{}` export `{}` is absent from the lock",
                    manifest.package_id, export.name
                ),
            ));
        };
        if pin.canonical_id != export.canonical_id || pin.descriptor_hash != export.descriptor_hash
        {
            return Err(error(
                "CND-IPK-005",
                None,
                format!(
                    "package `{}` export `{}` descriptor differs from the lock",
                    manifest.package_id, export.name
                ),
            ));
        }
    }
    if seen.len() != locked_exports.len() {
        return Err(error(
            "CND-IPK-005",
            None,
            format!(
                "package `{}` omits a descriptor pinned by the lock",
                manifest.package_id
            ),
        ));
    }
    Ok(())
}

fn valid_descriptor(export: &ContractPackageExport) -> bool {
    let Some(descriptor) = export.descriptor.as_object() else {
        return false;
    };
    let kind = match export.kind {
        ContractExportKind::Type => "type",
        ContractExportKind::Node => "node",
        ContractExportKind::Composite => "composite",
        ContractExportKind::Interface => "interface",
        ContractExportKind::Adapter => "adapter",
    };
    descriptor.get("id").and_then(serde_json::Value::as_str) == Some(export.canonical_id.as_str())
        && descriptor.get("kind").and_then(serde_json::Value::as_str) == Some(kind)
        && descriptor_contains_only_semantic_facts(&export.descriptor)
}

fn descriptor_contains_only_semantic_facts(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(fields) => fields.iter().all(|(name, value)| {
            !matches!(
                name.as_str(),
                "provider"
                    | "implementation"
                    | "artifact"
                    | "artifact_location"
                    | "download"
                    | "fetch"
                    | "url"
                    | "authority"
                    | "grant"
                    | "install"
                    | "enrollment"
                    | "prompt"
                    | "execute"
            ) && descriptor_contains_only_semantic_facts(value)
        }),
        serde_json::Value::Array(values) => {
            values.iter().all(descriptor_contains_only_semantic_facts)
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => true,
    }
}

fn bind_export(
    package: &ResolvedContractPackage,
    export_name: &str,
    local_name: &str,
    source_span: SourceSpan,
    local_names: &mut BTreeSet<String>,
    bindings: &mut Vec<PackageImportBinding>,
) -> Result<(), PackageResolutionError> {
    let export = package
        .manifest
        .exports
        .iter()
        .find(|candidate| candidate.name == export_name)
        .ok_or_else(|| {
            error(
                "CND-IPK-004",
                Some(source_span),
                format!(
                    "package `{}` has no export `{export_name}`",
                    package.package_id
                ),
            )
        })?;
    if !export.public {
        return Err(error(
            "CND-IPK-006",
            Some(source_span),
            format!(
                "package `{}` export `{export_name}` is private",
                package.package_id
            ),
        ));
    }
    if !local_names.insert(local_name.to_owned()) {
        return Err(error(
            "CND-IPK-002",
            Some(source_span),
            format!("duplicate or colliding import name `{local_name}`"),
        ));
    }
    bindings.push(PackageImportBinding {
        local_name: local_name.to_owned(),
        package_id: package.package_id.clone(),
        canonical_id: export.canonical_id.clone(),
        descriptor_hash: export.descriptor_hash.clone(),
        kind: export.kind,
        source_span,
    });
    Ok(())
}

fn rewrite_nodes(
    nodes: &mut [crate::Node],
    bindings: &BTreeMap<&str, &PackageImportBinding>,
) -> Result<(), PackageResolutionError> {
    for node in nodes {
        if let Some(binding) = bindings.get(node.kind.as_str()) {
            if !matches!(
                binding.kind,
                ContractExportKind::Node
                    | ContractExportKind::Composite
                    | ContractExportKind::Adapter
            ) {
                return Err(error(
                    "CND-IPK-007",
                    Some(node.source_span),
                    format!(
                        "import `{}` is not a node, composite, or adapter contract",
                        node.kind
                    ),
                ));
            }
            node.kind.clone_from(&binding.canonical_id);
        }
    }
    Ok(())
}

fn rewrite_claims(
    claims: &mut [InterfaceClaim],
    bindings: &BTreeMap<&str, &PackageImportBinding>,
) -> Result<(), PackageResolutionError> {
    for claim in claims {
        if let Some(binding) = bindings.get(claim.interface.as_str()) {
            if binding.kind != ContractExportKind::Interface {
                return Err(error(
                    "CND-IPK-007",
                    Some(claim.source_span),
                    format!("import `{}` is not an interface contract", claim.interface),
                ));
            }
            claim.interface.clone_from(&binding.canonical_id);
        }
    }
    Ok(())
}

fn rewrite_type_references(
    parameters: &mut [crate::Parameter],
    bindings: &BTreeMap<&str, &PackageImportBinding>,
) -> Result<(), PackageResolutionError> {
    for parameter in parameters {
        if let Some(binding) = bindings.get(parameter.value_type.as_str()) {
            if binding.kind != ContractExportKind::Type {
                return Err(error(
                    "CND-IPK-007",
                    Some(parameter.source_span),
                    format!("import `{}` is not a type contract", parameter.value_type),
                ));
            }
            parameter.value_type.clone_from(&binding.canonical_id);
        }
    }
    Ok(())
}

fn valid_package_id(value: &str) -> bool {
    !value.is_empty()
        && !value.contains("://")
        && !value.starts_with('/')
        && !value.ends_with('/')
        && value.contains('.')
        && value.split('/').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        })
}

fn valid_hash(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn error(
    code: &'static str,
    source_span: Option<SourceSpan>,
    message: impl Into<String>,
) -> PackageResolutionError {
    PackageResolutionError {
        code,
        source_span,
        message: message.into(),
    }
}
