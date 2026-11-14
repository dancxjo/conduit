//! Same-Body binding of an observed prior Make release. Packaging and target
//! semantic validation remain separate from this generic byte-identity check.
use conduit_body::{BodyBiographyEvidence, BodyLifecycleEvent, SpawnInvitationClaim};
use conduit_host_make::{
    CheckedHostConfiguration, HostImage, MakeCatalog, MakePackageSet, SporeOutputKind,
    VerifiedHostReleaseBytes,
};

use crate::{
    check_joining_host, seal_prebuilt_body_spore_with_content_digest, BuiltSpore,
    JoiningHostSelection, SelectedPrebuiltContent,
};

pub struct JoiningReleaseSelection<'a> {
    pub body: &'a BodyBiographyEvidence,
    pub invitation: &'a SpawnInvitationClaim,
    pub configuration: &'a CheckedHostConfiguration,
    pub output: SporeOutputKind,
    pub image: &'a HostImage,
    pub image_manifest: &'a [u8],
    pub release: &'a VerifiedHostReleaseBytes,
}

/// Bind the exact verified inventory aggregate, never a payload or archive SHA.
/// This performs no I/O, packaging, deployment, Boot or admission. In particular,
/// a browser caller must additionally validate its profile-gated bundle IMAGE.
/// The packaging owner must reverify the bytes it writes, retain private issued
/// provisioning, and recheck invitation expiry before publishing completion.
pub fn seal_joining_host_release(
    selected: JoiningReleaseSelection<'_>,
    catalog: &MakeCatalog,
    packages: &MakePackageSet,
    now_millis: u64,
) -> Result<BuiltSpore, String> {
    let construction = check_joining_host(
        selected.body,
        selected.invitation,
        JoiningHostSelection {
            configuration: selected.configuration,
            output: selected.output,
            deployment: None,
        },
        now_millis,
        catalog,
        packages,
    )?;
    let manifest = selected.release.manifest();
    let image = &selected.image.manifest;
    let output = serde_json::to_value(&image.output).map_err(|e| e.to_string())?;
    if manifest.target_id != image.target
        || manifest.make_package_id != image.make_package_id
        || output.as_str() != Some(manifest.output.as_str())
        || manifest.builder_adapter != image.builder_adapter
        || Some(manifest.deployment_adapter.as_str()) != image.deployment_adapter.as_deref()
        || manifest.source_identity != image.source_identity
    {
        return Err("joining release differs from the selected Make IMAGE/source".into());
    }
    let birth_sign = match selected.body.body.events.first() {
        Some(BodyLifecycleEvent::Born { sign_id, .. }) => sign_id.as_str(),
        _ => return Err("joining release lacks original Birth Sign".into()),
    };
    seal_prebuilt_body_spore_with_content_digest(
        &construction,
        &selected.configuration.configuration().name,
        birth_sign,
        selected.image,
        SelectedPrebuiltContent {
            image_manifest_bytes: selected.image_manifest,
            image_content_digest: selected.release.image_content_digest(),
        },
        catalog,
        packages,
    )
    .map_err(|e| format!("joining release Seed binding: {e:?}"))
}

#[cfg(test)]
mod tests;
