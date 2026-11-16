//! Fresh local OS selection against the serving Root; never Source adoption.
//! The one-slot inbox is deliberately fail-closed after claim. Neither restart
//! nor a failed operation replays it; retirement/replacement is a separate act.
use super::*;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::PathBuf;

const INBOX: &str = "source-execution";
const PENDING: &str = "pending.json";
const CLAIMED: &str = "claimed.json";
const SCHEMA: &str = "conduit.local-source-execution@1";
const MAX_LIFETIME_MS: u64 = 300_000;

/// A wakeup hint only. Original custody and private-file checks still precede
/// claim and every descriptor open. Idle service polling creates no inbox.
pub(crate) fn pending_request(state: &Path) -> Result<bool, String> {
    match std::fs::symlink_metadata(state.join(INBOX).join(PENDING)) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("inspect Source execution inbox: {error}")),
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    policy_sha256: String,
    root_host: String,
    root_boot: String,
    root_offer_generation: u64,
    release_bundle_sha256: String,
    configuration_id: String,
    expected: BodyBiographyEvidence,
    snapshot: SourceSnapshotRevision,
    entries: Vec<String>,
    expires_at_millis: u64,
    plays: Vec<UsbPlaySelection>,
    returns: Vec<UsbReturnSelection>,
    #[serde(default)]
    record_read_probe: Option<record_read::ReadSelection>,
}

/// Only the original retained guards, verified on the actual Owner, construct
/// this value. No deserialization or public DTO constructor stands in for IMAGE.
#[allow(dead_code)] // Runtime hook is integrated by the serving-Root owner.
pub(crate) struct VerifiedExecutionBasis {
    policy: Selection,
    digest: String,
    root_host: String,
    root_boot: String,
    root_generation: u64,
    current: BodyBiographyEvidence,
}

#[allow(dead_code)]
pub(crate) fn verify_retained(
    state: &Path,
    owner: &owner::Owner,
    original: &RetainedFileWorkset,
) -> Result<VerifiedExecutionBasis, String> {
    let current = owner.selected_source_current_basis();
    original.verify(&owner.host, &current.body.workset)?;
    if original.body_id != current.body_id {
        return Err("retained File belongs to another Body".into());
    }
    let bytes = bounded_read(&state.join(SELECTION_FILE), MAXIMUM_BODY_ADMISSION_BYTES)?;
    let policy: Selection = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let installation = read_installation(&state.join("installation.json"))?;
    // Validate the complete original policy BEFORE discarding old one-shots.
    validate_selection(&policy, &installation)?;
    if policy.execution_policy_fingerprint()? != original.original_policy_fingerprint {
        return Err("selected policy differs from original retained preparation identity".into());
    }
    let entries: Vec<_> = original
        .checked
        .plots
        .iter()
        .map(|plot| plot.expanded.name.clone())
        .collect();
    if policy.expected.body_id != current.body_id
        || policy.snapshot != original.source.value().request().snapshot
        || policy.entries != entries
        || serde_json::to_vec(&policy.local_fore_adapters).map_err(|e| e.to_string())?
            != serde_json::to_vec(&original.local_fore_adapters).map_err(|e| e.to_string())?
    {
        return Err("selected policy differs from original retained Source preparation".into());
    }
    let ad = owner.host.advertisement();
    if ad.host_id.as_str() != installation.host_id {
        return Err("serving Root differs from installed Host".into());
    }
    Ok(VerifiedExecutionBasis {
        policy,
        digest: digest(&bytes),
        root_host: ad.host_id.as_str().into(),
        root_boot: ad.boot_id.as_str().into(),
        root_generation: ad.offer_generation.0,
        current,
    })
}

impl VerifiedExecutionBasis {
    /// The Owner hook must pass its actual still-installed retained field, never
    /// a reconstructed File or a fallback capture. This rechecks original IMAGE
    /// custody at the mutation boundary without borrowing it across planning.
    pub(crate) fn verify_original(
        &self,
        owner: &owner::Owner,
        original: &RetainedFileWorkset,
    ) -> Result<(), String> {
        let current = owner.selected_source_current_basis();
        let ad = owner.host.advertisement();
        original.verify(&owner.host, &current.body.workset)?;
        if current != self.current
            || original.body_id != current.body_id
            || ad.host_id.as_str() != self.root_host
            || ad.boot_id.as_str() != self.root_boot
            || ad.offer_generation.0 != self.root_generation
            || original.original_policy_fingerprint != self.policy.execution_policy_fingerprint()?
        {
            return Err("original Source execution custody or actual Root basis changed".into());
        }
        Ok(())
    }
}

fn validate_request(request: &Request, now: u64) -> Result<(), String> {
    if request.schema != SCHEMA
        || !valid_digest(&request.policy_sha256)
        || request.root_host.is_empty()
        || request.root_boot.is_empty()
        || request.entries.is_empty()
        || request.entries.len() > conduit_body::MAX_BODY_PLOTS
        || request.plays.is_empty()
        || request.plays.len() > conduit_body::MAX_BODY_PLOTS
        || request.returns.len() > 4
        || request.expires_at_millis <= now
        || request
            .expires_at_millis
            .checked_sub(now)
            .is_none_or(|left| left > MAX_LIFETIME_MS)
    {
        return Err("invalid or expired finite local Source execution request".into());
    }
    Ok(())
}

