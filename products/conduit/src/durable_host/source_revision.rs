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

const SELECTION_FILE: &str = "source-check-selection.json";

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

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UsbReturnSelection {
    part_id: conduit_body::PartId,
    port: std::path::PathBuf,
    basis: std::path::PathBuf,
    receipt_output: std::path::PathBuf,
    wait_milliseconds: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UsbAdmissionSelection {
    host_id: conduit_core::HostId,
    member_verifying_key: [u8; 32],
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
#[cfg(target_os = "linux")]
pub(crate) type PreparationCustody = (
    ImagePreparedValue<OwnedSourceCheckInput>,
    ImagePreparedValue<()>,
);

#[cfg(target_os = "linux")]
impl AuthorizedFileRevision {
    pub(crate) fn local_fore_adapters(
        &self,
    ) -> &[super::local_fore_policy::LocalForeAdapterIntent] {
        &self.selection.local_fore_adapters
    }

    pub(crate) fn into_preparation(self) -> PreparationCustody {
        (self.source, self.caller)
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
                || member.member_verifying_key == [0; 32]
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
pub(crate) fn adopt(state_dir: &Path, policy: &Path) -> Result<(), String> {
    let _host_lock = owner_lock::acquire(state_dir)?;
    let _body_lock = owner_lock::body(state_dir)?;
    let installation = read_installation(&state_dir.join("installation.json"))?;
    let selection: Selection =
        serde_json::from_slice(&bounded_read(policy, MAXIMUM_BODY_ADMISSION_BYTES)?)
            .map_err(|e| format!("File revision selection: {e}"))?;
    validate_selection(&selection, &installation)?;
    // Reuse the ordinary OS-protected service credential; it grants no Source
    // authority. Prepare it before the transition so the retained Root can serve.
    crate::durable_host_control::ensure_secret(state_dir)?;
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
    let plots = selection
        .entries
        .iter()
        .map(|entry| source.expand_named_for_authoring(entry))
        .collect::<Result<Vec<_>, _>>()?;
    let checked = CheckedFileRevision { source, plots };
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
    for member in &selection.returns {
        return_selected_member(&mut owner, state_dir, member, &peer)?;
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
    let custody = owner.commit_authorized_file_revision(state_dir, authorization)?;
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
    let mut runtime = crate::durable_host_control::DurableHostRuntime::from_selected_owner(
        running_target_id()?.into(),
        content,
        owner,
        state_dir,
    );
    runtime.retain_source_preparation(Some(custody));
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
    println!("Selected Source workset committed; same owner Root is serving. Replacement Wake requires fresh owner authorization.");
    let outcome = crate::durable_host_control::serve(state_dir, runtime);
    let _ = fs::remove_file(state_dir.join("runtime.json"));
    outcome
}

/// Use the ordinary canonical image-return protocol on the same live owner.
/// Public basis files pin the reinstalled member to this actual owner peer;
/// neither a copied advertisement nor USB access establishes current offers.
#[cfg(target_os = "linux")]
fn return_selected_member(
    owner: &mut owner::Owner,
    state_dir: &Path,
    selected: &UsbReturnSelection,
    peer: &conduit_body::FederationPeer,
) -> Result<(), String> {
    use conduit_std_host::pico_spawn::PicoMemberReturnBasis;
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + Duration::from_millis(selected.wait_milliseconds);
    while !selected.basis.try_exists().map_err(|e| e.to_string())? {
        if Instant::now() >= deadline {
            return Err("selected member basis did not arrive within authorized wait".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let basis: PicoMemberReturnBasis =
        serde_json::from_slice(&bounded_read(&selected.basis, 8192)?)
            .map_err(|e| format!("selected member return basis: {e}"))?;
    basis
        .validate()
        .map_err(|e| format!("selected member return basis: {e:?}"))?;
    if &basis.owner != peer || basis.credential.part_id != selected.part_id {
        return Err("selected return differs from the member's independently installed current owner pin or selected Part".into());
    }
    return_selected_basis(
        owner,
        state_dir,
        &selected.port,
        &selected.receipt_output,
        &basis,
        peer,
    )
}

#[cfg(target_os = "linux")]
fn return_selected_basis(
    owner: &mut owner::Owner,
    state_dir: &Path,
    port: &Path,
    receipt_output: &Path,
    basis: &conduit_std_host::pico_spawn::PicoMemberReturnBasis,
    peer: &conduit_body::FederationPeer,
) -> Result<(), String> {
    use conduit_body::MemberReturnImageAcknowledgement;
    use conduit_std_host::pico_spawn::PicoSpawnSocket;
    use std::time::Duration;
    let mut receipt = membership_receipt_custody::reserve(receipt_output)?;
    let timeout = Duration::from_secs(5);
    let mut socket = PicoSpawnSocket::open(port.to_str().ok_or("USB path is not UTF-8")?)
        .map_err(|e| format!("open selected member return: {e:?}"))?;
    let observed = socket
        .request_member_return(basis, timeout)
        .map_err(|e| format!("observe selected member: {e:?}"))?;
    let resolved = conduit_std_host::pico_spawn::resolve_member_offer_images(
        &observed.presentation.projection,
        &observed.images,
    )
    .map_err(|e| format!("resolve selected member images: {e:?}"))?;
    if resolved != observed.advertisement {
        return Err("selected member image resolution differs from observed advertisement".into());
    }
    let challenge = owner.begin_member_image_return(
        &basis.credential,
        &observed.advertisement,
        Some(observed.presentation.clone()),
    )?;
    let proof = socket
        .prove_member_return(basis, &observed.advertisement, &challenge, timeout)
        .map_err(|e| format!("selected member continuity proof: {e:?}"))?;
    let credential = owner.complete_member_return(state_dir, &proof)?;
    let signed = owner.attest_current_member_receipt(
        &owner::MembershipReceiptDocument::ImageReturn {
            receipt: MemberReturnImageAcknowledgement::new(
                credential.clone(),
                observed.presentation.projection.clone(),
            ),
            images: observed.images,
        },
        peer,
    )?;
    signed
        .verify_image_return(
            peer,
            &basis.member_verifying_key,
            &basis.credential,
            &observed.presentation.projection,
        )
        .map_err(|e| format!("verify selected member receipt: {e:?}"))?;
    membership_receipt_custody::retain(&mut receipt, &signed)?;
    socket
        .deliver_member_return(&signed, basis, &observed.presentation.projection, timeout)
        .map_err(|e| {
            format!("return committed and receipt retained; member delivery uncertain: {e:?}")
        })?;
    owner.retire_completed_selected_return(&credential)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn admit_selected_member(
    owner: &mut owner::Owner,
    state_dir: &Path,
    selected: &UsbAdmissionSelection,
    peer: &conduit_body::FederationPeer,
) -> Result<(), String> {
    use conduit_std_host::pico_spawn::{
        PicoMemberReturnBasis, PicoSpawnAdmissionBasis, PicoSpawnSocket,
    };
    use std::{
        io::Write,
        time::{Duration, Instant},
    };
    // The local OS policy independently authorizes precisely this physical
    // Host/key. The ordinary invitation is still consumed only by actual proof.
    let mut invitation_file = membership_receipt_custody::reserve(&selected.invitation_output)?;
    let invitation = owner.issue_invitation(state_dir, 300, None)?;
    invitation_file
        .write_all(&serde_json::to_vec(&invitation).map_err(|e| e.to_string())?)
        .and_then(|_| invitation_file.sync_all())
        .map_err(|e| format!("retain selected private invitation: {e}"))?;
    let deadline = Instant::now() + Duration::from_millis(selected.wait_milliseconds);
    while !selected.basis.try_exists().map_err(|e| e.to_string())? {
        if Instant::now() >= deadline {
            return Err("selected admission basis did not arrive in its finite window".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let basis: PicoSpawnAdmissionBasis =
        serde_json::from_slice(&bounded_read(&selected.basis, 8192)?)
            .map_err(|e| format!("selected admission basis: {e}"))?;
    basis
        .validate(current_time_millis()?)
        .map_err(|e| format!("selected admission basis: {e:?}"))?;
    if &basis.owner != peer
        || basis.expected_host != selected.host_id
        || basis.member_verifying_key != selected.member_verifying_key
        || basis.invitation != invitation.claim
    {
        return Err("selected admission differs from actual owner invitation or independently pinned physical Host/key".into());
    }
    let mut receipt_file = membership_receipt_custody::reserve(&selected.receipt_output)?;
    let timeout = Duration::from_secs(5);
    let mut socket = PicoSpawnSocket::open(selected.port.to_str().ok_or("USB path is not UTF-8")?)
        .map_err(|e| format!("open selected admission: {e:?}"))?;
    let observed = socket
        .request_join(&basis.spore_id, &basis.image_id, &basis.invitation, timeout)
        .map_err(|e| format!("observe selected physical admission proof: {e:?}"))?;
    let request = basis
        .request(&observed, current_time_millis()?)
        .map_err(|e| format!("validate selected physical admission proof: {e:?}"))?;
    let receipt = owner.admit_invited(state_dir, request, selected.host_id.as_str())?;
    let return_basis = PicoMemberReturnBasis {
        credential: receipt.credential.clone(),
        expected_profile: basis.expected_profile.clone(),
        member_verifying_key: basis.member_verifying_key,
        owner: peer.clone(),
    };
    let signed = owner
        .attest_current_member_receipt(&owner::MembershipReceiptDocument::Spawn(receipt), peer)?;
    signed
        .verify(
            peer,
            &basis.member_verifying_key,
            &basis.invitation.body_id,
            &observed.advertisement,
        )
        .map_err(|e| format!("verify selected admission receipt: {e:?}"))?;
    membership_receipt_custody::retain(&mut receipt_file, &signed)?;
    socket
        .deliver_admission_receipt(
            &signed,
            peer,
            &basis.member_verifying_key,
            &basis.invitation.body_id,
            &observed.advertisement,
            timeout,
        )
        .map_err(|e| {
            format!("membership committed and receipt retained; delivery uncertain: {e:?}")
        })?;
    drop(socket);
    let mut return_file = membership_receipt_custody::reserve(&selected.return_basis_output)?;
    return_file
        .write_all(&serde_json::to_vec(&return_basis).map_err(|e| e.to_string())?)
        .and_then(|_| return_file.sync_all())
        .map_err(|e| format!("retain actual return basis: {e}"))?;
    // Ordinary admission does not carry native resource grants. The actual
    // newly joined member must now sign its current complete image projection.
    return_selected_basis(
        owner,
        state_dir,
        &selected.port,
        &selected.return_receipt_output,
        &return_basis,
        peer,
    )
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn adopt(_state_dir: &Path, _policy: &Path) -> Result<(), String> {
    Err("selected bounded Source compiler requires Linux".into())
}
