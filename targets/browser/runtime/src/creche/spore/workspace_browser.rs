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
    now_millis: u64,
) -> Result<PreparedSpore, String> {
    if invitation.body_id != body.body_id {
        return Err("Workspace invitation names a different Body".into());
    }
    let birth_sign_id = match body.body.events.first() {
        Some(BodyLifecycleEvent::Born { sign_id, .. }) => sign_id.as_str(),
        _ => return Err("Workspace Body has no exact BIRTH Sign".into()),
    };
    let (browser_review, checked, packages) = browser_configuration::review(selection)?;
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let output = packages
        .target_descriptor(&checked.profile().target.key())
        .ok_or("checked browser target is absent from its package")?
        .default_output
        .clone();
    let construction = conduit_body_make::check_joining_host(
        body,
        invitation,
        conduit_body_make::JoiningHostSelection {
            configuration: &checked,
            output: output.clone(),
            deployment: Some(conduit_body_make::DeploymentDescription {
                destination: "browser/local-sandbox".into(),
            }),
        },
        now_millis,
        &catalog,
        &packages,
    )
    .map_err(|error| error.to_string())?;
    let (image, image_bytes) = build_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &output,
        &BuildInputs {
            source_identity: spore_target::BROWSER_RELEASE_SOURCE_ID.into(),
            toolchain_available: true,
        },
    )
    .map_err(|errors| format!("select reviewed prebuilt IMAGE: {errors:?}"))?;
    let spore = seal_prebuilt_body_spore_with_content_digest(
        &construction,
        &checked.configuration().name,
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
