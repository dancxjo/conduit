//! Same-Body browser Spore preparation; admission remains a separate operation.
use super::*;
use crate::creche::browser_configuration::{self, BrowserConfigurationSelection};

#[cfg(test)]
mod tests;

pub(crate) fn prepare_workspace_browser(
    body: &BodyBiographyEvidence,
    invitation: &SpawnInvitationClaim,
    invitation_secret: &[u8; 32],
    selected_image_content_digest: &str,
    selection: BrowserConfigurationSelection,
) -> Result<PreparedSpore, String> {
    if invitation.body_id != body.body_id {
        return Err("Workspace invitation names a different Body".into());
    }
    let birth_sign_id = match body.body.events.first() {
        Some(BodyLifecycleEvent::Born { sign_id, .. }) => sign_id.as_str(),
        _ => return Err("Workspace Body has no exact BIRTH Sign".into()),
    };
    let (browser_review, checked, packages) = browser_configuration::review(selection)?;
    let target = spore_target::prepare_browser(
        body.body_id.as_str(),
        invitation.invitation_id.as_str(),
        checked,
    )?;
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let (image, image_bytes) = build_host_image(
        target.configuration.profile().clone(),
        &catalog,
        &packages,
        &target.output,
        &BuildInputs {
            source_identity: target.source_identity.into(),
            toolchain_available: true,
        },
    )
    .map_err(|errors| format!("select reviewed prebuilt IMAGE: {errors:?}"))?;
    let spore = seal_prebuilt_body_spore_with_content_digest(
        &target.body,
        target.host_name,
        birth_sign_id,
        &image,
        SelectedPrebuiltContent {
            image_manifest_bytes: &image_bytes,
            image_content_digest: selected_image_content_digest,
        },
        &catalog,
        &packages,
    )
    .map_err(|error| format!("seal Body spore: {error:?}"))?;
    if spore.manifest.binding
        != (SporeBinding::SelfJoining {
            invitation_id: invitation.invitation_id.as_str().into(),
        })
    {
        return Err("sealed spore lost its exact Workspace invitation".into());
    }
    let deployment_adapter = spore.manifest.make.deployment_adapter.clone();
    Ok(PreparedSpore {
        schema: "conduit.tour/prepared-physical-spore@1",
        disposition: "prepared",
        body_id: body.body_id.as_str().into(),
        spore_id: spore.manifest.spore_id.clone(),
        image_id: spore.manifest.image_id.clone(),
        image_content_digest: spore.manifest.image_content_digest.clone(),
        target_id: spore.manifest.target.clone(),
        output: spore.manifest.output.clone(),
        make_package_id: spore.manifest.make.make_package_id.clone(),
        deployment_adapter,
        invitation_id: invitation.invitation_id.as_str().into(),
        invitation_nonce: invitation.nonce,
        invitation_expires_at_millis: invitation.expires_at_millis,
        invitation_secret: invitation_secret.to_vec(),
        source_identity: spore.manifest.source_identity.clone(),
        browser_configuration_id: Some(browser_review.configuration_id),
        browser_profile_id: Some(browser_review.profile_id),
        browser_configuration_source: Some(browser_review.canonical_source),
        spore_manifest: spore.manifest,
        does_not_prove: [
            "deployment",
            "boot",
            "join",
            "membership",
            "offers",
            "readiness",
        ],
    })
}
