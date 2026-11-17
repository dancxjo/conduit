//! Private immutable release input, acquired before proposing the Womb Play.
use super::*;
use conduit_host_make::{
    verify_host_release_bytes, HostReleaseBounds, HostReleaseManifest, ReleaseTargetProfile,
    VerifiedHostReleaseBytes,
};
use std::io::Cursor;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReleasePin {
    pub(super) manifest_digest: String,
    pub(super) source_identity: String,
}

pub(super) struct Material {
    manifest_bytes: Vec<u8>,
    pub(super) manifest: HostReleaseManifest,
    pub(super) payloads: Vec<Vec<u8>>,
    pub(super) verified: Option<VerifiedHostReleaseBytes>,
    pub(super) derived: Option<super::browser_release::Derived>,
    pub(super) seed: Option<super::seed::Seed>,
}

impl Material {
    pub(super) fn is_verified(&self) -> bool {
        self.verified.is_some()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::workspace_runtime) struct Stage {
    host_id: HostId,
    boot_id: BootId,
    reservation_id: [u8; 32],
    operation: Operation,
}

#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum Operation {
    Begin {
        manifest: Vec<u8>,
    },
    Chunk {
        file: usize,
        offset: usize,
        bytes: Vec<u8>,
    },
    Finish,
}

pub(super) const BOUNDS: HostReleaseBounds = HostReleaseBounds {
    maximum_manifest_bytes: 256 * 1024,
    maximum_files: 16,
    maximum_file_bytes: 32 * 1024 * 1024,
    maximum_bundle_bytes: 64 * 1024 * 1024,
};

pub(in crate::workspace_runtime) fn stage(
    body: &BodyLifecycleSession,
    stage: Stage,
) -> Result<Vec<u8>, Refusal> {
    require_issuer(body, &stage.host_id, &stage.boot_id)?;
    if body.evidence().body.state != BodyState::Lulled {
        return Err(Refusal::new(
            "Womb.NotLulled",
            "Stage release inputs before proposing a Play",
        ));
    }
    crate::plot_runner::workspace::require_empty()?;
    PROVISION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let provision = slot.as_mut().ok_or("Womb has no private provision")?;
        if provision.id != stage.reservation_id
            || provision.host != stage.host_id
            || provision.boot != stage.boot_id
        {
            return Err(Refusal::new(
                "Womb.StaleReservation",
                "Release input names another reservation",
            ));
        }
        if provision.binding.is_some() || provision.provider_attempted {
            return Err(Refusal::new(
                "Womb.AlreadyBound",
                "Release inputs are immutable after Back preparation",
            ));
        }
        let pin = provision.release_pin.as_ref().ok_or_else(|| {
            Refusal::new(
                "Womb.UnpinnedRelease",
                "Reserve an exact reviewed manifest and source before staging",
            )
        })?;
        let verified = match stage.operation {
            Operation::Begin { manifest } => {
                if provision.material.is_some() {
                    return Err("Release input staging already began".into());
                }
                let material = begin(manifest, pin, &provision.expected_release_digest)
                    .map_err(|error| Refusal::new("Womb.InvalidRelease", error))?;
                provision.material = Some(material);
                false
            }
            Operation::Chunk {
                file,
                offset,
                bytes,
            } => {
                let material = provision
                    .material
                    .as_mut()
                    .ok_or("Begin release staging first")?;
                if material.verified.is_some() {
                    return Err("Verified release bytes are immutable".into());
                }
                let payload = material
                    .payloads
                    .get_mut(file)
                    .ok_or("Unknown release file")?;
                let expected = material.manifest.files[file].bytes as usize;
                if bytes.is_empty()
                    || bytes.len() > 64 * 1024
                    || offset != payload.len()
                    || offset
                        .checked_add(bytes.len())
                        .is_none_or(|end| end > expected)
                {
                    return Err("Release chunk exceeds its exact sequential file bound".into());
                }
                payload.extend_from_slice(&bytes);
                false
            }
            Operation::Finish => {
                let material = provision
                    .material
                    .as_mut()
                    .ok_or("Begin release staging first")?;
                if material.verified.is_none() {
                    let profile = browser_profile();
                    let verified = verify_host_release_bytes(
                        &material.manifest_bytes,
                        &profile,
                        &pin.source_identity,
                        BOUNDS,
                        |path| {
                            material
                                .manifest
                                .files
                                .iter()
                                .position(|file| file.path == path)
                                .map(|index| Cursor::new(material.payloads[index].as_slice()))
                                .ok_or_else(|| "Undeclared release path".into())
                        },
                        || false,
                    )
                    .map_err(|error| Refusal::new("Womb.InvalidRelease", error))?;
                    material.verified = Some(verified);
                }
                if material.seed.is_none() {
                    material.seed = Some(super::seed::Seed::reserve());
                }
                true
            }
        };
        encode(
            &serde_json::json!({"schema":"conduit.womb/release-input@1", "bytes_verified":verified,
            "artifact_prepared":false,"invitation_delivered":false,"membership_claimed":false}),
        )
    })
}

