//! Bounded, read-only, non-executing inspection of hosted Conduit artifacts.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use conduit_conformance::Manifest;
use conduit_core::{
    ArtifactLocationKind, ArtifactManifest, CapabilityReport, EvidencePolicy, ExecutionPlan, Id,
    ImplementationManifest, PlanValidationContext, SemanticHash, validate_artifact_manifest,
    validate_capability_report, validate_event_stream, validate_implementation_manifest,
};
use conduit_diagnostics::{OwnedDiagnostic, OwnedDiagnosticArgumentValue};
use conduit_panel::{
    LoadedModule, ModuleLoader, Panel, SourceValue, parse_document, resolve_modules,
};
use conduit_runtime::{
    LoweredConfigValue, LoweredSourceV2, OwnedEventPayload, OwnedExecutionEvent,
    decode_event_ndjson, validate_hosted_execution_plan,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

pub const INSPECTION_SCHEMA: &str = "conduit.inspection/v1";
pub const INSPECTION_SCHEMA_VERSION: u16 = 1;

/// Fixed resource ceilings applied before artifact-specific validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectLimits {
    pub max_input_bytes: u64,
    pub max_record_bytes: usize,
    pub max_records: usize,
    pub max_json_depth: usize,
    pub max_collection_items: usize,
    pub max_modules: usize,
    pub max_total_module_bytes: u64,
    pub max_total_reference_bytes: u64,
}

impl Default for InspectLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 8 * 1024 * 1024,
            max_record_bytes: 1024 * 1024,
            max_records: 4096,
            max_json_depth: 64,
            max_collection_items: 16_384,
            max_modules: 256,
            max_total_module_bytes: 32 * 1024 * 1024,
            max_total_reference_bytes: 64 * 1024 * 1024,
        }
    }
}

/// A safely recognizable artifact kind. `Auto` is a request, never a result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestedKind {
    Auto,
    Panel,
    LoweredSource,
    ExecutionPlan,
    Evidence,
    Diagnostic,
    Conformance,
}

/// Exact kind retained in one inspection result.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    PanelSource,
    LoweredSource,
    ExecutionPlan,
    ExecutionEvidence,
    StructuredDiagnostic,
    ConformanceManifest,
    ConformanceCases,
    ImplementationManifest,
    ArtifactManifest,
    CapabilityReport,
}

impl ArtifactKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PanelSource => "panel-source",
            Self::LoweredSource => "lowered-source",
            Self::ExecutionPlan => "execution-plan",
            Self::ExecutionEvidence => "execution-evidence",
            Self::StructuredDiagnostic => "structured-diagnostic",
            Self::ConformanceManifest => "conformance-manifest",
            Self::ConformanceCases => "conformance-cases",
            Self::ImplementationManifest => "implementation-manifest",
            Self::ArtifactManifest => "artifact-manifest",
            Self::CapabilityReport => "capability-report",
        }
    }
}

/// A typed reference without collapsing its identity category.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InspectionReference {
    pub category: String,
    pub value: String,
}

/// A value-safe, presentation-neutral inspection result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InspectionReport {
    pub schema: &'static str,
    pub schema_version: u16,
    pub kind: ArtifactKind,
    pub artifact_version: u32,
    pub content_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
    pub valid: bool,
    pub counts: BTreeMap<String, u64>,
    pub budgets: BTreeMap<String, u64>,
    pub references: Vec<InspectionReference>,
    pub redacted_fields: u64,
    pub notes: Vec<String>,
}

impl InspectionReport {
    /// Finite human rendering that never includes inspected value material.
    #[must_use]
    pub fn render_human(&self) -> String {
        use std::fmt::Write as _;

        let mut output = String::new();
        writeln!(
            output,
            "{} v{}: valid",
            self.kind.as_str(),
            self.artifact_version
        )
        .expect("writing to String cannot fail");
        writeln!(output, "  content {}", self.content_digest)
            .expect("writing to String cannot fail");
        if let Some(identity) = &self.identity {
            writeln!(output, "  identity {identity}").expect("writing to String cannot fail");
        }
        for (name, count) in &self.counts {
            writeln!(output, "  {name} {count}").expect("writing to String cannot fail");
        }
        for (name, amount) in &self.budgets {
            writeln!(output, "  budget {name} {amount}").expect("writing to String cannot fail");
        }
        for reference in &self.references {
            writeln!(output, "  {} {}", reference.category, reference.value)
                .expect("writing to String cannot fail");
        }
        if self.redacted_fields > 0 {
            writeln!(output, "  redacted_fields {}", self.redacted_fields)
                .expect("writing to String cannot fail");
        }
        for note in &self.notes {
            writeln!(output, "  note {note}").expect("writing to String cannot fail");
        }
        output
    }
}

/// Stable inspection rejection, suitable for conversion to a structured
/// diagnostic by any hosted presentation layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectionError {
    pub code: &'static str,
    pub message: String,
}

impl std::fmt::Display for InspectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for InspectionError {}

/// Inspect already-bounded bytes. No adapter performs I/O, execution,
/// provider discovery, dynamic loading, or mutation.
pub fn inspect_bytes(
    bytes: &[u8],
    requested: RequestedKind,
    extension_hint: Option<&str>,
    limits: InspectLimits,
) -> Result<InspectionReport, InspectionError> {
    enforce_input_bound(bytes.len(), limits)?;
    let detected = detect_kind(bytes, limits);
    let kind = match requested {
        RequestedKind::Auto => detected.clone()?,
        RequestedKind::Panel => ArtifactKind::PanelSource,
        RequestedKind::LoweredSource => ArtifactKind::LoweredSource,
        RequestedKind::ExecutionPlan => ArtifactKind::ExecutionPlan,
        RequestedKind::Evidence => ArtifactKind::ExecutionEvidence,
        RequestedKind::Diagnostic => ArtifactKind::StructuredDiagnostic,
        RequestedKind::Conformance => match detected.clone()? {
            kind @ (ArtifactKind::ConformanceManifest | ArtifactKind::ConformanceCases) => kind,
            _ => {
                return Err(failure(
                    "CND-INSP-003",
                    "explicit conformance type conflicts with the frozen input marker",
                ));
            }
        },
    };
    if requested != RequestedKind::Auto {
        if let Ok(detected) = detected {
            if detected != kind {
                return Err(failure(
                    "CND-INSP-003",
                    "explicit artifact type conflicts with the frozen input marker",
                ));
            }
        }
    }
    enforce_extension_hint(kind, extension_hint)?;
    match kind {
        ArtifactKind::PanelSource => inspect_panel_bytes(bytes, limits),
        ArtifactKind::ExecutionEvidence => inspect_evidence(bytes, limits),
        ArtifactKind::StructuredDiagnostic => inspect_diagnostic(bytes, limits),
        ArtifactKind::ConformanceManifest => inspect_conformance_manifest(bytes, limits),
        ArtifactKind::ConformanceCases => inspect_conformance_cases(bytes, limits),
        ArtifactKind::LoweredSource
        | ArtifactKind::ExecutionPlan
        | ArtifactKind::ImplementationManifest
        | ArtifactKind::ArtifactManifest
        | ArtifactKind::CapabilityReport => Err(failure(
            "CND-INSP-008",
            "this semantic kind has no frozen standalone byte encoding; use its typed inspection adapter",
        )),
    }
}

