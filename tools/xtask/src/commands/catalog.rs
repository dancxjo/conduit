pub(crate) mod inventory;
mod observation;
pub(crate) mod prerequisites;
pub(crate) mod profiles;
pub(crate) mod recursive;
mod sound;

use std::{error::Error, fmt, path::PathBuf};

use clap::{Args, Subcommand, ValueEnum};
use conduit_core::{CapabilityOffer, HostAdvertisement};
use serde::Serialize;

use crate::cli::GlobalOpts;

const MATRIX_SCHEMA: &str = "conduit.catalog/host-kind-matrix@2";

#[derive(Args, Debug)]
pub struct CatalogArgs {
    #[command(subcommand)]
    pub command: CatalogCommand,
}

#[derive(Subcommand, Debug)]
pub enum CatalogCommand {
    /// Emit every supported Kind against every authoritative Host profile.
    Matrix {
        #[arg(long)]
        observatory_snapshot: Option<PathBuf>,
    },
    /// Emit one focused Host profile column.
    Gap {
        #[arg(long)]
        host: CatalogHost,
        #[arg(long)]
        observatory_snapshot: Option<PathBuf>,
    },
    /// Emit sound requirements against every implemented realization seam.
    Sound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CatalogHost {
    Std,
    Browser,
    Conduitos,
    Pico,
    PatchbayConstrained,
}

impl CatalogHost {
    const ALL: [Self; 5] = [
        Self::Std,
        Self::Browser,
        Self::Conduitos,
        Self::Pico,
        Self::PatchbayConstrained,
    ];
}

#[derive(Debug)]
pub struct CatalogError {
    code: &'static str,
    detail: String,
}

impl CatalogError {
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    fn encoding(error: serde_json::Error) -> Self {
        Self::new("catalog-encoding-failed", error.to_string())
    }
}

impl fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl Error for CatalogError {}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Coverage {
    Direct,
    Recursive,
    MissingImplementation,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum GapClassification {
    Implemented,
    PortableImplementationMissing,
    MissingHostOperation,
    MissingResource,
    MissingBase,
    UnsupportedOnThisMachine,
    DeliberatelyNotApplicable,
}

#[derive(Clone, Serialize)]
pub(super) struct InstalledImplementation {
    pub(super) implementation_id: String,
    pub(super) artifact_id: String,
    pub(super) execution_profile_id: String,
    pub(super) host_operation_families: Vec<String>,
    pub(super) resource_families: Vec<String>,
}

#[derive(Serialize)]
struct MatrixEntry {
    host_profile: String,
    kind_id: String,
    contract_revision: String,
    coverage: Coverage,
    classification: GapClassification,
    reason_code: Option<&'static str>,
    required_host_operations: Vec<String>,
    required_resources: Vec<String>,
    required_bases: Vec<String>,
    unsatisfied_prerequisites: Vec<String>,
    machine_specific: bool,
    realization_id: Option<String>,
    implementation: Option<InstalledImplementation>,
    recursive_implementations: Vec<InstalledImplementation>,
    current_offer: observation::CurrentOffer,
}

#[derive(Serialize)]
struct MatrixReport {
    schema: &'static str,
    coverage_vocabulary: [Coverage; 4],
    classification_vocabulary: [GapClassification; 7],
    catalog_basis: &'static str,
    catalog_inventory_schema: &'static str,
    catalog_digest_algorithm: &'static str,
    catalog_digest: String,
    catalog_entry_count: usize,
    maximum_catalog_entries: usize,
    comparison_key: &'static str,
    current_offer_basis: &'static str,
    host_profile_count: usize,
    matrix_entry_count: usize,
    entries: Vec<MatrixEntry>,
}

pub fn run(args: CatalogArgs, opts: &GlobalOpts) -> Result<(), CatalogError> {
    if opts.dry_run {
        println!("derive portable catalog and exact Host profile advertisements; emit static coverage without claiming a current Boot");
        return Ok(());
    }
    if matches!(args.command, CatalogCommand::Sound) {
        return sound::run(opts);
    }
    let (hosts, snapshot_path): (&[CatalogHost], Option<&PathBuf>) = match &args.command {
        CatalogCommand::Matrix {
            observatory_snapshot,
        } => (&CatalogHost::ALL, observatory_snapshot.as_ref()),
        CatalogCommand::Gap {
            host,
            observatory_snapshot,
        } => (std::slice::from_ref(host), observatory_snapshot.as_ref()),
        CatalogCommand::Sound => unreachable!("sound returned above"),
    };
    let snapshot = snapshot_path
        .map(|path| observation::load(path))
        .transpose()?;
    let report = build_report(hosts, snapshot.as_ref())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(CatalogError::encoding)?
    );
    Ok(())
}

fn build_report(
    hosts: &[CatalogHost],
    snapshot: Option<&conduit_observatory::ObservatorySnapshot>,
) -> Result<MatrixReport, CatalogError> {
    let inventory = inventory::derive()?;
    let advertisements = hosts
        .iter()
        .copied()
        .map(profiles::advertisement)
        .collect::<Result<Vec<_>, _>>()?;
    let recursive = recursive::derive()?;
    for advertisement in &advertisements {
        validate_catalog_revisions(advertisement, &inventory.entries)?;
    }
    let mut entries = Vec::with_capacity(inventory.entries.len() * advertisements.len());
    for advertisement in &advertisements {
        for kind in &inventory.entries {
            let capability = advertisement.capabilities.iter().find(|capability| {
                capability.kind_id.as_str() == kind.kind_id
                    && capability.kind_contract_revision.as_str() == kind.contract_revision
            });
            let back = recursive.iter().find(|back| {
                back.host_profile == advertisement.profile.as_str()
                    && back.kind_id == kind.kind_id
                    && back.contract_revision == kind.contract_revision
            });
            entries.push(matrix_entry(
                advertisement,
                kind,
                capability,
                back,
                snapshot,
            ));
        }
    }
    Ok(MatrixReport {
        schema: MATRIX_SCHEMA,
        coverage_vocabulary: [
            Coverage::Direct,
            Coverage::Recursive,
            Coverage::MissingImplementation,
            Coverage::Unsupported,
        ],
        classification_vocabulary: [
            GapClassification::Implemented,
            GapClassification::PortableImplementationMissing,
            GapClassification::MissingHostOperation,
            GapClassification::MissingResource,
            GapClassification::MissingBase,
            GapClassification::UnsupportedOnThisMachine,
            GapClassification::DeliberatelyNotApplicable,
        ],
        catalog_basis: "portable contracts + exact Host/application offers",
        catalog_inventory_schema: inventory::SCHEMA,
        catalog_digest_algorithm: "sha256-canonical-json",
        catalog_digest: inventory.digest,
        catalog_entry_count: inventory.entries.len(),
        maximum_catalog_entries: inventory::MAXIMUM_ENTRIES,
        comparison_key: "exact-kind-id+kind-contract-revision",
        current_offer_basis: if snapshot.is_some() {
            "exact validated conduit.observatory.snapshot/v2"
        } else {
            "not-observed; static profile composition is not current Host/Boot truth"
        },
        host_profile_count: advertisements.len(),
        matrix_entry_count: entries.len(),
        entries,
    })
}

fn validate_catalog_revisions(
    host: &HostAdvertisement,
    inventory: &[inventory::InventoryEntry],
) -> Result<(), CatalogError> {
    for capability in &host.capabilities {
        if let Some(kind) = inventory
            .iter()
            .find(|kind| kind.kind_id == capability.kind_id.as_str())
        {
            if kind.contract_revision != capability.kind_contract_revision.as_str() {
                return Err(CatalogError::new(
                    "installed-kind-revision-mismatch",
                    format!(
                        "profile={}, kind={}, catalog={}, installed={}",
                        host.profile.as_str(),
                        kind.kind_id,
                        kind.contract_revision,
                        capability.kind_contract_revision.as_str()
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn matrix_entry(
    host: &HostAdvertisement,
    kind: &inventory::InventoryEntry,
    capability: Option<&CapabilityOffer>,
    recursive: Option<&recursive::Coverage>,
    snapshot: Option<&conduit_observatory::ObservatorySnapshot>,
) -> MatrixEntry {
    let prerequisites =
        prerequisites::classify(host, kind, capability.is_some() || recursive.is_some());
    let implementation = capability.map(|capability| InstalledImplementation {
        implementation_id: capability
            .implementation
            .implementation_id
            .as_str()
            .to_owned(),
        artifact_id: capability.implementation.artifact_id.as_str().to_owned(),
        execution_profile_id: capability
            .implementation
            .execution_profile_id
            .as_str()
            .to_owned(),
        host_operation_families: capability
            .host_operations
            .iter()
            .map(|requirement| requirement.contract_id.as_str().to_owned())
            .collect(),
        resource_families: capability
            .resource_requirements
            .iter()
            .map(|requirement| requirement.class_id.as_str().to_owned())
            .collect(),
    });
    MatrixEntry {
        host_profile: host.profile.as_str().to_owned(),
        kind_id: kind.kind_id.clone(),
        contract_revision: kind.contract_revision.clone(),
        coverage: if capability.is_some() {
            Coverage::Direct
        } else if recursive.is_some() {
            Coverage::Recursive
        } else {
            Coverage::MissingImplementation
        },
        classification: prerequisites.classification,
        reason_code: prerequisites.reason_code,
        required_host_operations: prerequisites.required_host_operations,
        required_resources: prerequisites.required_resources,
        required_bases: prerequisites.required_bases,
        unsatisfied_prerequisites: prerequisites.unsatisfied,
        machine_specific: prerequisites.machine_specific,
        realization_id: capability
            .map(|offer| offer.capability_id.as_str().to_owned())
            .or_else(|| recursive.map(|back| back.realization_id.clone())),
        implementation,
        recursive_implementations: recursive
            .map(|back| back.leaves.clone())
            .unwrap_or_default(),
        current_offer: observation::current(host, kind, snapshot),
    }
}

#[cfg(test)]
mod tests;
