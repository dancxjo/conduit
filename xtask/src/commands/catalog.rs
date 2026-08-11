pub(crate) mod inventory;
mod observation;

use std::{error::Error, fmt, path::PathBuf};

use clap::{Args, Subcommand, ValueEnum};
use conduit_core::{BootId, CapabilityOffer, HostAdvertisement, HostId, OfferGeneration};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
use conduitos::{
    identity::BootIdentities,
    offer::{CpuFeatures, HostOffer},
};
use serde::Serialize;

use crate::cli::GlobalOpts;

const MATRIX_SCHEMA: &str = "conduit.catalog/host-kind-matrix@1";

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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CatalogHost {
    Std,
    Browser,
    Conduitos,
    Pico,
}

impl CatalogHost {
    const ALL: [Self; 4] = [Self::Std, Self::Browser, Self::Conduitos, Self::Pico];
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

#[derive(Serialize)]
struct InstalledImplementation {
    implementation_id: String,
    artifact_id: String,
    execution_profile_id: String,
    host_operation_families: Vec<String>,
    resource_families: Vec<String>,
}

#[derive(Serialize)]
struct MatrixEntry {
    host_profile: String,
    kind_id: String,
    contract_revision: String,
    coverage: Coverage,
    reason_code: Option<&'static str>,
    realization_id: Option<String>,
    implementation: Option<InstalledImplementation>,
    current_offer: observation::CurrentOffer,
}

#[derive(Serialize)]
struct MatrixReport {
    schema: &'static str,
    coverage_vocabulary: [Coverage; 4],
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
    let (hosts, snapshot_path): (&[CatalogHost], Option<&PathBuf>) = match &args.command {
        CatalogCommand::Matrix {
            observatory_snapshot,
        } => (&CatalogHost::ALL, observatory_snapshot.as_ref()),
        CatalogCommand::Gap {
            host,
            observatory_snapshot,
        } => (std::slice::from_ref(host), observatory_snapshot.as_ref()),
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
        .map(profile_advertisement)
        .collect::<Result<Vec<_>, _>>()?;
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
            entries.push(matrix_entry(advertisement, kind, capability, snapshot));
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
        catalog_basis:
            "conduit_std_catalog::supported_nucleus_contracts()+supported_nucleus_offers()",
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
    snapshot: Option<&conduit_observatory::ObservatorySnapshot>,
) -> MatrixEntry {
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
        } else {
            Coverage::MissingImplementation
        },
        reason_code: capability.is_none().then_some("no-exact-installed-offer"),
        realization_id: capability.map(|offer| offer.capability_id.as_str().to_owned()),
        implementation,
        current_offer: observation::current(host, kind, snapshot),
    }
}

fn profile_advertisement(host: CatalogHost) -> Result<HostAdvertisement, CatalogError> {
    match host {
        CatalogHost::Std => Ok(StdHost::new_with_composition(
            StdHostConfig {
                host_id: HostId::from("catalog-std-reference"),
                boot_id: BootId::from("catalog-static-not-a-boot"),
                offer_generation: OfferGeneration(1),
            },
            StdHostComposition::reference(),
        )
        .advertisement()
        .clone()),
        CatalogHost::Browser => Ok(conduit_signal::distributed_browser_sink_advertisement()),
        CatalogHost::Pico => Ok(conduit_signal::pico_local_advertisement()),
        CatalogHost::Conduitos => conduitos_advertisement(),
    }
}