pub(super) fn browser_profile() -> ReleaseTargetProfile<'static> {
    ReleaseTargetProfile {
        target_id: "browser/wasm32/page",
        package_id: "browser-wasm@1",
        output: "browser-bundle",
        builder_adapter: "conduit-host-browser/build-wasm@1",
        deployment_adapter: "conduit-host-browser/load@1",
    }
}

fn begin(bytes: Vec<u8>, pin: &ReleasePin, aggregate: &str) -> Result<Material, String> {
    if bytes.is_empty()
        || bytes.len() > BOUNDS.maximum_manifest_bytes
        || format!("sha256:{:x}", Sha256::digest(&bytes)) != pin.manifest_digest
        || pin.source_identity.is_empty()
        || pin.source_identity.len() > 256
    {
        return Err(
            "Release manifest differs from reserved exact manifest/source selection".into(),
        );
    }
    let manifest: HostReleaseManifest =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let profile = browser_profile();
    if manifest.schema != conduit_host_make::HOST_RELEASE_BUNDLE_SCHEMA
        || manifest.target_id != profile.target_id
        || manifest.make_package_id != profile.package_id
        || manifest.output != profile.output
        || manifest.builder_adapter != profile.builder_adapter
        || manifest.deployment_adapter != profile.deployment_adapter
        || manifest.source_identity != pin.source_identity
        || manifest.bundle_sha256 != aggregate
        || manifest.files.is_empty()
        || manifest.files.len() > BOUNDS.maximum_files
    {
        return Err("Release manifest differs from reserved browser Make selection".into());
    }
    let mut total = 0u64;
    for (index, file) in manifest.files.iter().enumerate() {
        if file.path.is_empty()
            || file.path.len() > 256
            || file.path == "."
            || file.path == ".."
            || !file
                .path
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
            || manifest.files[..index]
                .iter()
                .any(|other| other.path == file.path)
            || file.bytes == 0
            || file.bytes > BOUNDS.maximum_file_bytes
        {
            return Err("Release file violates its finite flat inventory".into());
        }
        total = total
            .checked_add(file.bytes)
            .filter(|total| *total <= BOUNDS.maximum_bundle_bytes)
            .ok_or("Release inventory exceeds its total byte bound")?;
    }
    // Allocate each exact capacity during preparation, never during Play.
    let payloads = manifest
        .files
        .iter()
        .map(|file| Vec::with_capacity(file.bytes as usize))
        .collect();
    Ok(Material {
        manifest_bytes: bytes,
        manifest,
        payloads,
        verified: None,
        derived: None,
        seed: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Vec<u8>, ReleasePin) {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "schema":"conduit.release/host-bundle@1", "target_id":"browser/wasm32/page",
            "make_package_id":"browser-wasm@1", "output":"browser-bundle",
            "builder_adapter":"conduit-host-browser/build-wasm@1", "deployment_adapter":"conduit-host-browser/load@1",
            "source_identity":"source/reviewed", "bundle_sha256":"selected-aggregate",
            "files":[{"path":"runtime.wasm","bytes":8,"sha256":"unverified","media_type":"application/wasm"}]
        })).unwrap();
        let pin = ReleasePin {
            manifest_digest: format!("sha256:{:x}", Sha256::digest(&bytes)),
            source_identity: "source/reviewed".into(),
        };
        (bytes, pin)
    }
    #[test]
    fn retained_manifest_is_pinned_but_not_payload_verification() {
        let (bytes, pin) = fixture();
        let material = begin(bytes.clone(), &pin, "selected-aggregate").unwrap();
        assert!(!material.is_verified());
        assert_eq!(material.payloads[0].capacity(), 8);
        assert!(material.payloads[0].is_empty());
        let mut changed = bytes.clone();
        changed.push(b' ');
        assert!(begin(changed, &pin, "selected-aggregate").is_err());
        assert!(begin(bytes.clone(), &pin, "other-aggregate").is_err());
        let wrong_source = ReleasePin {
            source_identity: "source/other".into(),
            ..pin
        };
        assert!(begin(bytes, &wrong_source, "selected-aggregate").is_err());
    }
    #[test]
    fn even_exactly_pinned_manifest_cannot_exceed_file_bound() {
        let (bytes, mut pin) = fixture();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["files"][0]["bytes"] = (BOUNDS.maximum_file_bytes + 1).into();
        let changed = serde_json::to_vec(&value).unwrap();
        pin.manifest_digest = format!("sha256:{:x}", Sha256::digest(&changed));
        assert!(begin(changed, &pin, "selected-aggregate").is_err());
    }
}
