pub(crate) mod inventory;
mod observation;
mod profiles;
mod recursive;
mod sound;

use std::{error::Error, fmt, path::PathBuf};

use clap::{Args, Subcommand, ValueEnum};
use conduit_core::{CapabilityOffer, HostAdvertisement};
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
    reason_code: Option<&'static str>,
    realization_id: Option<String>,
    implementation: Option<InstalledImplementation>,
    recursive_implementations: Vec<InstalledImplementation>,
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
    recursive: Option<&recursive::Coverage>,
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
        } else if recursive.is_some() {
            Coverage::Recursive
        } else {
            Coverage::MissingImplementation
        },
        reason_code: (capability.is_none() && recursive.is_none())
            .then_some("no-exact-installed-offer"),
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
mod tests {
    use super::*;

    #[test]
    fn matrix_has_one_obligation_per_profile_and_kind() {
        let report = build_report(&CatalogHost::ALL, None).unwrap();
        assert_eq!(report.host_profile_count, 5);
        assert_eq!(
            report.matrix_entry_count,
            report.catalog_entry_count * report.host_profile_count
        );
    }

    #[test]
    fn exact_profile_offers_drive_positive_cells() {
        let std = build_report(&[CatalogHost::Std], None).unwrap();
        let std_missing = std
            .entries
            .iter()
            .filter(|entry| matches!(entry.coverage, Coverage::MissingImplementation))
            .map(|entry| entry.kind_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            std_missing,
            vec![
                conduit_std_catalog::PATCHBAY_PRESENTATION_KIND,
                conduit_std_catalog::PATCHBAY_GEAR_FACE_KIND,
                conduit_std_catalog::PATCHBAY_PORT_KIND,
                conduit_std_catalog::PATCHBAY_CORD_KIND,
            ]
        );
        assert!(std.entries.iter().any(|entry| {
            entry.kind_id == conduit_std_catalog::STATE_TOGGLE_KIND
                && matches!(entry.coverage, Coverage::Direct)
        }));

        let browser = build_report(&[CatalogHost::Browser], None).unwrap();
        assert!(browser.entries.iter().any(|entry| {
            entry.kind_id == conduit_std_catalog::BOOL_PRESENTATION_KIND
                && matches!(entry.coverage, Coverage::Direct)
        }));

        let os = build_report(&[CatalogHost::Conduitos], None).unwrap();
        assert_eq!(
            os.entries
                .iter()
                .filter(|entry| matches!(entry.coverage, Coverage::Direct))
                .count(),
            21
        );
        let missing = os
            .entries
            .iter()
            .filter(|entry| matches!(entry.coverage, Coverage::MissingImplementation))
            .count();
        assert_eq!(missing, os.catalog_entry_count - 22);
        let gear_face = os
            .entries
            .iter()
            .find(|entry| entry.kind_id == conduit_std_catalog::PATCHBAY_GEAR_FACE_KIND)
            .unwrap();
        assert!(matches!(gear_face.coverage, Coverage::Recursive));
        assert!(gear_face.implementation.is_none());
        assert_eq!(gear_face.recursive_implementations.len(), 10);
    }

    #[test]
    fn profile_offer_removal_cannot_leave_a_stale_positive() {
        let inventory = inventory::derive().unwrap();
        let mut profile = profiles::advertisement(CatalogHost::Std).unwrap();
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
        let entry = matrix_entry(&profile, kind, None, None, None);
        assert!(matches!(entry.coverage, Coverage::MissingImplementation));
        assert_eq!(entry.reason_code, Some("no-exact-installed-offer"));
    }

    #[test]
    fn stale_installed_revision_is_a_drift_error() {
        let inventory = inventory::derive().unwrap();
        let mut profile = profiles::advertisement(CatalogHost::Std).unwrap();
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

    #[test]
    fn patchbay_high_level_and_subject_kinds_are_recursive_on_the_constrained_profile() {
        let report = build_report(&[CatalogHost::PatchbayConstrained], None).unwrap();
        for kind in [
            conduit_std_catalog::PATCHBAY_PRESENTATION_KIND,
            conduit_std_catalog::PATCHBAY_GEAR_FACE_KIND,
            conduit_std_catalog::PATCHBAY_PORT_KIND,
            conduit_std_catalog::PATCHBAY_CORD_KIND,
        ] {
            let entry = report
                .entries
                .iter()
                .find(|entry| entry.kind_id == kind)
                .unwrap();
            assert!(matches!(entry.coverage, Coverage::Recursive));
            assert!(entry
                .realization_id
                .as_deref()
                .unwrap()
                .starts_with("canonical-back:"));
            assert!(!entry.recursive_implementations.is_empty());
        }
    }

    #[test]
    fn terminal_graphics_manifestation_is_direct_only_where_installed() {
        let report = build_report(&CatalogHost::ALL, None).unwrap();
        let entries = report
            .entries
            .iter()
            .filter(|entry| entry.kind_id == conduit_std_catalog::GRAPHICS_PRESENTATION_KIND)
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), CatalogHost::ALL.len());
        let std_profile = profiles::advertisement(CatalogHost::Std)
            .unwrap()
            .profile
            .as_str()
            .to_owned();
        let std = entries
            .iter()
            .find(|entry| entry.host_profile == std_profile)
            .unwrap();
        assert!(matches!(std.coverage, Coverage::Direct));
        assert_eq!(
            std.implementation.as_ref().unwrap().implementation_id,
            conduit_std_catalog::GRAPHICS_PRESENTATION_IMPLEMENTATION
        );
        let conduitos_profile = profiles::advertisement(CatalogHost::Conduitos)
            .unwrap()
            .profile
            .as_str()
            .to_owned();
        let conduitos = entries
            .iter()
            .find(|entry| entry.host_profile == conduitos_profile)
            .unwrap();
        assert!(matches!(conduitos.coverage, Coverage::Direct));
        let conduitos_implementation = conduit_std_catalog::conduitos_presentation_nucleus_offers()
            .into_iter()
            .find(|offer| offer.kind_id.as_str() == conduit_std_catalog::GRAPHICS_PRESENTATION_KIND)
            .unwrap()
            .implementation
            .implementation_id;
        assert_eq!(
            conduitos.implementation.as_ref().unwrap().implementation_id,
            conduitos_implementation.as_str()
        );

        let browser_profile = profiles::advertisement(CatalogHost::Browser)
            .unwrap()
            .profile
            .as_str()
            .to_owned();
        let browser = entries
            .iter()
            .find(|entry| entry.host_profile == browser_profile)
            .unwrap();
        assert!(matches!(browser.coverage, Coverage::MissingImplementation));
    }
}