/// Inspect a local panel and its local import closure. Imports are confined to
/// the entry directory and subject to aggregate byte/module ceilings.
pub fn inspect_panel_path(
    path: &Path,
    limits: InspectLimits,
) -> Result<InspectionReport, InspectionError> {
    let canonical = path.canonicalize().map_err(|error| {
        failure(
            "CND-IO-001",
            format!("cannot resolve {}: {error}", path.display()),
        )
    })?;
    let root = canonical
        .parent()
        .ok_or_else(|| failure("CND-INSP-006", "panel path has no parent"))?
        .to_owned();
    let loader = LocalModuleLoader::new(root, limits);
    let entry = canonical.to_string_lossy();
    let graph = resolve_modules(&entry, None, &loader)
        .map_err(|error| failure(error.code, error.to_string()))?;
    let state = loader.state.borrow();
    let entry_source = graph
        .modules
        .iter()
        .find(|module| module.canonical_uri == entry)
        .ok_or_else(|| failure("CND-INSP-006", "resolved graph omitted its entry module"))?;
    let mut report = panel_report(
        &entry_source.panel,
        entry_source.source.as_bytes(),
        graph.modules.iter().map(|module| &module.panel),
    );
    report
        .counts
        .insert("modules".to_owned(), graph.modules.len() as u64);
    report
        .counts
        .insert("module_bytes".to_owned(), state.total_bytes);
    report
        .references
        .extend(graph.modules.iter().map(|module| InspectionReference {
            category: "source-module".to_owned(),
            value: format!("{}@{}", module.canonical_uri, module.content_hash),
        }));
    report
        .references
        .sort_by(|left, right| (&left.category, &left.value).cmp(&(&right.category, &right.value)));
    Ok(report)
}

/// Inspect a local conformance manifest and verify every referenced artifact
/// digest within the manifest's conformance root and aggregate byte ceiling.
pub fn inspect_conformance_manifest_path(
    path: &Path,
    limits: InspectLimits,
) -> Result<InspectionReport, InspectionError> {
    let canonical = path.canonicalize().map_err(|error| {
        failure(
            "CND-IO-001",
            format!("cannot resolve {}: {error}", path.display()),
        )
    })?;
    let bytes = read_bounded(&canonical, limits.max_input_bytes)?;
    let mut report = inspect_conformance_manifest(&bytes, limits)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|error| failure("CND-INSP-006", format!("malformed manifest: {error}")))?;
    let parent = canonical
        .parent()
        .ok_or_else(|| failure("CND-INSP-006", "manifest path has no parent"))?;
    let root = parent
        .parent()
        .unwrap_or(parent)
        .canonicalize()
        .map_err(|error| failure("CND-IO-001", error.to_string()))?;
    let mut total = 0_u64;
    let mut verified = BTreeSet::new();
    for artifact in manifest.suites.iter().flat_map(|suite| &suite.artifacts) {
        let artifact_path = parent
            .join(&artifact.path)
            .canonicalize()
            .map_err(|error| {
                failure(
                    "CND-INSP-006",
                    format!("cannot resolve conformance reference: {error}"),
                )
            })?;
        if !artifact_path.starts_with(&root) {
            return Err(failure(
                "CND-INSP-006",
                "conformance reference escapes its conformance root",
            ));
        }
        if !verified.insert(artifact_path.clone()) {
            continue;
        }
        let artifact_bytes = read_bounded(&artifact_path, limits.max_input_bytes)?;
        total = total
            .checked_add(artifact_bytes.len() as u64)
            .ok_or_else(|| failure("CND-INSP-007", "referenced byte count overflow"))?;
        if total > limits.max_total_reference_bytes {
            return Err(failure(
                "CND-INSP-007",
                "aggregate referenced byte limit exceeded",
            ));
        }
        if content_digest(&artifact_bytes) != artifact.sha256 {
            return Err(failure(
                "CND-INSP-006",
                "conformance reference digest does not match the manifest",
            ));
        }
    }
    report
        .counts
        .insert("verified_artifacts".to_owned(), verified.len() as u64);
    report.counts.insert("referenced_bytes".to_owned(), total);
    Ok(report)
}

/// Inspect a typed lowered source without defining a replacement wire format.
pub fn inspect_lowered_source(
    source: &LoweredSourceV2,
    content_digest: &str,
    limits: InspectLimits,
) -> Result<InspectionReport, InspectionError> {
    if source.schema_version != 2 || source.source_ast_schema_version != 2 {
        return Err(failure(
            "CND-LWR-011",
            "unsupported lowered/source-AST schema combination",
        ));
    }
    require_digest(content_digest, "content digest")?;
    let structural_items = [
        source.nodes.len(),
        source.cords.len(),
        source.composites.len(),
        source.composite_children.len(),
        source.exports.len(),
        source.bindings.len(),
        source.group_ports.len(),
        source.pools.len(),
        source.source_map.len(),
        source
            .source_map
            .iter()
            .map(|entry| entry.origins.len())
            .sum(),
    ]
    .into_iter()
    .try_fold(0_usize, usize::checked_add)
    .ok_or_else(|| failure("CND-INSP-007", "lowered structural item count overflow"))?;
    enforce_collection_bound(structural_items, limits, "lowered structural items")?;
    let mut counts = BTreeMap::new();
    counts.insert("nodes".to_owned(), source.nodes.len() as u64);
    counts.insert("cords".to_owned(), source.cords.len() as u64);
    counts.insert("composites".to_owned(), source.composites.len() as u64);
    counts.insert("exports".to_owned(), source.exports.len() as u64);
    counts.insert("bindings".to_owned(), source.bindings.len() as u64);
    counts.insert("group_ports".to_owned(), source.group_ports.len() as u64);
    counts.insert("pools".to_owned(), source.pools.len() as u64);
    counts.insert(
        "source_map_entries".to_owned(),
        source.source_map.len() as u64,
    );
    let mut budgets = BTreeMap::new();
    budgets.insert(
        "queue_memory_bytes".to_owned(),
        source
            .cords
            .iter()
            .try_fold(0_u64, |total, cord| {
                total.checked_add(cord.max_queued_bytes)
            })
            .ok_or_else(|| failure("CND-INSP-007", "lowered queue budget overflow"))?,
    );
    budgets.insert(
        "queue_items".to_owned(),
        source
            .cords
            .iter()
            .try_fold(0_u64, |total, cord| {
                total.checked_add(u64::from(cord.capacity_items))
            })
            .ok_or_else(|| failure("CND-INSP-007", "lowered queue item overflow"))?,
    );
    budgets.insert(
        "pool_instances".to_owned(),
        source
            .pools
            .iter()
            .try_fold(0_u64, |total, pool| {
                total.checked_add(u64::from(pool.maximum))
            })
            .ok_or_else(|| failure("CND-INSP-007", "lowered pool maximum overflow"))?,
    );
    let redacted_fields = source
        .nodes
        .iter()
        .flat_map(|node| &node.config)
        .filter(|entry| matches!(entry.value, LoweredConfigValue::SecretReference(_)))
        .count() as u64;
    let mut references = source
        .nodes
        .iter()
        .map(|node| InspectionReference {
            category: "semantic-contract".to_owned(),
            value: format!("{}@{}", node.contract_id, node.contract_hash),
        })
        .collect::<Vec<_>>();
    if let Some(root) = &source.root_selection {
        references.push(InspectionReference {
            category: "authored-source".to_owned(),
            value: root.authored_source_hash.clone(),
        });
    }
    references.extend(source.source_map.iter().flat_map(|entry| {
        entry.origins.iter().map(|origin| InspectionReference {
            category: "source-module".to_owned(),
            value: format!("{}@{}", origin.module_uri, origin.module_hash),
        })
    }));
    enforce_collection_bound(references.len(), limits, "lowered references")?;
    stable_references(&mut references);
    Ok(base_report(
        ArtifactKind::LoweredSource,
        source.schema_version.into(),
        content_digest.to_owned(),
        Some(source.semantic_hash.to_string()),
        counts,
        budgets,
        references,
        redacted_fields,
        vec!["semantic lowering is distinct from an exact execution plan".to_owned()],
    ))
}

