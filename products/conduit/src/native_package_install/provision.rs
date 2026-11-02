//! Exact private Seed provisioning and retained checked Host source.
use conduit_host_make::BodyBoundArtifactIdentity;
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct Provision {
    pub(super) schema: String,
    pub(super) spore: NativeSpore,
    pub(super) invitation_provision: InvitationProvision,
    #[serde(default)]
    pub(super) portable_invitation: Option<conduit_body::PortableInvitation>,
    #[serde(default)]
    pub(super) host_configuration_source: Option<String>,
    #[serde(default)]
    pub(super) host_image: Option<conduit_host_make::HostImage>,
    #[serde(default)]
    pub(super) release_manifest: Option<conduit_host_make::HostReleaseManifest>,
}

impl Drop for Provision {
    fn drop(&mut self) {
        self.invitation_provision.secret.fill(0);
        if let Some(invitation) = &mut self.portable_invitation {
            invitation.secret.fill(0);
        }
    }
}

#[derive(Deserialize)]
pub(super) struct NativeSpore {
    pub(super) schema: String,
    pub(super) spore_id: String,
    pub(super) body_id: String,
    pub(super) binding: SporeBinding,
    pub(super) image_id: String,
    pub(super) image_content_digest: String,
    pub(super) target: String,
    pub(super) output: String,
    pub(super) make: MakeSelection,
    pub(super) source_identity: String,
    pub(super) host_configuration_id: String,
    pub(super) profile_id: String,
    pub(super) build_id: String,
}

#[derive(Deserialize)]
pub(super) struct SporeBinding {
    pub(super) mode: String,
    pub(super) invitation_id: String,
}

#[derive(Deserialize)]
pub(super) struct MakeSelection {
    pub(super) make_package_id: String,
    pub(super) builder_adapter: String,
    pub(super) deployment_adapter: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct InvitationProvision {
    pub(super) invitation_id: String,
    pub(super) nonce: Vec<u8>,
    pub(super) secret: Vec<u8>,
    pub(super) expires_at_millis: u64,
    #[serde(default)]
    pub(super) rendezvous: Option<conduit_body::SpawnRendezvousDescriptor>,
}

pub(super) fn validate_provision(
    provision: &Provision,
    artifact: &BodyBoundArtifactIdentity,
) -> Result<SelectedConfiguration, String> {
    let spore = &provision.spore;
    let invitation = &provision.invitation_provision;
    let portable = provision
        .portable_invitation
        .as_ref()
        .ok_or("missing original portable invitation")?;
    portable.validate(0).map_err(|error| error.to_string())?;
    if portable.claim.body_id.as_str() != spore.body_id
        || portable.claim.invitation_id.as_str() != invitation.invitation_id
        || portable.claim.nonce.as_slice() != invitation.nonce
        || portable.claim.expires_at_millis != invitation.expires_at_millis
        || portable.secret.as_slice() != invitation.secret
        || portable.rendezvous != invitation.rendezvous
    {
        return Err("portable invitation differs from exact package binding".into());
    }

    if provision.schema != "conduit.spore/native-package-provision@1"
        || spore.schema != "conduit.body/spore-manifest@2"
        || spore.spore_id != artifact.spore_id
        || spore.image_id != artifact.image_id
        || spore.image_content_digest != artifact.image_content_sha256
        || spore.target != artifact.target_id
        || spore.output != "native-bundle"
        || spore.body_id.is_empty()
        || spore.binding.mode != "self-joining"
        || spore.binding.invitation_id != invitation.invitation_id
        || invitation.nonce.len() != 32
        || invitation.nonce.iter().all(|byte| *byte == 0)
        || invitation.secret.len() != 32
        || invitation.secret.iter().all(|byte| *byte == 0)
        || invitation.expires_at_millis == 0
        || spore.source_identity.is_empty()
    {
        return Err(
            "native package provision lost its exact body, spore, or IMAGE identity".into(),
        );
    }
    checked_source(provision)
}

#[derive(Debug)]
pub(super) struct SelectedConfiguration {
    pub(super) source: String,
    pub(super) configuration_id: String,
    pub(super) image: conduit_host_make::HostImage,
}

fn checked_source(provision: &Provision) -> Result<SelectedConfiguration, String> {
    let source = provision
        .host_configuration_source
        .as_deref()
        .ok_or("Seed lacks selected Host source")?;
    if source.len() > 16 * 1024 {
        return Err("Seed Host source exceeds retained bound".into());
    }
    let image = provision
        .host_image
        .as_ref()
        .ok_or("Seed lacks actual selected IMAGE")?;
    let checked = crate::durable_host::check_seed_host_image(source, image)?;
    let canonical =
        conduit_host_make::canonical_host_configuration_conduit(checked.configuration())
            .map_err(|_| "Seed Host source cannot be canonicalized")?;
    let manifest = &image.manifest;
    let release = provision
        .release_manifest
        .as_ref()
        .ok_or("Seed lacks selected Make release")?;
    if canonical != source
        || checked.configuration_id() != provision.spore.host_configuration_id
        || manifest.profile_id != provision.spore.profile_id
        || manifest.image_id != provision.spore.image_id
        || manifest.build_id != provision.spore.build_id
        || manifest.source_identity != release.source_identity
        || manifest.make_package_id != release.make_package_id
        || manifest.builder_adapter != release.builder_adapter
        || manifest.deployment_adapter.as_deref() != Some(release.deployment_adapter.as_str())
        || manifest.output != conduit_host_make::SporeOutputKind::NativeBundle
        || checked.profile().target.key() != provision.spore.target
    {
        return Err("Seed source differs from selected configuration/profile/target".into());
    }
    Ok(SelectedConfiguration {
        source: canonical,
        configuration_id: checked.configuration_id().into(),
        image: image.clone(),
    })
}
