//! Local OS-selected File policy. Neither a control token nor a copied report
//! can select Source, authorize adoption or enlarge the original IMAGE budget.
use super::*;
use conduit_body::BodyBiographyEvidence;
use conduit_plot::SourceSnapshotRevision;
#[cfg(target_os = "linux")]
use conduit_plot::{CheckContextRevision, ExpandedAuthoringPlot};
#[cfg(target_os = "linux")]
use conduit_std_host::{
    body_execution::ImagePreparedValue, source_check::process::OwnedSourceCheckInput,
};

#[cfg(target_os = "linux")]
#[path = "source_revision_carriage.rs"]
mod carriage;
#[cfg(target_os = "linux")]
use carriage::{
    admit_selected_member, open_selected_play_carriers, return_selected_member,
    select_direct_fore_lines, submit_selected_play,
};

const SELECTION_FILE: &str = "source-check-selection.json";

#[cfg(target_os = "linux")]
#[path = "source_revision_reselect.rs"]
mod reselection;
#[cfg(target_os = "linux")]
pub(crate) use reselection::reselect;
#[cfg(not(target_os = "linux"))]
pub(crate) fn reselect(_state_dir: &Path, _request: &Path) -> Result<(), String> {
    Err("selected bounded Source compiler requires Linux".into())
}

/// Explicit operator review of one finite transition. This document is consumed
/// only by the local stopped-service entrance, never through network control.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Selection {
    host_id: String,
    configuration_id: String,
    release_bundle_sha256: String,
    expected: BodyBiographyEvidence,
    file: std::path::PathBuf,
    snapshot: SourceSnapshotRevision,
    entries: Vec<String>,
    maximum_source_bytes: usize,
    #[serde(default)]
    resident_preparation: Option<ResidentPreparationSelection>,
    #[serde(default)]
    plays: Vec<UsbPlaySelection>,
    #[serde(default)]
    continuation_outputs: Vec<super::source_continuation::OutputSelection>,
    capture_preparation_bytes: u64,
    capture_retained_bytes: u64,
    /// Full independently qualified caller/catalog/compiler/planner/journal
    /// overlap. Kept reserved without shrinking until this process releases it.
    caller_workspace_bytes: usize,
    #[serde(default)]
    returns: Vec<UsbReturnSelection>,
    #[serde(default)]
    admissions: Vec<UsbAdmissionSelection>,
    #[serde(default)]
    local_fore_adapters: Vec<super::local_fore_policy::LocalForeAdapterIntent>,
}

/// Finite attended deployment request selected by the local installer.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UsbPlaySelection {
    /// Independently installed direct USB carrier; never selected by incoming frames.
    #[cfg(target_os = "linux")]
    #[serde(default)]
    direct_fore_carrier: Option<carriage::DirectForeCarrierSelection>,
    #[serde(default)]
    carrier: conduit_std_host::pico_spawn::ProvisioningCarrier,
    port: std::path::PathBuf,
    member_basis: std::path::PathBuf,
    report_output: std::path::PathBuf,
    wait_milliseconds: u64,
    image: ImageProjectionSelection,
    maximum_program_bytes: u32,
    maximum_total_program_bytes: u32,
    assigned_encoded_bytes: u16,
    assigned_runtime_state_bytes: u16,
    assigned_counts: [u8; conduit_core::ASSIGNED_PLAN_COUNT_KINDS],
    maximum_fore_ports: usize,
    maximum_document_bytes: u32,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageProjectionSelection {
    maximum_nodes: usize,
    maximum_cords: usize,
    maximum_routes: usize,
    maximum_route_targets: usize,
    maximum_host_calls: usize,
    maximum_resources: usize,
    maximum_sign_expectations: usize,
    maximum_configuration_entries: usize,
    maximum_ports_per_node: usize,
    maximum_remote_endpoints: usize,
    maximum_cord_value_slots: u16,
    maximum_cord_value_bytes: u32,
    maximum_sign_items: u16,
    maximum_sign_bytes: u32,
}
impl ImageProjectionSelection {
    fn bounds(&self) -> conduit_embedded_build::EmbeddedImageBounds {
        conduit_embedded_build::EmbeddedImageBounds {
            maximum_nodes: self.maximum_nodes,
            maximum_cords: self.maximum_cords,
            maximum_routes: self.maximum_routes,
            maximum_route_targets: self.maximum_route_targets,
            maximum_host_calls: self.maximum_host_calls,
            maximum_resources: self.maximum_resources,
            maximum_sign_expectations: self.maximum_sign_expectations,
            maximum_configuration_entries: self.maximum_configuration_entries,
            maximum_ports_per_node: self.maximum_ports_per_node,
            maximum_remote_endpoints: self.maximum_remote_endpoints,
            maximum_cord_value_slots: self.maximum_cord_value_slots,
            maximum_cord_value_bytes: self.maximum_cord_value_bytes,
            maximum_sign_items: self.maximum_sign_items,
            maximum_sign_bytes: self.maximum_sign_bytes,
        }
    }
}