/// Inspect a canonical implementation manifest without resolving, loading, or
/// executing any referenced artifact or backend.
pub fn inspect_implementation_manifest(
    manifest: &ImplementationManifest<'_>,
    content_digest: &str,
    limits: InspectLimits,
) -> Result<InspectionReport, InspectionError> {
    require_digest(content_digest, "content digest")?;
    let items = manifest
        .artifacts
        .len()
        .checked_add(manifest.required_interfaces.len())
        .and_then(|value| value.checked_add(manifest.provided_interfaces.len()))
        .and_then(|value| value.checked_add(manifest.required_authorities.len()))
        .and_then(|value| value.checked_add(manifest.required_effects.len()))
        .ok_or_else(|| failure("CND-INSP-007", "manifest item count overflow"))?;
    enforce_collection_bound(items, limits, "implementation manifest items")?;
    let mut scratch = vec![SemanticHash::from_bytes([0; 32]); manifest.identity_fact_count()];
    validate_implementation_manifest(manifest, &mut scratch)
        .map_err(|reason| failure(reason.code(), "invalid implementation manifest"))?;

    let mut counts = BTreeMap::new();
    counts.insert("artifacts".to_owned(), manifest.artifacts.len() as u64);
    counts.insert(
        "required_interfaces".to_owned(),
        manifest.required_interfaces.len() as u64,
    );
    counts.insert(
        "provided_interfaces".to_owned(),
        manifest.provided_interfaces.len() as u64,
    );
    counts.insert(
        "required_authorities".to_owned(),
        manifest.required_authorities.len() as u64,
    );
    counts.insert(
        "required_effects".to_owned(),
        manifest.required_effects.len() as u64,
    );
    let mut budgets = BTreeMap::new();
    budgets.insert(
        "coexistence_memory_bytes".to_owned(),
        manifest.coexistence_memory_bytes,
    );
    let mut references = vec![
        InspectionReference {
            category: "semantic-contract".to_owned(),
            value: format!(
                "{}@{}",
                manifest.semantic_contract.id, manifest.semantic_contract.semantic_hash
            ),
        },
        InspectionReference {
            category: "execution-profile".to_owned(),
            value: format!(
                "{}@{}",
                manifest.execution_profile.id, manifest.execution_profile.semantic_hash
            ),
        },
    ];
    references.extend(
        manifest
            .artifacts
            .iter()
            .map(|artifact| InspectionReference {
                category: "artifact".to_owned(),
                value: format!("{}@{}", artifact.id, artifact.digest),
            }),
    );
    references.extend(
        manifest
            .required_interfaces
            .iter()
            .map(|interface| InspectionReference {
                category: "required-interface".to_owned(),
                value: format!(
                    "{}@{}",
                    interface.interface.id, interface.interface.semantic_hash
                ),
            }),
    );
    references.extend(
        manifest
            .provided_interfaces
            .iter()
            .map(|interface| InspectionReference {
                category: "provided-interface".to_owned(),
                value: format!(
                    "{}@{}",
                    interface.interface.id, interface.interface.semantic_hash
                ),
            }),
    );
    stable_references(&mut references);
    Ok(base_report(
        ArtifactKind::ImplementationManifest,
        manifest.schema_version,
        content_digest.to_owned(),
        Some(manifest.identity.to_string()),
        counts,
        budgets,
        references,
        0,
        vec![
            format!("executor {}", manifest.executor.as_str()),
            "inspection does not resolve or execute the implementation".to_owned(),
        ],
    ))
}

/// Inspect provenance, licensing, signatures, and transitive references
/// without fetching or executing artifact bytes.
pub fn inspect_artifact_manifest(
    manifest: &ArtifactManifest<'_>,
    content_digest: &str,
    limits: InspectLimits,
) -> Result<InspectionReport, InspectionError> {
    require_digest(content_digest, "content digest")?;
    let items = manifest
        .identity_fact_count()
        .checked_add(manifest.locations.len())
        .ok_or_else(|| failure("CND-INSP-007", "artifact manifest item count overflow"))?;
    enforce_collection_bound(items, limits, "artifact manifest items")?;
    let mut scratch = vec![SemanticHash::from_bytes([0; 32]); manifest.identity_fact_count()];
    validate_artifact_manifest(manifest, &mut scratch)
        .map_err(|reason| failure(reason.code(), "invalid artifact manifest"))?;

    let mut counts = BTreeMap::new();
    counts.insert("signatures".to_owned(), manifest.signatures.len() as u64);
    counts.insert(
        "license_expressions".to_owned(),
        manifest.license_expressions.len() as u64,
    );
    counts.insert("notices".to_owned(), manifest.notices.len() as u64);
    counts.insert(
        "related_artifacts".to_owned(),
        manifest.related_artifacts.len() as u64,
    );
    counts.insert("locations".to_owned(), manifest.locations.len() as u64);
    let mut budgets = BTreeMap::new();
    budgets.insert("artifact_bytes".to_owned(), manifest.byte_size);
    let mut references = vec![InspectionReference {
        category: "provenance-builder".to_owned(),
        value: manifest.provenance.builder.to_string(),
    }];
    references.extend(
        manifest
            .license_expressions
            .iter()
            .map(|license| InspectionReference {
                category: "license".to_owned(),
                value: (*license).to_owned(),
            }),
    );
    references.extend(
        manifest
            .signatures
            .iter()
            .map(|signature| InspectionReference {
                category: "signature-signer".to_owned(),
                value: signature.signer.to_string(),
            }),
    );
    references.extend(manifest.locations.iter().map(|location| {
        InspectionReference {
            category: match location.kind {
                ArtifactLocationKind::BundlePath => "bundle-location",
                ArtifactLocationKind::RemoteUri => "remote-location",
            }
            .to_owned(),
            value: location.locator.to_owned(),
        }
    }));
    for (category, reference) in manifest
        .notices
        .iter()
        .map(|value| ("notice", value))
        .chain(manifest.sbom.iter().map(|value| ("sbom", value)))
        .chain(manifest.source.iter().map(|value| ("source", value)))
        .chain(
            manifest
                .related_artifacts
                .iter()
                .map(|value| ("related-artifact", value)),
        )
    {
        references.push(InspectionReference {
            category: category.to_owned(),
            value: format!("{}@{}", reference.id, reference.digest),
        });
    }
    stable_references(&mut references);
    Ok(base_report(
        ArtifactKind::ArtifactManifest,
        manifest.schema_version,
        content_digest.to_owned(),
        Some(manifest.identity.to_string()),
        counts,
        budgets,
        references,
        0,
        vec![
            format!("media_type {}", manifest.media_type),
            "locations are non-identity retrieval hints; bytes remain digest-gated".to_owned(),
            "inspection performs no fetch, load, signature verification, or execution".to_owned(),
        ],
    ))
}

