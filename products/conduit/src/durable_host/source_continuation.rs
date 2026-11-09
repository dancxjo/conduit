//! Optional local File-policy outputs from a real retained owner proposal.
//! No control request, candidate digest or copied signing key enters this path.
use super::*;
use std::{fs::File, io::Read, path::PathBuf};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OutputSelection {
    pub host_id: String,
    pub member_basis: PathBuf,
    pub make_manifest: PathBuf,
    pub make_bundle_sha256: String,
    pub make_source_identity: String,
    pub target_id: String,
    pub package_id: String,
    pub output: String,
    pub builder_adapter: String,
    pub deployment_adapter: String,
    pub artifact_output: PathBuf,
    pub membership_generation: u64,
    pub intent_generation: u64,
    pub ceilings: conduit_body::ContinuationCeilings,
}
/// Constructed only by the finite local selected-file verifier. Not Clone,
/// deserializable, public-field policy or a network control argument.
pub(crate) struct PreparedOutput {
    selection: OutputSelection,
    recipe: conduit_body::continuation_recipe::Document,
    member: owner::MemberPresentationBasis,
}
impl PreparedOutput {
    pub(crate) fn selection(&self) -> &OutputSelection {
        &self.selection
    }
    pub(crate) fn recipe(&self) -> &conduit_body::continuation_recipe::Document {
        &self.recipe
    }
    pub(crate) fn member(&self) -> &owner::MemberPresentationBasis {
        &self.member
    }
}
pub(crate) fn validate(selections: &[OutputSelection]) -> Result<(), String> {
    if selections.len() > conduit_body::MAX_BODY_PARTS {
        return Err("continuation output capacity exceeded".into());
    }
    for (index, s) in selections.iter().enumerate() {
        if s.host_id.is_empty()
            || s.host_id.len() > 256
            || !s.ceilings.has_finite_bounds()
            || s.membership_generation == 0
            || s.intent_generation == 0
            || [&s.member_basis, &s.make_manifest, &s.artifact_output]
                .iter()
                .any(|p| !p.is_absolute())
            || !valid_digest(&s.make_bundle_sha256)
            || [
                &s.make_source_identity,
                &s.target_id,
                &s.package_id,
                &s.output,
                &s.builder_adapter,
                &s.deployment_adapter,
            ]
            .iter()
            .any(|s| s.is_empty() || s.len() > 256)
            || selections[..index].iter().any(|prior| {
                prior.host_id == s.host_id || prior.artifact_output == s.artifact_output
            })
        {
            return Err("continuation output is not an exact finite local selection".into());
        }
    }
    Ok(())
}
fn prepare(s: &OutputSelection) -> Result<PreparedOutput, String> {
    use conduit_host_make::{verify_host_release_bytes, HostReleaseBounds, ReleaseTargetProfile};
    validate(std::slice::from_ref(s))?;
    let bytes = bounded_read(&s.make_manifest, 256 * 1024)?;
    let directory = s
        .make_manifest
        .parent()
        .ok_or("Make manifest has no directory")?;
    let expected = ReleaseTargetProfile {
        target_id: &s.target_id,
        package_id: &s.package_id,
        output: &s.output,
        builder_adapter: &s.builder_adapter,
        deployment_adapter: &s.deployment_adapter,
    };
    let mut recipe_file = None;
    let release = verify_host_release_bytes(
        &bytes,
        &expected,
        &s.make_source_identity,
        HostReleaseBounds {
            maximum_manifest_bytes: 256 * 1024,
            maximum_files: 32,
            maximum_file_bytes: 16 * 1024 * 1024,
            maximum_bundle_bytes: 32 * 1024 * 1024,
        },
        |name| {
            let path = directory.join(name);
            if !fs::symlink_metadata(&path)
                .map_err(|e| e.to_string())?
                .is_file()
            {
                return Err("selected Make payload is not a regular file".into());
            }
            let file = File::open(path).map_err(|e| e.to_string())?;
            if name == "continuation-recipe.json" {
                recipe_file = Some(file.try_clone().map_err(|e| e.to_string())?);
            }
            Ok(file)
        },
        || false,
    )?;
    if release.image_content_digest() != s.make_bundle_sha256 {
        return Err("Make bundle differs from independently selected content identity".into());
    }
    let descriptor = release
        .manifest()
        .files
        .iter()
        .find(|f| f.path == "continuation-recipe.json")
        .ok_or("sealed Make bundle has no continuation recipe")?;
    let mut file = recipe_file.ok_or("verified recipe file custody absent")?;
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut recipe_bytes = Vec::new();
    file.take(conduit_body::continuation_recipe::MAXIMUM_DOCUMENT_BYTES as u64 + 1)
        .read_to_end(&mut recipe_bytes)
        .map_err(|e| e.to_string())?;
    if recipe_bytes.len() > conduit_body::continuation_recipe::MAXIMUM_DOCUMENT_BYTES
        || recipe_bytes.len() as u64 != descriptor.bytes
        || digest(&recipe_bytes) != descriptor.sha256
    {
        return Err("retained sealed recipe bytes changed or exceeded preparation bound".into());
    }
    let recipe: conduit_body::continuation_recipe::Document =
        serde_json::from_slice(&recipe_bytes).map_err(|e| e.to_string())?;
    recipe
        .checked_digest()
        .map_err(|e| format!("Make recipe content: {e:?}"))?;
    let member = serde_json::from_slice(&bounded_read(
        &s.member_basis,
        MAXIMUM_BODY_ADMISSION_BYTES,
    )?)
    .map_err(|e| e.to_string())?;
    Ok(PreparedOutput {
        selection: s.clone(),
        recipe,
        member,
    })
}
#[cfg(target_os = "linux")]
pub(crate) fn export(owner: &owner::Owner, selections: &[OutputSelection]) -> Result<(), String> {
    validate(selections)?;
    // Prepare every selected file before publishing any output. No Source CAS,
    // Body membership, device installation or Play is performed by file export.
    let prepared = selections
        .iter()
        .map(prepare)
        .collect::<Result<Vec<_>, _>>()?;
    let artifacts = prepared
        .iter()
        .map(|selected| owner.selected_continuation_artifact(selected))
        .collect::<Result<Vec<_>, _>>()?;
    for selected in &prepared {
        if selected.selection.artifact_output.exists() {
            return Err(
                "continuation output already exists; select a fresh finite output path".into(),
            );
        }
    }
    for (selected, artifact) in prepared.iter().zip(artifacts) {
        publish_new(&selected.selection.artifact_output, &artifact)?;
    }
    Ok(())
}