fn private_directory(path: &Path, uid: u32) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
        return Err("Source execution requires an owner-private real directory".into());
    }
    Ok(())
}

fn inbox(state: &Path) -> Result<PathBuf, String> {
    let metadata = std::fs::symlink_metadata(state).map_err(|e| e.to_string())?;
    private_directory(state, metadata.uid())?;
    let path = state.join(INBOX);
    match std::fs::DirBuilder::new().mode(0o700).create(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.to_string()),
    }
    private_directory(&path, metadata.uid())?;
    Ok(path)
}

pub(crate) fn submit(state: &Path, path: &Path) -> Result<(), String> {
    let bytes = bounded_read(path, MAXIMUM_BODY_ADMISSION_BYTES)?;
    let request: Request = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    validate_request(&request, current_time_millis()?)?;
    let directory = inbox(state)?;
    if directory
        .join(CLAIMED)
        .try_exists()
        .map_err(|e| e.to_string())?
    {
        return Err(
            "Source execution slot already claimed; no automatic replay or replacement".into(),
        );
    }
    let staged = directory.join("staged.json");
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&staged)
        .map_err(|e| e.to_string())?;
    output.write_all(&bytes).map_err(|e| e.to_string())?;
    output.sync_all().map_err(|e| e.to_string())?;
    std::fs::hard_link(&staged, directory.join(PENDING)).map_err(|e| e.to_string())?;
    std::fs::remove_file(staged).map_err(|e| e.to_string())?;
    std::fs::File::open(&directory)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| e.to_string())
}

/// Claim precedes parsing/verification/effects. A crash at any subsequent point
/// leaves the fixed claimed slot, so neither another Boot nor polling replays it.
fn claim(state: &Path) -> Result<Option<Vec<u8>>, String> {
    let directory = inbox(state)?;
    let pending = directory.join(PENDING);
    let claimed = directory.join(CLAIMED);
    if claimed.try_exists().map_err(|e| e.to_string())? {
        return Ok(None);
    }
    let metadata = match std::fs::symlink_metadata(&pending) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let uid = std::fs::metadata(state).map_err(|e| e.to_string())?.uid();
    if !metadata.is_file() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
        return Err("Source execution request is not an owner-private regular file".into());
    }
    // hard_link is no-clobber, unlike rename. The private directory is the OS
    // authority boundary; only its owner may mutate selection files.
    std::fs::hard_link(&pending, &claimed).map_err(|e| e.to_string())?;
    std::fs::File::open(&directory)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| e.to_string())?;
    std::fs::remove_file(pending).map_err(|e| e.to_string())?;
    bounded_read(&claimed, MAXIMUM_BODY_ADMISSION_BYTES).map(Some)
}

fn selected(
    request: Request,
    basis: &VerifiedExecutionBasis,
    now: u64,
) -> Result<Selection, String> {
    validate_request(&request, now)?;
    if request
        .record_read_probe
        .as_ref()
        .is_some_and(|read| read.member_host.is_empty() || read.member_host.len() > 256)
    {
        return Err("record read requires one finite selected member Host".into());
    }
    if request.policy_sha256 != basis.digest
        || request.root_host != basis.root_host
        || request.root_boot != basis.root_boot
        || request.root_offer_generation != basis.root_generation
        || request.expected != basis.current
        || request.snapshot != basis.policy.snapshot
        || request.entries != basis.policy.entries
        || request.release_bundle_sha256 != basis.policy.release_bundle_sha256
        || request.configuration_id != basis.policy.configuration_id
    {
        return Err(
            "fresh execution does not match actual Root, Body, policy or original Source".into(),
        );
    }
    let mut selection = basis.policy.clone();
    selection.expected = request.expected;
    selection.plays = request.plays;
    selection.returns = request.returns;
    selection.admissions.clear();
    selection.continuation_outputs.clear();
    Ok(selection)
}

/// The caller must retain this opaque owner alongside the SAME Root. It keeps
/// every opened descriptor, including uncertain Return/Prepare outcomes. It is
/// not a claim of a successful Plan, Start, application delivery or recording.
#[allow(dead_code)]
pub(crate) struct RetainedExecution {
    carriers: Vec<carriage::RetainedForeClient>,
    returned: Vec<carriage::SelectedPlayCarrier>,
    failed_returns: Vec<carriage::RetainedReturnFailure>,
    pub(crate) outcomes: Vec<String>,
    read: Option<record_read::PendingRead>,
}