/// Inspect a fresh host report without probing, discovering, configuring, or
/// otherwise mutating the host.
pub fn inspect_capability_report(
    report: &CapabilityReport<'_>,
    content_digest: &str,
    limits: InspectLimits,
) -> Result<InspectionReport, InspectionError> {
    require_digest(content_digest, "content digest")?;
    let items = report.identity_fact_count();
    enforce_collection_bound(items, limits, "capability report items")?;
    let mut scratch = vec![SemanticHash::from_bytes([0; 32]); items];
    validate_capability_report(
        report,
        report.time_basis,
        report.observed_at_tick,
        report.minimum_plan_version,
        &mut scratch,
    )
    .map_err(|reason| failure(reason.code(), "invalid capability report"))?;

    let mut counts = BTreeMap::new();
    counts.insert("capabilities".to_owned(), report.capabilities.len() as u64);
    counts.insert("resources".to_owned(), report.resources.len() as u64);
    counts.insert("topology".to_owned(), report.topology.len() as u64);
    counts.insert(
        "supported_executors".to_owned(),
        report.supported_executors.len() as u64,
    );
    counts.insert(
        "current_constraints".to_owned(),
        report.current_constraints.len() as u64,
    );
    counts.insert(
        "membership_bindings".to_owned(),
        u64::from(report.membership.is_some()),
    );
    let mut budgets = BTreeMap::new();
    budgets.insert(
        "available_memory_bytes".to_owned(),
        report.available.memory_bytes,
    );
    budgets.insert(
        "available_storage_bytes".to_owned(),
        report.available.storage_bytes,
    );
    budgets.insert(
        "available_cpu_units".to_owned(),
        u64::from(report.available.cpu_units),
    );
    budgets.insert(
        "available_transports".to_owned(),
        u64::from(report.available.transports),
    );
    let mut references = vec![
        InspectionReference {
            category: "reporter".to_owned(),
            value: format!("{}@{}", report.reporter.id, report.reporter.semantic_hash),
        },
        InspectionReference {
            category: "report-trust".to_owned(),
            value: format!("{}@{}", report.trust.id, report.trust.semantic_hash),
        },
    ];
    if let Some(membership) = report.membership {
        references.extend([
            InspectionReference {
                category: "realm".to_owned(),
                value: membership.realm.to_string(),
            },
            InspectionReference {
                category: "entity".to_owned(),
                value: membership.entity.to_string(),
            },
            InspectionReference {
                category: "passport-identity".to_owned(),
                value: membership.passport.to_string(),
            },
            InspectionReference {
                category: "passport-status-reporter".to_owned(),
                value: format!(
                    "{}@{}",
                    membership.status.reporter.id, membership.status.reporter.semantic_hash
                ),
            },
        ]);
    }
    references.extend(
        report
            .capabilities
            .iter()
            .map(|capability| InspectionReference {
                category: "host-capability".to_owned(),
                value: format!(
                    "{}@{}:{}:{}",
                    capability.interface.id,
                    capability.interface.semantic_hash,
                    capability.mode,
                    capability.subject
                ),
            }),
    );
    references.extend(report.resources.iter().map(|resource| InspectionReference {
        category: "host-resource".to_owned(),
        value: format!("{}:{}", resource.resource.kind, resource.resource.id),
    }));
    references.extend(report.topology.iter().map(|topology| InspectionReference {
        category: "host-topology".to_owned(),
        value: format!("{}:{}->{}", topology.id, topology.from, topology.to),
    }));
    stable_references(&mut references);
    Ok(base_report(
        ArtifactKind::CapabilityReport,
        report.schema_version,
        content_digest.to_owned(),
        Some(report.identity.to_string()),
        counts,
        budgets,
        references,
        0,
        vec![
            format!("host {}", report.host),
            format!(
                "freshness {}:{}..={}",
                report.time_basis, report.observed_at_tick, report.valid_until_tick
            ),
            "inspection does not refresh, discover, configure, or provision the host".to_owned(),
        ],
    ))
}