fn conduitos_advertisement() -> Result<HostAdvertisement, CatalogError> {
    let ids = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &ids,
        "catalog-static-artifact",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        256 * 1024,
    );
    offer
        .validate()
        .map_err(|error| CatalogError::new("conduitos-offer-invalid", error.as_str()))?;
    Ok(HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: HostId::from("catalog-conduitos-reference"),
        boot_id: BootId::from("catalog-static-not-a-boot"),
        offer_generation: OfferGeneration(offer.generation),
        profile: conduit_core::HostProfileId::from(offer.profile),
        resources: Vec::new(),
        planner_capabilities: Vec::new(),
        capabilities: offer
            .capabilities
            .iter()
            .map(|capability| {
                let canonical = conduit_std_catalog::supported_nucleus_offers()
                    .into_iter()
                    .find(|candidate| {
                        candidate.kind_id.as_str() == capability.kind
                            && candidate.kind_contract_revision.as_str()
                                == capability.contract_revision
                    })
                    .ok_or_else(|| {
                        CatalogError::new("conduitos-capability-not-in-catalog", capability.kind)
                    })?;
                let mut exact = canonical;
                exact.capability_id = conduit_core::CapabilityId::from(capability.implementation);
                exact.implementation.implementation_id =
                    conduit_core::ImplementationId::from(capability.implementation);
                exact.implementation.artifact_id =
                    conduit_core::ArtifactId::from(capability.artifact_build);
                Ok(exact)
            })
            .collect::<Result<Vec<_>, CatalogError>>()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_has_one_obligation_per_profile_and_kind() {
        let report = build_report(&CatalogHost::ALL, None).unwrap();
        assert_eq!(report.catalog_entry_count, 21);
        assert_eq!(report.host_profile_count, 4);
        assert_eq!(report.matrix_entry_count, 84);
    }

    #[test]
    fn exact_profile_offers_drive_positive_cells() {
        let std = build_report(&[CatalogHost::Std], None).unwrap();
        assert!(std
            .entries
            .iter()
            .all(|entry| matches!(entry.coverage, Coverage::Direct)));

        let os = build_report(&[CatalogHost::Conduitos], None).unwrap();
        assert_eq!(
            os.entries
                .iter()
                .filter(|entry| matches!(entry.coverage, Coverage::Direct))
                .count(),
            5
        );
        assert_eq!(
            os.entries
                .iter()
                .filter(|entry| matches!(entry.coverage, Coverage::MissingImplementation))
                .count(),
            16
        );
    }

    #[test]
    fn profile_offer_removal_cannot_leave_a_stale_positive() {
        let inventory = inventory::derive().unwrap();
        let mut profile = profile_advertisement(CatalogHost::Std).unwrap();
        let index = profile
            .capabilities
            .iter()
            .position(|capability| {
                inventory
                    .entries
                    .iter()
                    .any(|entry| entry.kind_id == capability.kind_id.as_str())
            })
            .unwrap();
        let removed = profile.capabilities.remove(index);
        let kind = inventory
            .entries
            .iter()
            .find(|entry| entry.kind_id == removed.kind_id.as_str())
            .unwrap();
        let entry = matrix_entry(&profile, kind, None, None);
        assert!(matches!(entry.coverage, Coverage::MissingImplementation));
        assert_eq!(entry.reason_code, Some("no-exact-installed-offer"));
    }

    #[test]
    fn stale_installed_revision_is_a_drift_error() {
        let inventory = inventory::derive().unwrap();
        let mut profile = profile_advertisement(CatalogHost::Std).unwrap();
        let capability = profile
            .capabilities
            .iter_mut()
            .find(|capability| {
                inventory
                    .entries
                    .iter()
                    .any(|entry| entry.kind_id == capability.kind_id.as_str())
            })
            .unwrap();
        capability.kind_contract_revision =
            conduit_core::KindContractRevision::from("stale/revision@0");
        let error = validate_catalog_revisions(&profile, &inventory.entries).unwrap_err();
        assert_eq!(error.code, "installed-kind-revision-mismatch");
    }

    #[test]
    fn report_vocabulary_preserves_future_truthful_states() {
        assert_eq!(
            serde_json::to_string(&Coverage::Recursive).unwrap(),
            "\"recursive\""
        );
        assert_eq!(
            serde_json::to_string(&Coverage::Unsupported).unwrap(),
            "\"unsupported\""
        );
    }
}