/// The metadata describes the original exact Fore; it grants no Start/Ready.
pub(crate) use super::carriage::AuthorizedForeProbe;
#[allow(dead_code)]
impl RetainedExecution {
    pub(crate) fn progress_record_read(
        &mut self,
        owner: &owner::Owner,
        state: &Path,
    ) -> Result<(), String> {
        let Some(mut read) = self.read.take() else {
            return Ok(());
        };
        let result = if read.attempted() {
            Ok(())
        } else {
            let host = conduit_core::HostId::from(read.member_host());
            self.probe(owner, &host, |probe| read.exchange(probe))
        };
        let copy = read.retain_diagnostic_copy(state);
        // Retain the full canonical outcome even if carriage or disk later failed.
        self.read = Some(read);
        result.and(copy)
    }
    /// Borrow one exact current Host's authorized probe synchronously. The
    /// callback cannot detach or clone the descriptor; no IO occurs on selection.
    pub(crate) fn probe<R>(
        &mut self,
        owner: &owner::Owner,
        host: &conduit_core::HostId,
        operation: impl for<'probe> FnOnce(&mut AuthorizedForeProbe<'probe>) -> Result<R, String>,
    ) -> Result<R, String> {
        let mut selected = self
            .carriers
            .iter_mut()
            .filter(|carrier| carrier.matches_host(host));
        let carrier = selected
            .next()
            .ok_or("no retained original Fore carrier for Host")?;
        if selected.next().is_some() {
            return Err("retained Fore Host selection is ambiguous".into());
        }
        carrier.probe(owner, operation)
    }
}

/// Called only by the serving local runtime, with original retained File guards.
/// No network request selects a path or manufactures this verified basis.
#[allow(dead_code)]
pub(crate) fn claim_and_execute(
    state: &Path,
    owner: &mut owner::Owner,
    basis: VerifiedExecutionBasis,
) -> Result<Option<RetainedExecution>, String> {
    // Guard verification must precede any carrier open. No target offer is
    // required here: its independently authenticated Return supplies evidence.
    owner.verify_selected_source_execution_basis(&basis)?;
    let current_policy = bounded_read(&state.join(SELECTION_FILE), MAXIMUM_BODY_ADMISSION_BYTES)?;
    if digest(&current_policy) != basis.digest {
        return Err("selected Source policy changed before execution claim".into());
    }
    let current_installation = read_installation(&state.join("installation.json"))?;
    validate_selection(&basis.policy, &current_installation)?;
    let Some(bytes) = claim(state)? else {
        return Ok(None);
    };
    let request: Request = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let read = request.record_read_probe.clone();
    let selection = selected(request, &basis, current_time_millis()?)?;
    // Fixed staging is part of the retained original caller envelope. This
    // witness keeps that original charge alive; it creates no subreservation.
    let read = read
        .map(|selection| {
            owner
                .retain_selected_source_read_memory()
                .map(|memory| record_read::PendingRead::new(selection, memory))
        })
        .transpose()?;
    let installation = read_installation(&state.join("installation.json"))?;
    validate_selection(&selection, &installation)?;
    owner.require_remote_workset_retired()?;
    carriage::validate_execution_carrier_paths(&selection.plays, &selection.returns)?;
    let mut retained = RetainedExecution {
        carriers: Vec::new(),
        returned: Vec::new(),
        failed_returns: Vec::new(),
        outcomes: Vec::new(),
        read,
    };
    for selected in &selection.returns {
        match carriage::return_selected_member_retained(owner, state, selected) {
            Ok(Ok(carrier)) => retained.returned.push(carrier),
            Ok(Err(failure)) => {
                retained.outcomes.push(failure.error.clone());
                retained.failed_returns.push(failure);
                // Never reopen a failed Return's original descriptor for Play.
                return Ok(Some(retained));
            }
            Err(error) => {
                retained.outcomes.push(error);
                return Ok(Some(retained));
            }
        }
    }
    let mut carriers = open_selected_play_carriers(&selection.plays, &mut retained.returned);
    let mut sessions = Vec::new();
    let result = owner.member_carrier_peer().and_then(|peer| {
        owner.plan_retained_with_fore_provider(state, |partitions| {
            let (lines, selected_sessions) =
                select_direct_fore_lines(&selection.plays, &mut carriers, &peer, partitions)?;
            sessions = selected_sessions;
            Ok(lines)
        })
    });
    if let Err(error) = result {
        retained.outcomes.push(error);
    } else {
        for ((play, carrier), sessions) in selection.plays.iter().zip(&mut carriers).zip(sessions) {
            if let Ok(carrier) = carrier {
                if let Err(error) = submit_selected_play(owner, play, carrier, sessions) {
                    retained.outcomes.push(error);
                }
            }
        }
    }
    for (carrier, play) in carriers.into_iter().zip(&selection.plays) {
        match carrier {
            Ok(carrier) => retained
                .carriers
                .push(negotiate_selected_fore(carrier, play)),
            Err(error) => retained.outcomes.push(error),
        }
    }
    Ok(Some(retained))
}

#[cfg(test)]
#[path = "source_execution_tests.rs"]
mod tests;

#[path = "source_execution_record_read.rs"]
mod record_read;