/// Inspect and validate an already-decoded exact plan without executing or
/// loading any selected artifact.
pub fn inspect_execution_plan(
    plan: &ExecutionPlan<'_>,
    context: PlanValidationContext<'_>,
    content_digest: &str,
    limits: InspectLimits,
) -> Result<InspectionReport, InspectionError> {
    require_digest(content_digest, "content digest")?;
    let mut structural_items = [
        plan.host_observations.len(),
        plan.resources.len(),
        plan.artifacts.len(),
        plan.nodes.len(),
        plan.cords.len(),
        plan.fanouts.len(),
        plan.merges.len(),
        plan.event_streams.len(),
        plan.jobs.len(),
        plan.satisfaction_proofs.len(),
        plan.authorities.len(),
        plan.composites.len(),
        plan.port_groups.len(),
        plan.instance_pools.len(),
        plan.unresolved.len(),
    ]
    .into_iter()
    .try_fold(0_usize, usize::checked_add)
    .ok_or_else(|| failure("CND-INSP-007", "plan structural item count overflow"))?;
    for node in plan.nodes {
        structural_items = structural_items
            .checked_add(node.required_resources.len())
            .and_then(|value| value.checked_add(node.required_effects.len()))
            .and_then(|value| {
                node.execution_profile.map_or(Some(value), |profile| {
                    value
                        .checked_add(profile.representations.len())
                        .and_then(|sum| sum.checked_add(profile.memory_claims.len()))
                })
            })
            .ok_or_else(|| failure("CND-INSP-007", "plan structural item count overflow"))?;
    }
    for composite in plan.composites {
        structural_items = structural_items
            .checked_add(composite.members.len())
            .and_then(|value| value.checked_add(composite.exports.len()))
            .ok_or_else(|| failure("CND-INSP-007", "plan structural item count overflow"))?;
    }
    for fanout in plan.fanouts {
        structural_items = structural_items
            .checked_add(fanout.branches.len())
            .ok_or_else(|| failure("CND-INSP-007", "plan structural item count overflow"))?;
    }
    for merge in plan.merges {
        structural_items = structural_items
            .checked_add(merge.inputs.len())
            .ok_or_else(|| failure("CND-INSP-007", "plan structural item count overflow"))?;
    }
    for group in plan.port_groups {
        structural_items = structural_items
            .checked_add(group.members.len())
            .ok_or_else(|| failure("CND-INSP-007", "plan structural item count overflow"))?;
    }
    for pool in plan.instance_pools {
        structural_items = structural_items
            .checked_add(pool.authority_grants.len())
            .ok_or_else(|| failure("CND-INSP-007", "plan structural item count overflow"))?;
    }
    for binding in plan.satisfaction_proofs {
        structural_items = structural_items
            .checked_add(binding.proof.facets.len())
            .and_then(|value| value.checked_add(binding.proof.obligations.len()))
            .ok_or_else(|| failure("CND-INSP-007", "plan structural item count overflow"))?;
    }
    enforce_collection_bound(structural_items, limits, "plan structural items")?;
    validate_hosted_execution_plan(plan, context)
        .map_err(|error| failure(error.code.as_str(), error.to_string()))?;
    let mut counts = BTreeMap::new();
    counts.insert(
        "host_observations".to_owned(),
        plan.host_observations.len() as u64,
    );
    counts.insert("resources".to_owned(), plan.resources.len() as u64);
    counts.insert("artifacts".to_owned(), plan.artifacts.len() as u64);
    counts.insert("nodes".to_owned(), plan.nodes.len() as u64);
    counts.insert(
        "execution_profiles".to_owned(),
        plan.nodes
            .iter()
            .filter(|node| node.execution_profile.is_some())
            .count() as u64,
    );
    counts.insert("cords".to_owned(), plan.cords.len() as u64);
    counts.insert("fanouts".to_owned(), plan.fanouts.len() as u64);
    counts.insert("merges".to_owned(), plan.merges.len() as u64);
    counts.insert("event_streams".to_owned(), plan.event_streams.len() as u64);
    counts.insert("jobs".to_owned(), plan.jobs.len() as u64);
    counts.insert(
        "satisfaction_proofs".to_owned(),
        plan.satisfaction_proofs.len() as u64,
    );
    counts.insert("authorities".to_owned(), plan.authorities.len() as u64);
    counts.insert("composites".to_owned(), plan.composites.len() as u64);
    counts.insert("port_groups".to_owned(), plan.port_groups.len() as u64);
    counts.insert(
        "instance_pools".to_owned(),
        plan.instance_pools.len() as u64,
    );
    let mut budgets = BTreeMap::new();
    budgets.insert("memory_bytes".to_owned(), plan.budget.memory_bytes);
    budgets.insert("storage_bytes".to_owned(), plan.budget.storage_bytes);
    budgets.insert("cpu_units".to_owned(), u64::from(plan.budget.cpu_units));
    budgets.insert("timers".to_owned(), u64::from(plan.budget.timers));
    budgets.insert("transports".to_owned(), u64::from(plan.budget.transports));
    budgets.insert("checkpoints".to_owned(), u64::from(plan.budget.checkpoints));
    budgets.insert("evidence_bytes".to_owned(), plan.budget.evidence_bytes);
    budgets.insert(
        "implementation_memory_bytes".to_owned(),
        plan.nodes
            .iter()
            .filter_map(|node| node.execution_profile)
            .try_fold(0_u64, |total, profile| {
                total.checked_add(profile.limits.implementation_memory_bytes)
            })
            .ok_or_else(|| {
                failure(
                    "CND-INSP-007",
                    "implementation profile memory budget overflow",
                )
            })?,
    );
    let mut references = vec![
        InspectionReference {
            category: "source-semantic".to_owned(),
            value: plan.source_semantic_hash.to_string(),
        },
        InspectionReference {
            category: "resolver".to_owned(),
            value: format!("{}@{}", plan.resolver.id, plan.resolver.semantic_hash),
        },
    ];
    references.extend(
        plan.host_observations
            .iter()
            .map(|value| InspectionReference {
                category: "host-observation".to_owned(),
                value: format!("{}@{}", value.id, value.semantic_hash),
            }),
    );
    references.extend(plan.artifacts.iter().map(|value| InspectionReference {
        category: "artifact".to_owned(),
        value: format!("{}@{}", value.id, value.digest),
    }));
    for node in plan.nodes {
        references.extend([
            InspectionReference {
                category: "semantic-contract".to_owned(),
                value: format!("{}@{}", node.contract.id, node.contract.semantic_hash),
            },
            InspectionReference {
                category: "implementation".to_owned(),
                value: format!(
                    "{}@{}",
                    node.implementation.id, node.implementation.semantic_hash
                ),
            },
            InspectionReference {
                category: "lifecycle-policy".to_owned(),
                value: format!(
                    "{}@{}",
                    node.lifecycle_policy.id, node.lifecycle_policy.semantic_hash
                ),
            },
        ]);
        if let Some(profile) = node.execution_profile {
            references.push(InspectionReference {
                category: "execution-profile".to_owned(),
                value: format!("{}@{}", profile.id, profile.semantic_hash),
            });
        }
    }
    for job in plan.jobs {
        references.push(InspectionReference {
            category: "job-contract".to_owned(),
            value: format!(
                "{}@{}",
                job.contract.id,
                job.contract
                    .semantic_hash()
                    .map_err(|_| failure("CND-JOB-006", "invalid job contract identity"))?
            ),
        });
        if let Some(provider) = job.contract.checkpoint_provider {
            references.push(InspectionReference {
                category: "checkpoint-provider".to_owned(),
                value: format!("{}@{}", provider.id, provider.semantic_hash),
            });
        }
    }
    references.extend(
        plan.satisfaction_proofs
            .iter()
            .map(|binding| InspectionReference {
                category: "satisfaction-proof".to_owned(),
                value: binding.proof.identity.to_string(),
            }),
    );
    references.extend(plan.resources.iter().map(|value| InspectionReference {
        category: "resource".to_owned(),
        value: format!(
            "{}:{}:{}@{}",
            value.id, value.resource.kind, value.resource.id, value.host_observation
        ),
    }));
    references.extend(plan.authorities.iter().map(|value| InspectionReference {
        category: "authority".to_owned(),
        value: format!("{}@{}", value.grant.id, value.grant_hash),
    }));
    enforce_collection_bound(references.len(), limits, "plan references")?;
    stable_references(&mut references);
    Ok(base_report(
        ArtifactKind::ExecutionPlan,
        plan.schema_version,
        content_digest.to_owned(),
        Some(plan.identity.to_string()),
        counts,
        budgets,
        references,
        0,
        vec!["validation confirms structure and pins, not artifact executability".to_owned()],
    ))
}

fn inspect_panel_bytes(
    bytes: &[u8],
    _limits: InspectLimits,
) -> Result<InspectionReport, InspectionError> {
    let source = std::str::from_utf8(bytes)
        .map_err(|_| failure("CND-INSP-006", "panel source is not UTF-8"))?;
    let document = parse_document(source);
    let panel = document
        .panel()
        .map_err(|error| failure(error.code, error.to_string()))?;
    let mut report = panel_report(panel, bytes, std::iter::once(panel));
    report
        .counts
        .insert("cst_tokens".to_owned(), document.tokens.len() as u64);
    Ok(report)
}

fn panel_report<'a>(
    entry: &Panel,
    entry_bytes: &[u8],
    panels: impl IntoIterator<Item = &'a Panel>,
) -> InspectionReport {
    let mut counts = BTreeMap::new();
    let mut references = Vec::new();
    let mut redacted_fields = 0_u64;
    let mut unresolved = 0_u64;
    let mut modules = 0_u64;
    for panel in panels {
        modules += 1;
        add_count(&mut counts, "definitions", panel.definitions.len());
        add_count(&mut counts, "nodes", panel.nodes.len());
        add_count(&mut counts, "cords", panel.cords.len());
        add_count(&mut counts, "roots", panel.roots.len());
        add_count(&mut counts, "port_groups", panel.port_groups.len());
        add_count(&mut counts, "pools", panel.pools.len());
        for import in &panel.imports {
            references.push(InspectionReference {
                category: "source-import".to_owned(),
                value: match &import.content_hash {
                    Some(hash) => format!("{}@{hash}", import.target),
                    None => import.target.clone(),
                },
            });
        }
        for node in panel.nodes.iter().chain(
            panel
                .definitions
                .iter()
                .flat_map(|definition| &definition.nodes),
        ) {
            unresolved += u64::from(node.constraint.is_some());
            redacted_fields += node
                .config
                .iter()
                .map(|entry| count_secrets(&entry.value))
                .sum::<u64>();
            references.push(InspectionReference {
                category: "semantic-contract".to_owned(),
                value: node.kind.clone(),
            });
        }
    }
    counts.insert("modules".to_owned(), modules);
    counts.insert("unresolved_selectors".to_owned(), unresolved);
    stable_references(&mut references);
    let mut notes = vec!["source identity is distinct from lowering and plan identity".to_owned()];
    if unresolved > 0 {
        notes.push("unresolved selectors were reported without provider resolution".to_owned());
    }
    base_report(
        ArtifactKind::PanelSource,
        entry.version.into(),
        content_digest(entry_bytes),
        Some(conduit_panel::semantic_source_hash_v2(entry)),
        counts,
        BTreeMap::new(),
        references,
        redacted_fields,
        notes,
    )
}

