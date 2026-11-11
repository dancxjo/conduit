//! Explicit local preparation-policy renewal, never Source adoption or deployment.
use super::*;
use std::io::Write;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    original_policy_sha256: String,
    previous_release_bundle_sha256: String,
    installed_release_bundle_sha256: String,
    expected: BodyBiographyEvidence,
    /// Retire, rather than replay, every recorded one-shot deployment request.
    retire_actions: bool,
}

#[derive(Serialize)]
struct RetiredActions {
    admissions: usize,
    returns: usize,
    plays: usize,
    continuation_outputs: usize,
}

fn candidate(
    original: &[u8],
    request: &Request,
    installation: &Installation,
    current: &BodyBiographyEvidence,
) -> Result<(Selection, RetiredActions), String> {
    if !valid_digest(&request.previous_release_bundle_sha256)
        || !valid_digest(&request.installed_release_bundle_sha256)
        || digest(original) != request.original_policy_sha256
    {
        return Err("retained Source policy digest changed".into());
    }
    let mut selection: Selection = serde_json::from_slice(original).map_err(|e| e.to_string())?;
    if selection.release_bundle_sha256 != request.previous_release_bundle_sha256
        || installation.release_bundle_sha256 != request.installed_release_bundle_sha256
        || selection.release_bundle_sha256 == installation.release_bundle_sha256
        || selection.expected.body_id != current.body_id
        || selection.expected.body.workset != current.body.workset
        || &request.expected != current
    {
        return Err("Source re-selection release, Body basis or committed workset changed".into());
    }
    let retired = RetiredActions {
        admissions: selection.admissions.len(),
        returns: selection.returns.len(),
        plays: selection.plays.len(),
        continuation_outputs: selection.continuation_outputs.len(),
    };
    if !request.retire_actions
        && retired.admissions + retired.returns + retired.plays + retired.continuation_outputs != 0
    {
        return Err(
            "Source re-selection requires explicit retirement of recorded one-shot actions".into(),
        );
    }
    // Keep the original, complete policy valid under its original release too.
    // No malformed old intent may be laundered by discarding it first.
    let mut original_installation = installation.clone();
    original_installation.release_bundle_sha256 = request.previous_release_bundle_sha256.clone();
    validate_selection(&selection, &original_installation)?;
    selection.release_bundle_sha256 = installation.release_bundle_sha256.clone();
    selection.expected = current.clone();
    selection.admissions.clear();
    selection.returns.clear();
    selection.plays.clear();
    selection.continuation_outputs.clear();
    validate_selection(&selection, installation)?;
    Ok((selection, retired))
}

fn verify_capture_and_workset(selection: &Selection, source: &str) -> Result<(), String> {
    if format!("{:x}", Sha256::digest(source.as_bytes())) != selection.snapshot.sha256
        || conduit_plot::canonical_source_document_id(source).0
            != selection.snapshot.source_document_id
    {
        return Err("retained File content differs from the original snapshot".into());
    }
    let canonical = crate::plot_source::parse(source)?;
    let document = canonical.check()?;
    let plots = selection
        .entries
        .iter()
        .map(|entry| canonical.expand_checked_for_authoring(&document, entry))
        .collect::<Result<Vec<_>, _>>()?;
    let workset = conduit_body::BodyWorkset::from_plots(plots.iter().map(|plot| {
        conduit_body::ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        )
    }))
    .map_err(|e| format!("reselected File workset: {e:?}"))?;
    if workset != selection.expected.body.workset {
        return Err("reselected File entries differ from committed Source workset".into());
    }
    // Recheck resident compilation as ordinary startup will; keep original ceilings.
    if let Some(policy) = &selection.resident_preparation {
        let _images = policy.prepare(&document, &plots)?;
    }
    Ok(())
}

fn retain_original(state_dir: &Path, original: &[u8]) -> Result<std::path::PathBuf, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let audit = state_dir.join("source-selection-audit");
    fs::create_dir_all(&audit).map_err(|e| e.to_string())?;
    restrict_directory(&audit)?;
    let path = audit.join(format!("{}.json", &digest(original)[7..]));
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o400)
        .open(&path)
    {
        Ok(mut file) => {
            file.write_all(original)
                .and_then(|()| file.sync_all())
                .map_err(|e| e.to_string())?;
            fs::File::open(&audit)
                .and_then(|d| d.sync_all())
                .map_err(|e| e.to_string())?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if bounded_read(&path, MAXIMUM_BODY_ADMISSION_BYTES)? != original {
                return Err("original Source policy audit differs".into());
            }
        }
        Err(e) => return Err(e.to_string()),
    }
    Ok(path)
}