fn publish_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // Exclusive creation never replaces a prior artifact or a raced pathname.
    // An interrupted partial file is retained evidence and cannot decode as a
    // complete signed fixed-size artifact. A retry must select a fresh path.
    let mut file = options
        .open(path)
        .map_err(|e| format!("create selected continuation output: {e}"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    File::open(path.parent().ok_or("continuation output has no parent")?)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_output_never_replaces_prior_or_invented_artifact() {
        let root = std::env::temp_dir().join(format!(
            "conduit-continuation-output-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let path = root.join("artifact.bin");
        publish_new(&path, b"original").unwrap();
        assert!(publish_new(&path, b"replacement").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn continuation_policy_is_explicit_and_finite() {
        assert!(validate(&[]).is_ok());
        let mut selection = OutputSelection {
            host_id: "target".into(),
            member_basis: "/selected/member".into(),
            make_manifest: "/selected/manifest".into(),
            make_bundle_sha256: format!("sha256:{}", "a".repeat(64)),
            make_source_identity: "source/exact".into(),
            target_id: "target/exact".into(),
            package_id: "package/exact".into(),
            output: "bin".into(),
            builder_adapter: "builder/exact".into(),
            deployment_adapter: "deployment/exact".into(),
            artifact_output: "/selected/output".into(),
            membership_generation: 1,
            intent_generation: 1,
            ceilings: conduit_body::ContinuationCeilings {
                assignment_bytes: 1,
                runtime_bytes: 1,
                durable_bytes: 1,
                operation_slots: 1,
                active_worksets: 1,
            },
        };
        assert!(validate(std::slice::from_ref(&selection)).is_ok());
        assert!(validate(&[selection.clone(), selection.clone()]).is_err());
        selection.intent_generation = 0;
        assert!(validate(&[selection]).is_err());
    }
}