fn inspect_diagnostic(
    bytes: &[u8],
    limits: InspectLimits,
) -> Result<InspectionReport, InspectionError> {
    let value = parse_json(bytes, limits)?;
    let diagnostic: OwnedDiagnostic = serde_json::from_value(value)
        .map_err(|error| failure("CND-INSP-006", format!("malformed diagnostic: {error}")))?;
    diagnostic
        .validate()
        .map_err(|error| failure("CND-INSP-006", format!("invalid diagnostic: {error:?}")))?;
    let redacted_fields = diagnostic
        .arguments
        .iter()
        .filter(|argument| {
            matches!(
                argument.value,
                OwnedDiagnosticArgumentValue::Redacted { .. }
            )
        })
        .count() as u64;
    let mut counts = BTreeMap::new();
    counts.insert("related".to_owned(), diagnostic.related.len() as u64);
    counts.insert("arguments".to_owned(), diagnostic.arguments.len() as u64);
    counts.insert("fixes".to_owned(), diagnostic.fixes.len() as u64);
    counts.insert(
        "edits".to_owned(),
        diagnostic
            .fixes
            .iter()
            .map(|fix| fix.edits.len() as u64)
            .sum(),
    );
    let mut references = Vec::new();
    if let Some(primary) = &diagnostic.primary {
        references.push(InspectionReference {
            category: "source-document".to_owned(),
            value: primary.document_id.clone(),
        });
    }
    if let Some(path) = &diagnostic.semantic_path {
        references.push(InspectionReference {
            category: "semantic-path".to_owned(),
            value: path.clone(),
        });
    }
    Ok(base_report(
        ArtifactKind::StructuredDiagnostic,
        diagnostic.schema_version,
        content_digest(bytes),
        None,
        counts,
        BTreeMap::new(),
        references,
        redacted_fields,
        vec!["diagnostic message and argument value material are not reproduced".to_owned()],
    ))
}

fn inspect_evidence(
    bytes: &[u8],
    limits: InspectLimits,
) -> Result<InspectionReport, InspectionError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| failure("CND-INSP-006", "evidence stream is not UTF-8"))?;
    let lines = bounded_lines(text, limits)?;
    let events =
        decode_event_ndjson(text).map_err(|error| failure("CND-INSP-006", error.to_string()))?;
    if events.len() != lines {
        return Err(failure(
            "CND-INSP-006",
            "evidence record count changed during decoding",
        ));
    }
    validate_owned_event_stream(&events, limits)?;
    let first = events
        .first()
        .ok_or_else(|| failure("CND-INSP-006", "evidence stream is empty"))?;
    let redacted_fields = events
        .iter()
        .filter(|event| {
            matches!(
                event.payload,
                OwnedEventPayload::Reference { .. } | OwnedEventPayload::Redacted { .. }
            )
        })
        .count() as u64;
    let mut counts = BTreeMap::new();
    counts.insert("records".to_owned(), events.len() as u64);
    counts.insert(
        "terminal_records".to_owned(),
        events
            .iter()
            .filter(|event| {
                matches!(
                    event.terminality,
                    conduit_runtime::OwnedEventTerminality::Terminal { .. }
                )
            })
            .count() as u64,
    );
    let references = vec![
        InspectionReference {
            category: "run".to_owned(),
            value: first.run_id.clone(),
        },
        InspectionReference {
            category: "execution-plan".to_owned(),
            value: first.plan_identity.clone(),
        },
    ];
    Ok(base_report(
        ArtifactKind::ExecutionEvidence,
        first.schema_version,
        content_digest(bytes),
        None,
        counts,
        BTreeMap::new(),
        references,
        redacted_fields,
        vec!["payload material is not reproduced by inspection".to_owned()],
    ))
}

fn validate_owned_event_stream(
    events: &[OwnedExecutionEvent],
    limits: InspectLimits,
) -> Result<(), InspectionError> {
    let mut scratches = events
        .iter()
        .map(|event| vec![Id(""); event.relations.derived_from.len()])
        .collect::<Vec<_>>();
    let mut borrowed = Vec::with_capacity(events.len());
    for (event, scratch) in events.iter().zip(&mut scratches) {
        borrowed.push(
            event
                .as_event(scratch)
                .map_err(|error| failure("CND-INSP-006", error.to_string()))?,
        );
    }
    validate_event_stream(
        &borrowed,
        EvidencePolicy {
            max_inline_payload_bytes: limits.max_record_bytes.try_into().unwrap_or(u32::MAX),
            reveal_redacted_byte_length: true,
            reveal_redacted_item_count: true,
        },
    )
    .map_err(|error| failure(error.reason.code(), error.to_string()))
}

fn inspect_conformance_manifest(
    bytes: &[u8],
    limits: InspectLimits,
) -> Result<InspectionReport, InspectionError> {
    let value = parse_json(bytes, limits)?;
    let manifest: Manifest = serde_json::from_value(value)
        .map_err(|error| failure("CND-INSP-006", format!("malformed manifest: {error}")))?;
    if manifest.fixture_version != "conduit.conformance/v1"
        || manifest.manifest_revision == 0
        || manifest.protocol_version != 1
    {
        return Err(failure(
            "CND-INSP-004",
            "unsupported conformance fixture or protocol version",
        ));
    }
    if manifest.suites.len() > limits.max_collection_items {
        return Err(failure("CND-INSP-007", "conformance suite limit exceeded"));
    }
    let mut suite_ids = BTreeSet::new();
    let mut artifact_ids = BTreeSet::new();
    let mut artifact_count = 0_u64;
    let mut references = Vec::new();
    for suite in &manifest.suites {
        if !suite_ids.insert(&suite.id) {
            return Err(failure("CND-INSP-006", "duplicate conformance suite id"));
        }
        for artifact in &suite.artifacts {
            artifact_count += 1;
            if !artifact_ids.insert(&artifact.id) {
                return Err(failure("CND-INSP-006", "duplicate conformance artifact id"));
            }
            require_digest(&artifact.sha256, "conformance artifact digest")?;
            references.push(InspectionReference {
                category: "conformance-artifact".to_owned(),
                value: format!("{}@{}", artifact.path, artifact.sha256),
            });
        }
    }
    if usize::try_from(artifact_count).unwrap_or(usize::MAX) > limits.max_collection_items {
        return Err(failure(
            "CND-INSP-007",
            "conformance artifact limit exceeded",
        ));
    }
    stable_references(&mut references);
    let mut counts = BTreeMap::new();
    counts.insert("suites".to_owned(), manifest.suites.len() as u64);
    counts.insert("artifacts".to_owned(), artifact_count);
    Ok(base_report(
        ArtifactKind::ConformanceManifest,
        manifest.manifest_revision,
        content_digest(bytes),
        None,
        counts,
        BTreeMap::new(),
        references,
        0,
        vec!["referenced artifacts are reported without executing reference tests".to_owned()],
    ))
}