fn commit_selection(
    state_dir: &Path,
    original: &[u8],
    selection: &Selection,
) -> Result<(std::path::PathBuf, Vec<u8>), String> {
    let path = state_dir.join(SELECTION_FILE);
    let replacement = serde_json::to_vec_pretty(selection).map_err(|e| e.to_string())?;
    if bounded_read(&path, MAXIMUM_BODY_ADMISSION_BYTES)? != original {
        return Err("Source policy changed before re-selection commit".into());
    }
    let audit = retain_original(state_dir, original)?;
    write_bytes_atomic(&path, &replacement)?;
    Ok((audit, replacement))
}

pub(crate) fn reselect(state_dir: &Path, request_path: &Path) -> Result<(), String> {
    let _host = owner_lock::acquire(state_dir)?;
    let _body = owner_lock::body(state_dir)?;
    for journal in [
        "body/owner-transaction.json",
        "body/admission-transaction.json",
    ] {
        if state_dir
            .join(journal)
            .try_exists()
            .map_err(|e| e.to_string())?
        {
            return Err(
                "Source re-selection requires ordinary recovery of the pending Body transaction"
                    .into(),
            );
        }
    }
    let installation = read_installation(&state_dir.join("installation.json"))?;
    let request: Request =
        serde_json::from_slice(&bounded_read(request_path, MAXIMUM_BODY_ADMISSION_BYTES)?)
            .map_err(|e| format!("Source re-selection request: {e}"))?;
    let path = state_dir.join(SELECTION_FILE);
    let original = bounded_read(&path, MAXIMUM_BODY_ADMISSION_BYTES)?;
    let binding = installation
        .body_state
        .as_ref()
        .ok_or("Source re-selection requires an installed owner Body")?;
    let biography = bounded_read(
        Path::new(&binding.biography_path),
        MAXIMUM_BODY_ADMISSION_BYTES,
    )?;
    if digest(&biography) != binding.biography_sha256 {
        return Err("Source re-selection biography digest changed".into());
    }
    let current: BodyBiographyEvidence =
        serde_json::from_slice(&biography).map_err(|e| e.to_string())?;
    current
        .validate()
        .map_err(|e| format!("Source re-selection Body: {e:?}"))?;
    let (selection, retired) = candidate(&original, &request, &installation, &current)?;
    let selected = installation
        .selected_host_configuration
        .as_ref()
        .ok_or("missing selected IMAGE")?;
    let creche = selected.prepare_creche()?;
    let prepared = prepare(
        &selection,
        &installation,
        conduit_std_host::StdHostConfig {
            host_id: installation.host_id.as_str().into(),
            boot_id: fresh_identity("boot/local-source-reselection", &installation.host_id)
                .as_str()
                .into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        creche.as_ref().map(|f| f.resource_offer()),
    )?;
    if prepared.source.value().request().snapshot != selection.snapshot {
        return Err("reselected File lost its original capture".into());
    }
    verify_capture_and_workset(&selection, prepared.source.value().source())?;
    prepared
        .host
        .verify_selected_image_prepared_value(&prepared.source)?;
    prepared
        .host
        .verify_selected_image_prepared_value(&prepared.caller)?;
    // All validation precedes persistence. Ordinary startup must still re-prepare;
    // this command neither installs a compiler nor creates any execution grant.
    let (audit, replacement) = commit_selection(state_dir, &original, &selection)?;
    println!(
        "{}",
        serde_json::json!({
            "schema": "conduit.install/source-policy-reselected@1",
            "original_policy_sha256": digest(&original),
            "selected_policy_sha256": digest(&replacement),
            "original_policy_audit": audit,
            "retired_actions": retired,
            "source_adopted": false,
            "body_changed": false,
            "execution_authorized": false,
        })
    );
    Ok(())
}

#[cfg(test)]
#[path = "source_revision_reselect_tests.rs"]
mod tests;