/// Local installation selection of compiler ceilings, not execution grants.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResidentPreparationSelection {
    maximum_images: usize,
    canonical_bytes: usize,
    scopes: usize,
    invariants: usize,
    contracts: usize,
    maximum_occurrences: usize,
    maximum_program_bytes: u32,
    maximum_generated_bytes: u32,
    private_work_units: u16,
}
#[cfg(target_os = "linux")]
#[path = "source_revision_residents.rs"]
mod residents;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UsbReturnSelection {
    #[serde(default)]
    carrier: conduit_std_host::pico_spawn::ProvisioningCarrier,
    /// Explicit local File authority to recover a fresh Boot from the stored key.
    /// The incoming retained credential remains a hint; strict is the default.
    #[serde(default)]
    recover_retained_key: bool,
    part_id: conduit_body::PartId,
    port: std::path::PathBuf,
    basis: std::path::PathBuf,
    receipt_output: std::path::PathBuf,
    wait_milliseconds: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UsbAdmissionSelection {
    #[serde(default)]
    carrier: conduit_std_host::pico_spawn::ProvisioningCarrier,
    host_id: conduit_core::HostId,
    member_key: AdmissionMemberKey,
    image_id: String,
    configuration_id: String,
    port: std::path::PathBuf,
    basis: std::path::PathBuf,
    invitation_output: std::path::PathBuf,
    receipt_output: std::path::PathBuf,
    return_receipt_output: std::path::PathBuf,
    return_basis_output: std::path::PathBuf,
    wait_milliseconds: u64,
}

#[cfg(target_os = "linux")]
pub(crate) struct CheckedFileRevision {
    pub(crate) source: crate::plot_source::CanonicalSource,
    pub(crate) plots: Vec<ExpandedAuthoringPlot>,
    pub(crate) resident_images: Vec<conduit_embedded_build::PreparedResidentExpression>,
}

/// Independently selected local bootstrap construction; never an incoming key.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "kebab-case", deny_unknown_fields)]
enum AdmissionMemberKey {
    Pinned { verifying_key: [u8; 32] },
    FromOwnedInvitation,
}
impl AdmissionMemberKey {
    fn valid(&self) -> bool {
        !matches!(self, Self::Pinned { verifying_key } if *verifying_key == [0; 32])
    }
    fn resolve(&self, invitation: &conduit_body::PortableInvitation) -> Result<[u8; 32], String> {
        match self {
            Self::Pinned { verifying_key } => Ok(*verifying_key),
            Self::FromOwnedInvitation => {
                conduit_body::SpawnInvitationSecret::from_csprng_bytes(invitation.secret)
                    .map(|key| key.verifying_key())
                    .map_err(|_| "owned invitation bootstrap key refused".into())
            }
        }
    }
}
#[cfg(target_os = "linux")]
impl UsbAdmissionSelection {
    fn verify_basis(
        &self,
        basis: &conduit_std_host::pico_spawn::PicoSpawnAdmissionBasis,
        peer: &conduit_body::FederationPeer,
        invitation: &conduit_body::PortableInvitation,
    ) -> Result<[u8; 32], String> {
        let key = self.member_key.resolve(invitation)?;
        if &basis.owner != peer
            || basis.expected_host != self.host_id
            || basis.member_verifying_key != key
            || basis.invitation != invitation.claim
            || basis.image_id.as_str() != self.image_id
            || basis.expected_profile.as_str() != self.configuration_id
        {
            return Err("selected admission differs from owned invitation, reviewed IMAGE, or independently selected physical Host/key construction".into());
        }
        Ok(key)
    }
}

/// Affine policy custody, constructed only after the local OS File selection,
/// original-counter preparation and exact checked entries all succeed.
#[cfg(target_os = "linux")]
pub(crate) struct AuthorizedFileRevision {
    selection: Selection,
    current_expected: BodyBiographyEvidence,
    host: conduit_core::HostId,
    boot: conduit_core::BootId,
    generation: conduit_core::OfferGeneration,
    source: ImagePreparedValue<OwnedSourceCheckInput>,
    checked: CheckedFileRevision,
    caller: ImagePreparedValue<()>,
}

/// Actual original captures and the checked workset move together. This is
/// retained planning custody after admission, never fresh Source authority.
#[cfg(target_os = "linux")]
pub(crate) struct RetainedFileWorkset {
    // Field order releases all derived backing before its original guards.
    pub(crate) checked: CheckedFileRevision,
    source: ImagePreparedValue<OwnedSourceCheckInput>,
    caller: ImagePreparedValue<()>,
}
#[cfg(target_os = "linux")]
impl RetainedFileWorkset {
    pub(crate) fn verify(
        &self,
        host: &conduit_std_host::StdHost,
        workset: &conduit_body::BodyWorkset,
    ) -> Result<(), String> {
        host.verify_selected_image_prepared_value(&self.source)?;
        host.verify_selected_image_prepared_value(&self.caller)?;
        let selected =
            conduit_body::BodyWorkset::from_plots(self.checked.plots.iter().map(|plot| {
                conduit_body::ResidentPlot::new(
                    plot.expanded.source_document_id.clone(),
                    plot.expanded.checked_plot_id.clone(),
                )
            }))
            .map_err(|e| format!("retained File workset: {e:?}"))?;
        if &selected != workset || self.checked.source.source != self.source.value().source() {
            return Err(
                "retained checked File differs from current Body workset or original capture"
                    .into(),
            );
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl AuthorizedFileRevision {
    pub(crate) fn local_fore_adapters(
        &self,
    ) -> &[super::local_fore_policy::LocalForeAdapterIntent] {
        &self.selection.local_fore_adapters
    }

    pub(crate) fn into_retained(self) -> RetainedFileWorkset {
        RetainedFileWorkset {
            source: self.source,
            caller: self.caller,
            checked: self.checked,
        }
    }
    pub(crate) fn verify<'a>(
        &'a self,
        host: &conduit_std_host::StdHost,
    ) -> Result<(&'a BodyBiographyEvidence, &'a CheckedFileRevision), String> {
        let ad = host.advertisement();
        if ad.host_id != self.host
            || ad.boot_id != self.boot
            || ad.offer_generation != self.generation
        {
            return Err("File revision belongs to another original Host/Boot/offer".into());
        }
        host.verify_selected_image_prepared_value(&self.source)?;
        host.verify_selected_image_prepared_value(&self.caller)?;
        if self.source.value().request().snapshot != self.selection.snapshot
            || self.checked.source.source != self.source.value().source()
        {
            return Err("File revision lost its exact original Source correspondence".into());
        }
        Ok((&self.current_expected, &self.checked))
    }
}

#[cfg(target_os = "linux")]
fn validate_selection(selection: &Selection, installation: &Installation) -> Result<(), String> {
    super::source_continuation::validate(&selection.continuation_outputs)?;
    let configured = installation
        .selected_host_configuration
        .as_ref()
        .ok_or("File revision requires an explicitly installed checked IMAGE")?;
    if configured.host_image().is_none()
        || installation.host_id != selection.host_id
        || configured.configuration_id() != selection.configuration_id
        || installation.release_bundle_sha256 != selection.release_bundle_sha256
        || selection.file.is_relative()
        || selection.entries.is_empty()
        || selection.entries.len() > conduit_body::MAX_BODY_PLOTS
        || selection
            .entries
            .iter()
            .any(|e| e.is_empty() || e.len() > conduit_plot::MAX_CHECK_LABEL_BYTES)
        || selection.entries.iter().collect::<BTreeSet<_>>().len() != selection.entries.len()
        || selection.plays.len() > conduit_body::MAX_BODY_PLOTS
        || selection.plays.iter().any(|p| {
            p.port.is_relative()
                || p.member_basis.is_relative()
                || p.report_output.is_relative()
                || !(1..=10_000).contains(&p.wait_milliseconds)
                || p.maximum_document_bytes == 0
                || p.maximum_document_bytes > u16::MAX.into()
                || p.maximum_fore_ports > 64
        })
        || selection.resident_preparation.as_ref().is_some_and(|p| {
            p.maximum_images == 0
                || p.maximum_images > 128
                || p.canonical_bytes == 0
                || p.scopes == 0
                || p.invariants == 0
                || p.contracts == 0
                || p.maximum_occurrences == 0
                || p.maximum_program_bytes == 0
                || p.maximum_generated_bytes == 0
                || p.private_work_units == 0
        })
        || selection.maximum_source_bytes == 0
        || selection.maximum_source_bytes > conduit_plot::MAXIMUM_PLOT_SOURCE_BYTES
        || selection.capture_preparation_bytes == 0
        || selection.capture_retained_bytes > selection.capture_preparation_bytes
        || selection.caller_workspace_bytes == 0
        || selection
            .returns
            .len()
            .saturating_add(selection.admissions.len())
            > 4
        || selection.admissions.iter().any(|member| {
            [
                &member.port,
                &member.basis,
                &member.invitation_output,
                &member.receipt_output,
                &member.return_receipt_output,
                &member.return_basis_output,
            ]
            .iter()
            .any(|path| !path.is_absolute())
                || !(1..=300_000).contains(&member.wait_milliseconds)
                || member.host_id == conduit_core::HostId::from(selection.host_id.as_str())
                || !member.member_key.valid()
                || member.image_id.is_empty()
                || member.image_id.len() > 256
                || member.configuration_id.is_empty()
                || member.configuration_id.len() > 256
                || selection.expected.membership.parts.iter().any(|part| {
                    part.current
                        .as_ref()
                        .is_some_and(|current| current.host_id == member.host_id)
                })
        })
        || selection
            .admissions
            .iter()
            .map(|member| &member.host_id)
            .collect::<BTreeSet<_>>()
            .len()
            != selection.admissions.len()
        || selection.returns.iter().any(|member| {
            !member.port.is_absolute()
                || !member.basis.is_absolute()
                || !member.receipt_output.is_absolute()
                || !(1..=300_000).contains(&member.wait_milliseconds)
                || !selection
                    .expected
                    .membership
                    .parts
                    .iter()
                    .any(|part| part.part_id == member.part_id)
        })
        || selection
            .returns
            .iter()
            .map(|member| &member.part_id)
            .collect::<BTreeSet<_>>()
            .len()
            != selection.returns.len()
        || current_body_id(installation) != Some(selection.expected.body_id.as_str())
    {
        return Err(
            "File revision selection differs from installed Root, Body or finite bounds".into(),
        );
    }
    selection
        .expected
        .validate()
        .map_err(|e| format!("File revision Body basis: {e:?}"))
}

pub(crate) fn retained(state_dir: &Path) -> Result<Option<Selection>, String> {
    let path = state_dir.join(SELECTION_FILE);
    if !path.try_exists().map_err(|e| e.to_string())? {
        return Ok(None);
    }
    let bytes = bounded_read(&path, MAXIMUM_BODY_ADMISSION_BYTES)?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| format!("selected File policy: {e}"))
}

#[cfg(target_os = "linux")]
pub(crate) struct PreparedFileSource {
    pub(crate) host: conduit_std_host::StdHost,
    pub(crate) source: ImagePreparedValue<OwnedSourceCheckInput>,
    pub(crate) caller: ImagePreparedValue<()>,
}

/// Build only the exact independently selected compiler/File scope. The caller
/// retains reviewed configuration/catalog metadata under its original Root
/// startup custody; declared limits never create additional grant capacity.
#[cfg(target_os = "linux")]
pub(super) fn prepare(
    selection: &Selection,
    installation: &Installation,
    config: conduit_std_host::StdHostConfig,
    creche: Option<conduit_core::ResourceOffer>,
) -> Result<PreparedFileSource, String> {
    use conduit::source_check_worker::*;
    use conduit_core::*;
    use conduit_std_host::{ImmediateSourcePreparationBudget, RunControl};
    use conduit_std_offers::*;
    validate_selection(selection, installation)?;
    let selected = installation
        .selected_host_configuration
        .as_ref()
        .ok_or("missing selected IMAGE")?;
    let checked_host = selected.checked()?;
    let image = selected.host_image().ok_or("missing selected IMAGE")?;
    // Actual descriptor access is supplied by the independently selected local
    // OS policy. Configuration labels and network clients cannot open it.
    let file =
        fs::File::open(&selection.file).map_err(|e| format!("open owner-selected File: {e}"))?;
    let grant = AuthorityGrant {
        grant_id: fresh_identity("grant/local-source-file", &selection.snapshot.sha256)
            .as_str()
            .into(),
        contract_id: SOURCE_CHECK_AUTHORITY.into(),
        host_call_contract_id: SOURCE_CHECK_CALL.into(),
        subject_kind: SOURCE_CHECK_KIND.into(),
        host_id: config.host_id.clone(),
        boot_id: config.boot_id.clone(),
        capability_id: SOURCE_CHECK_IMPLEMENTATION.into(),
    };
    let provider = SourceCheckProviderIdentity {
        instance: fresh_identity("base/local-source-compiler", &selection.configuration_id)
            .as_str()
            .into(),
        generation: config.offer_generation.0,
        resource_generation: ResourceGenerationId(fresh_identity(
            "resource/local-source-compiler",
            &selection.configuration_id,
        )),
    };
    let control = RunControl::default();
    let catalog = conduit_workspace_make::catalog();
    // SAFETY: the stopped local OS selection supplies the actual File and
    // original Root authority. Explicit reviewed preparation envelopes remain
    // the caller's qualification obligation; the IMAGE counter must admit them.
    let initialized = unsafe {
        initialize_image_with_caller_workspace(
            SourceCheckImageInstallation {
                config,
                image,
                checked: &checked_host,
                catalog: &catalog,
                grant,
                provider,
                file,
                maximum_source_bytes: selection.maximum_source_bytes,
                capture_budget: ImmediateSourcePreparationBudget {
                    preparation_bytes: selection
                        .capture_preparation_bytes
                        .try_into()
                        .map_err(|_| "Source capture preparation overflow")?,
                    retained_bytes: selection
                        .capture_retained_bytes
                        .try_into()
                        .map_err(|_| "Source capture retention overflow")?,
                },
                snapshot: &selection.snapshot,
                entry: &selection.entries[0],
                context: CheckContextRevision {
                    grammar_edition: "conduitese-v1".into(),
                    catalog: "conduit/product-standard@1".into(),
                    kinds_fores: "conduit/product-standard@1".into(),
                    types_laws: "conduit/product-standard@1".into(),
                    backs: "empty@1".into(),
                },
                control: &control,
            },
            creche,
            Some(selection.caller_workspace_bytes),
        )?
    };
    let host = initialized.host;
    let caller = initialized
        .caller
        .ok_or("selected Source compiler lost original caller custody")?;
    // Product catalogs are immutable OnceLock stores. Their original charge
    // must survive Host/runtime destruction and failed later preparation alike.
    // Retain the first actual reservation until process exit, not merely while
    // this particular adoption/Host happens to exist.
    static CATALOG_CUSTODY: std::sync::OnceLock<
        conduit_std_host::body_execution::ImageMemoryRetention,
    > = std::sync::OnceLock::new();
    let shared_custody = CATALOG_CUSTODY.get_or_init(|| caller.retain_memory());
    host.verify_selected_image_memory_retention(shared_custody)?;
    Ok(PreparedFileSource {
        host,
        source: initialized.source,
        caller,
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn adopt(state_dir: &Path, policy: &Path, prepare_only: bool) -> Result<(), String> {
    if prepare_only && !cfg!(all(feature = "root-allocation-observation", not(test))) {
        return Err("preparation measurement requires the explicit original-process allocator observation build".into());
    }
    let _host_lock = owner_lock::acquire(state_dir)?;
    let _body_lock = owner_lock::body(state_dir)?;
    let installation = read_installation(&state_dir.join("installation.json"))?;
    let selection: Selection =
        serde_json::from_slice(&bounded_read(policy, MAXIMUM_BODY_ADMISSION_BYTES)?)
            .map_err(|e| format!("File revision selection: {e}"))?;
    validate_selection(&selection, &installation)?;
    // Reuse the ordinary OS-protected service credential; it grants no Source
    // authority. Prepare it before the transition so the retained Root can serve.
    if !prepare_only {
        crate::durable_host_control::ensure_secret(state_dir)?;
    }
    if prepare_only {
        let binding = installation
            .body_state
            .as_ref()
            .ok_or("Source preparation requires the installed owner Body")?;
        let bytes = bounded_read(
            Path::new(&binding.biography_path),
            MAXIMUM_BODY_ADMISSION_BYTES,
        )?;
        let current: BodyBiographyEvidence = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Source preparation Body basis: {e}"))?;
        if digest(&bytes) != binding.biography_sha256 || current != selection.expected {
            return Err("Source preparation differs from exact installed owner Body basis".into());
        }
    }
    let config = conduit_std_host::StdHostConfig {
        host_id: installation.host_id.as_str().into(),
        boot_id: fresh_identity("boot/local-source-policy", &installation.host_id)
            .as_str()
            .into(),
        offer_generation: conduit_core::OfferGeneration(1),
    };
    let creche = installation
        .selected_host_configuration
        .as_ref()
        .ok_or("missing selected IMAGE")?
        .prepare_creche()?;
    let prepared = prepare(
        &selection,
        &installation,
        config,
        creche.as_ref().map(|f| f.resource_offer()),
    )?;
    let source = crate::plot_source::parse_owned(prepared.source.value().source().to_owned())?;
    let document = source.check()?;
    let plots = selection
        .entries
        .iter()
        .map(|entry| source.expand_checked_for_authoring(&document, entry))
        .collect::<Result<Vec<_>, _>>()?;
    let resident_images = selection
        .resident_preparation
        .as_ref()
        .map(|policy| policy.prepare(&document, &plots))
        .transpose()?
        .unwrap_or_default();
    // Expanded Plots and resident programs own their required immutable data;
    // the compiler's checked syntax is no longer execution-time custody.
    drop(document);
    let checked = CheckedFileRevision {
        source,
        plots,
        resident_images,
    };
    if prepare_only {
        prepared
            .host
            .verify_selected_image_prepared_value(&prepared.source)?;
        prepared
            .host
            .verify_selected_image_prepared_value(&prepared.caller)?;
        #[cfg(all(feature = "root-allocation-observation", not(test)))]
        {
            let observation = conduit_std_host::allocation_observation::snapshot();
            let within = observation.peak_requested_bytes <= selection.caller_workspace_bytes;
            write_json_atomic(
                &state_dir.join("source-policy-preparation.json"),
                &serde_json::json!({
                    "schema": "conduit.diagnostic/funded-source-preparation@1",
                    "body_id": selection.expected.body_id,
                    "host_id": prepared.host.advertisement().host_id,
                    "configuration_id": selection.configuration_id,
                    "release_bundle_sha256": selection.release_bundle_sha256,
                    "snapshot": selection.snapshot,
                    "entries": selection.entries,
                    "resident_images": checked.resident_images.len(),
                    "caller_workspace_bytes": selection.caller_workspace_bytes,
                    "capture_preparation_bytes": selection.capture_preparation_bytes,
                    "capture_retained_bytes": selection.capture_retained_bytes,
                    "observation": observation,
                    "rust_requested_peak_within_caller_envelope": within,
                    "source_committed": false,
                    "owner_published": false,
                    "scope": "actual installed Host IMAGE original-counter capture/compiler/caller preparation, exact owned Source checking/entry expansion/resident codebooks; process-start conservative requested Rust peak includes temporary old+new realloc backing; excludes allocator metadata, C allocations, mappings, stacks, child compiler and report serialization; never an execution grant or qualification of later member/journal/planning overlap",
                }),
            )?;
            if !within {
                return Err("actual Source preparation Rust backing exceeds independently selected caller envelope".into());
            }
            println!("Funded Source preparation measured; no owner, admission, Source revision or Play was published.");
        }
        return Ok(());
    }
    let host_id = prepared.host.advertisement().host_id.clone();
    let boot = prepared.host.advertisement().boot_id.clone();
    let generation = prepared.host.advertisement().offer_generation;
    let (mut owner, _resumed_expected) =
        owner::resume_selected_source_service(prepared.host, state_dir, &selection.expected)?;
    if let Some(factory) = creche {
        owner.install_creche_factory(factory)?;
    }
    let peer = owner.prepare_current_member_carrier_receiver()?;
    write_json_atomic(&state_dir.join("source-policy-owner-peer.json"), &peer)?;
    for member in &selection.admissions {
        admit_selected_member(&mut owner, state_dir, member, &peer)?;
    }
    let mut returned_carriers = Vec::with_capacity(selection.returns.len());
    for member in &selection.returns {
        returned_carriers.push(return_selected_member(
            &mut owner, state_dir, member, &peer,
        )?);
    }
    // Only the ordinary authenticated returns selected by this policy advanced
    // this Root since the exact old basis and trusted restart were verified.
    // Source authority remains the same selected File/workset transition.
    let current_expected = owner.selected_source_current_basis();
    let authorization = AuthorizedFileRevision {
        selection: selection.clone(),
        current_expected,
        host: host_id,
        boot,
        generation,
        source: prepared.source,
        checked,
        caller: prepared.caller,
    };
    // Retaining check input authorizes no automatic adoption at subsequent Boot.
    // An interrupted CAS is recovered by the existing journal, never replaying
    // this single-use authorization.
    write_json_atomic(&state_dir.join(SELECTION_FILE), &selection)?;
    owner.commit_authorized_file_revision(state_dir, authorization)?;
    let mut carriers = open_selected_play_carriers(&selection.plays, &mut returned_carriers);
    let mut line_sessions = Vec::new();
    let mut native_lines_planned = false;
    if !selection.plays.is_empty() || !selection.continuation_outputs.is_empty() {
        // Original sockets are held before planning and through submission.
        // Open failures are explicit; possession alone advertises no Ready Line.
        let line_peer = owner.member_carrier_peer()?;
        match owner.plan_retained_with_fore_provider(state_dir, |partitions| {
            let (lines, sessions) =
                select_direct_fore_lines(&selection.plays, &mut carriers, &line_peer, partitions)?;
            line_sessions = sessions;
            Ok(lines)
        }) {
            Ok(()) => {
                native_lines_planned = true;
                if let Err(error) =
                    super::source_continuation::export(&owner, &selection.continuation_outputs)
                {
                    // Source CAS is already committed. Preserve the SAME Root,
                    // current members and approvals while the selected output
                    // refusal remains explicit; an export is never a Start.
                    eprintln!("selected continuation output refused: {error}");
                }
                for ((play, carrier), sessions) in selection
                    .plays
                    .iter()
                    .zip(&mut carriers)
                    .zip(line_sessions.drain(..))
                {
                    let result = match carrier {
                        Ok(carrier) => submit_selected_play(&mut owner, play, carrier, sessions),
                        Err(error) => Err(error.clone()),
                    };
                    if let Err(error) = result {
                        // CAS has committed; losing carriage cannot revoke a
                        // receiver or discard the original approval owner.
                        eprintln!(
                            "selected Play request refused or uncertain; owner retained: {error}"
                        );
                    }
                }
            }
            Err(error) => eprintln!("selected workset retained; Plan refused: {error}"),
        }
    }
    // A selected native Line keeps the original descriptor, including an
    // uncertain preparation reply, under this same Root's lifetime. Ordinary
    // preparation-only carriers retain their existing release behavior.
    let _native_line_carriers: Vec<_> = carriers
        .into_iter()
        .zip(&selection.plays)
        .filter_map(|(carrier, play)| {
            (native_lines_planned && play.direct_fore_carrier.is_some()).then_some(carrier)
        })
        .collect();
    // Returns not selected for a Play have no continuing serial assignment.
    drop(returned_carriers);
    let ad = owner.host.advertisement();
    let status = RuntimeStatus {
        schema: RUNTIME_SCHEMA.into(),
        host_id: ad.host_id.as_str().into(),
        boot_id: ad.boot_id.as_str().into(),
        offer_generation: ad.offer_generation.0,
        process_id: std::process::id(),
        body_id: Some(selection.expected.body_id.as_str().into()),
        release_bundle_sha256: installation.release_bundle_sha256.clone(),
    };
    let content = bounded_file_digest(
        Path::new(&installation.product_executable),
        MAXIMUM_RELEASE_FILE_BYTES,
    )?;
    let runtime = crate::durable_host_control::DurableHostRuntime::from_selected_owner(
        running_target_id()?.into(),
        content,
        owner,
        state_dir,
    );
    // The owner itself retains capture, compiler workspace and checked workset.

    write_json_atomic(&state_dir.join("runtime.json"), &status)?;
    #[cfg(all(feature = "root-allocation-observation", not(test)))]
    {
        let observation = conduit_std_host::allocation_observation::snapshot();
        write_json_atomic(
            &state_dir.join("source-policy-memory.json"),
            &serde_json::json!({
                "schema": "conduit.diagnostic/root-preparation-allocations@1",
                "body_id": selection.expected.body_id,
                "snapshot": selection.snapshot,
                "entries": selection.entries,
                "caller_workspace_bytes": selection.caller_workspace_bytes,
                "observation": observation,
                "scope": "successful requested Rust backing, all threads, process start through completed Source adoption and runtime publication; conservative old+new realloc peak; excludes allocator metadata, C allocation, mappings, stacks, child compiler and this report serialization; diagnostic only, never a resource grant or complete qualification",
            }),
        )?;
    }
    println!("Selected Source workset committed; same owner Root is serving. Carrier reports do not prove receiver admission or Play.");
    let outcome = crate::durable_host_control::serve(state_dir, runtime);
    let _ = fs::remove_file(state_dir.join("runtime.json"));
    outcome
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn adopt(_state_dir: &Path, _policy: &Path, _prepare_only: bool) -> Result<(), String> {
    Err("selected bounded Source compiler requires Linux".into())
}

#[cfg(all(test, target_os = "linux"))]
#[path = "source_revision_admission_key_tests.rs"]
mod admission_key_tests;