fn inspect_conformance_cases(
    bytes: &[u8],
    limits: InspectLimits,
) -> Result<InspectionReport, InspectionError> {
    if bytes.starts_with(b"# case\t") || bytes.starts_with(b"case\t") {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| failure("CND-INSP-006", "conformance table is not UTF-8"))?;
        let lines = bounded_lines(text, limits)?;
        let mut counts = BTreeMap::new();
        counts.insert("cases".to_owned(), lines.saturating_sub(1) as u64);
        return Ok(base_report(
            ArtifactKind::ConformanceCases,
            1,
            content_digest(bytes),
            None,
            counts,
            BTreeMap::new(),
            Vec::new(),
            0,
            vec!["tabular conformance cases are data, not executable tests".to_owned()],
        ));
    }
    let value = parse_json(bytes, limits)?;
    let object = value
        .as_object()
        .ok_or_else(|| failure("CND-INSP-006", "conformance fixture is not a JSON object"))?;
    let suite = object
        .get("suite")
        .and_then(Value::as_str)
        .ok_or_else(|| failure("CND-INSP-006", "conformance fixture has no suite marker"))?;
    let mut cases = 0_usize;
    for (name, value) in object {
        if name == "suite" || name == "measurement" {
            continue;
        }
        if let Some(items) = value.as_array() {
            cases = cases
                .checked_add(items.len())
                .ok_or_else(|| failure("CND-INSP-007", "conformance case count overflow"))?;
        }
    }
    if cases > limits.max_collection_items {
        return Err(failure("CND-INSP-007", "conformance case limit exceeded"));
    }
    let mut counts = BTreeMap::new();
    counts.insert("cases".to_owned(), cases as u64);
    Ok(base_report(
        ArtifactKind::ConformanceCases,
        schema_suffix_version(suite)?,
        content_digest(bytes),
        None,
        counts,
        BTreeMap::new(),
        vec![InspectionReference {
            category: "conformance-suite".to_owned(),
            value: suite.to_owned(),
        }],
        0,
        vec!["conformance cases are inspected without invoking a runner".to_owned()],
    ))
}

fn detect_kind(bytes: &[u8], limits: InspectLimits) -> Result<ArtifactKind, InspectionError> {
    if bytes.is_empty() {
        return Err(failure(
            "CND-INSP-001",
            "empty input has no artifact marker",
        ));
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        let trimmed = text.trim_start_matches('\u{feff}').trim_start();
        if panel_marker(trimmed) {
            return Ok(ArtifactKind::PanelSource);
        }
        if bytes.starts_with(b"# case\t") || bytes.starts_with(b"case\t") {
            return Ok(ArtifactKind::ConformanceCases);
        }
        if trimmed.starts_with('{') {
            preflight_json(trimmed.as_bytes(), limits)?;
            match serde_json::from_str::<Value>(trimmed) {
                Ok(value) => {
                    enforce_json_limits(&value, limits)?;
                    return detect_json_object(&value);
                }
                Err(error) if error.classify() == serde_json::error::Category::Syntax => {
                    if let Some(first) = trimmed.lines().next() {
                        if let Ok(value) = serde_json::from_str::<Value>(first) {
                            if is_evidence_marker(&value) {
                                return Ok(ArtifactKind::ExecutionEvidence);
                            }
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }
    Err(failure(
        "CND-INSP-001",
        "input has no supported frozen magic, schema, or version marker",
    ))
}

fn panel_marker(text: &str) -> bool {
    text.lines()
        .map(str::trim_start)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .is_some_and(|line| line.starts_with("panel "))
}

fn detect_json_object(value: &Value) -> Result<ArtifactKind, InspectionError> {
    let object = value
        .as_object()
        .ok_or_else(|| failure("CND-INSP-001", "JSON input has no object marker"))?;
    let mut candidates = Vec::new();
    if object.contains_key("fixture_version")
        && object.contains_key("manifest_revision")
        && object.contains_key("protocol_version")
    {
        candidates.push(ArtifactKind::ConformanceManifest);
    }
    if object.contains_key("suite") {
        candidates.push(ArtifactKind::ConformanceCases);
    }
    if object.contains_key("code")
        && object.contains_key("severity")
        && object.contains_key("schema_version")
    {
        candidates.push(ArtifactKind::StructuredDiagnostic);
    }
    if is_evidence_marker(value) {
        candidates.push(ArtifactKind::ExecutionEvidence);
    }
    if let Some(schema) = object.get("schema").and_then(Value::as_str) {
        match schema {
            "conduit.lowered-source/v2" => candidates.push(ArtifactKind::LoweredSource),
            "conduit.execution-plan/v1" | "conduit.execution-plan/v2" => {
                candidates.push(ArtifactKind::ExecutionPlan);
            }
            _ => {}
        }
    }
    candidates.sort_by_key(|kind| kind.as_str());
    candidates.dedup();
    match candidates.as_slice() {
        [kind] => Ok(*kind),
        [] => Err(failure(
            "CND-INSP-001",
            "JSON input has no supported frozen schema marker",
        )),
        _ => Err(failure(
            "CND-INSP-002",
            "input contains conflicting artifact markers",
        )),
    }
}

fn is_evidence_marker(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    [
        "schema_version",
        "identity",
        "event_id",
        "run_id",
        "plan_identity",
        "sequence",
        "kind",
        "payload",
    ]
    .iter()
    .all(|field| object.contains_key(*field))
}

fn parse_json(bytes: &[u8], limits: InspectLimits) -> Result<Value, InspectionError> {
    preflight_json(bytes, limits)?;
    let value = serde_json::from_slice(bytes)
        .map_err(|error| failure("CND-INSP-006", format!("malformed JSON: {error}")))?;
    enforce_json_limits(&value, limits)?;
    Ok(value)
}

fn preflight_json(bytes: &[u8], limits: InspectLimits) -> Result<(), InspectionError> {
    let mut depth = 0_usize;
    let mut items = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    for byte in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth
                    .checked_add(1)
                    .ok_or_else(|| failure("CND-INSP-007", "JSON depth overflow"))?;
                items = items
                    .checked_add(1)
                    .ok_or_else(|| failure("CND-INSP-007", "JSON item count overflow"))?;
                if depth > limits.max_json_depth {
                    return Err(failure("CND-INSP-007", "JSON nesting limit exceeded"));
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            b',' => {
                items = items
                    .checked_add(1)
                    .ok_or_else(|| failure("CND-INSP-007", "JSON item count overflow"))?;
            }
            _ => {}
        }
        if items > limits.max_collection_items {
            return Err(failure("CND-INSP-007", "JSON item limit exceeded"));
        }
    }
    Ok(())
}

fn enforce_json_limits(value: &Value, limits: InspectLimits) -> Result<(), InspectionError> {
    fn visit(
        value: &Value,
        depth: usize,
        items: &mut usize,
        limits: InspectLimits,
    ) -> Result<(), InspectionError> {
        if depth > limits.max_json_depth {
            return Err(failure("CND-INSP-007", "JSON nesting limit exceeded"));
        }
        match value {
            Value::Array(values) => {
                *items = items
                    .checked_add(values.len())
                    .ok_or_else(|| failure("CND-INSP-007", "JSON item count overflow"))?;
                for value in values {
                    visit(value, depth + 1, items, limits)?;
                }
            }
            Value::Object(values) => {
                *items = items
                    .checked_add(values.len())
                    .ok_or_else(|| failure("CND-INSP-007", "JSON item count overflow"))?;
                for value in values.values() {
                    visit(value, depth + 1, items, limits)?;
                }
            }
            _ => {}
        }
        if *items > limits.max_collection_items {
            return Err(failure("CND-INSP-007", "JSON item limit exceeded"));
        }
        Ok(())
    }
    visit(value, 1, &mut 0, limits)
}

fn bounded_lines(text: &str, limits: InspectLimits) -> Result<usize, InspectionError> {
    let mut count = 0_usize;
    for line in text.split_terminator('\n') {
        count += 1;
        if count > limits.max_records {
            return Err(failure("CND-INSP-007", "record count limit exceeded"));
        }
        if line.len() > limits.max_record_bytes {
            return Err(failure("CND-INSP-007", "record byte limit exceeded"));
        }
        if line.is_empty() {
            return Err(failure("CND-INSP-006", "empty record is not allowed"));
        }
        if line.trim_start().starts_with('{') {
            preflight_json(line.as_bytes(), limits)?;
        }
    }
    Ok(count)
}

fn enforce_extension_hint(
    kind: ArtifactKind,
    extension: Option<&str>,
) -> Result<(), InspectionError> {
    let hinted = match extension {
        Some("panel") => Some(ArtifactKind::PanelSource),
        Some("ndjson") => Some(ArtifactKind::ExecutionEvidence),
        Some("json" | "tsv") | None => None,
        Some(_) => None,
    };
    if hinted.is_some_and(|hint| hint != kind) {
        return Err(failure(
            "CND-INSP-003",
            "file extension conflicts with the detected artifact marker",
        ));
    }
    Ok(())
}

fn enforce_input_bound(length: usize, limits: InspectLimits) -> Result<(), InspectionError> {
    if u64::try_from(length).unwrap_or(u64::MAX) > limits.max_input_bytes {
        Err(failure("CND-INSP-005", "input byte limit exceeded"))
    } else {
        Ok(())
    }
}

fn enforce_collection_bound(
    count: usize,
    limits: InspectLimits,
    collection: &str,
) -> Result<(), InspectionError> {
    if count > limits.max_collection_items {
        return Err(failure(
            "CND-INSP-007",
            format!("{collection} exceed the inspection item limit"),
        ));
    }
    Ok(())
}

fn schema_suffix_version(schema: &str) -> Result<u32, InspectionError> {
    schema
        .rsplit_once("/v")
        .or_else(|| schema.rsplit_once("-v"))
        .and_then(|(_, version)| version.parse().ok())
        .ok_or_else(|| failure("CND-INSP-004", "unsupported or absent schema version"))
}

fn content_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn require_digest(value: &str, label: &str) -> Result<(), InspectionError> {
    let valid = value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()));
    if valid {
        Ok(())
    } else {
        Err(failure(
            "CND-INSP-006",
            format!("{label} is not a SHA-256 digest"),
        ))
    }
}

fn add_count(counts: &mut BTreeMap<String, u64>, name: &str, amount: usize) {
    *counts.entry(name.to_owned()).or_default() += amount as u64;
}

fn count_secrets(value: &SourceValue) -> u64 {
    match value {
        SourceValue::SecretReference(_) => 1,
        SourceValue::List(values) => values.iter().map(count_secrets).sum(),
        SourceValue::Record(fields) => fields.iter().map(|(_, value)| count_secrets(value)).sum(),
        _ => 0,
    }
}

fn stable_references(references: &mut Vec<InspectionReference>) {
    references
        .sort_by(|left, right| (&left.category, &left.value).cmp(&(&right.category, &right.value)));
    references.dedup();
}

#[allow(clippy::too_many_arguments)]
fn base_report(
    kind: ArtifactKind,
    artifact_version: u32,
    content_digest: String,
    identity: Option<String>,
    counts: BTreeMap<String, u64>,
    budgets: BTreeMap<String, u64>,
    references: Vec<InspectionReference>,
    redacted_fields: u64,
    notes: Vec<String>,
) -> InspectionReport {
    InspectionReport {
        schema: INSPECTION_SCHEMA,
        schema_version: INSPECTION_SCHEMA_VERSION,
        kind,
        artifact_version,
        content_digest,
        identity,
        valid: true,
        counts,
        budgets,
        references,
        redacted_fields,
        notes,
    }
}

fn failure(code: &'static str, message: impl Into<String>) -> InspectionError {
    InspectionError {
        code,
        message: message.into(),
    }
}

struct LocalModuleLoader {
    root: PathBuf,
    limits: InspectLimits,
    state: std::cell::RefCell<LocalLoaderState>,
}

#[derive(Default)]
struct LocalLoaderState {
    modules: usize,
    total_bytes: u64,
}

impl LocalModuleLoader {
    fn new(root: PathBuf, limits: InspectLimits) -> Self {
        Self {
            root,
            limits,
            state: std::cell::RefCell::new(LocalLoaderState::default()),
        }
    }
}

impl ModuleLoader for LocalModuleLoader {
    fn load(&self, canonical_uri: &str) -> Result<Option<LoadedModule>, String> {
        if canonical_uri.contains("://") {
            return Err("network/URI module loading is disabled during inspection".to_owned());
        }
        let path = Path::new(canonical_uri)
            .canonicalize()
            .map_err(|error| format!("cannot resolve module {canonical_uri}: {error}"))?;
        if !path.starts_with(&self.root) {
            return Err(format!(
                "module {} escapes inspection root {}",
                path.display(),
                self.root.display()
            ));
        }
        let bytes =
            read_bounded(&path, self.limits.max_input_bytes).map_err(|error| error.to_string())?;
        let mut state = self.state.borrow_mut();
        state.modules += 1;
        state.total_bytes = state
            .total_bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| "module byte count overflow".to_owned())?;
        if state.modules > self.limits.max_modules {
            return Err("module count limit exceeded".to_owned());
        }
        if state.total_bytes > self.limits.max_total_module_bytes {
            return Err("aggregate module byte limit exceeded".to_owned());
        }
        let source = String::from_utf8(bytes)
            .map_err(|_| format!("module {} is not UTF-8", path.display()))?;
        Ok(Some(LoadedModule {
            canonical_uri: path.to_string_lossy().into_owned(),
            source,
        }))
    }
}

/// Read one file without allocating beyond `maximum + 1`.
pub fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, InspectionError> {
    let mut file = File::open(path).map_err(|error| {
        failure(
            "CND-IO-001",
            format!("cannot read {}: {error}", path.display()),
        )
    })?;
    if file
        .metadata()
        .map_err(|error| failure("CND-IO-001", error.to_string()))?
        .len()
        > maximum
    {
        return Err(failure("CND-INSP-005", "input byte limit exceeded"));
    }
    read_stream_bounded(&mut file, maximum)
}

/// Read one stream without retaining more than `maximum + 1` bytes.
pub fn read_stream_bounded(
    reader: &mut impl std::io::Read,
    maximum: u64,
) -> Result<Vec<u8>, InspectionError> {
    let mut bytes = Vec::new();
    reader
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| failure("CND-IO-001", error.to_string()))?;
    if bytes.len() as u64 > maximum {
        return Err(failure("CND-INSP-005", "input byte limit exceeded"));
    }
    Ok(bytes)
}
