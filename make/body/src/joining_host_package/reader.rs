//! Verified package input for target installers; this performs no deployment.
use super::*;
use conduit_host_make::{
    CarrierTerminal, DeploymentRealizationReceipt, HostImage, HostReleaseManifest,
};

#[derive(Deserialize)]
struct PackageProvision {
    schema: String,
    spore: SporeManifest,
    release_manifest: HostReleaseManifest,
    portable_invitation: PortableInvitation,
    host_configuration_source: String,
    host_image: HostImage,
    invitation_provision: LegacyInvitation,
}
#[derive(Deserialize)]
struct LegacyInvitation {
    invitation_id: String,
    nonce: [u8; 32],
    expires_at_millis: u64,
    secret: [u8; 32],
    rendezvous: Option<conduit_body::SpawnRendezvousDescriptor>,
}
impl Drop for PackageProvision {
    fn drop(&mut self) {
        self.portable_invitation.secret.fill(0);
        self.invitation_provision.secret.fill(0);
    }
}

/// A checked construction package, never proof of a deployed or admitted Host.
/// Private invitation material has no Debug/Serialize representation here.
pub struct VerifiedJoiningHostPackage {
    provision: PackageProvision,
    configuration: CheckedHostConfiguration,
    payloads: Vec<crate::JoiningZipEntry>,
}
impl VerifiedJoiningHostPackage {
    pub fn spore(&self) -> &SporeManifest {
        &self.provision.spore
    }
    pub fn image(&self) -> &HostImage {
        &self.provision.host_image
    }
    pub fn invitation(&self) -> &PortableInvitation {
        &self.provision.portable_invitation
    }
    pub fn configuration(&self) -> &CheckedHostConfiguration {
        &self.configuration
    }
    pub fn release(&self) -> &HostReleaseManifest {
        &self.provision.release_manifest
    }
    pub fn payload(&self, name: &str) -> Option<&[u8]> {
        self.payloads
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.bytes.as_slice())
    }
}

/// The independently selected exported-artifact identity pins the whole private
/// archive. Configuration and release bytes are then checked through ordinary
/// Make contracts, including the same binding check used by the producer.
pub fn verify_joining_host_package(
    bytes: &[u8],
    expected: &DeploymentRealizationReceipt,
    catalog: &conduit_host_make::MakeCatalog,
    packages: &conduit_host_make::MakePackageSet,
    now_millis: u64,
) -> Result<VerifiedJoiningHostPackage, String> {
    if expected.schema != "conduit.carrier/realization@1"
        || expected.terminal != CarrierTerminal::Completed
        || !expected.byte_verification_completed
        || bytes.len() as u64 != expected.bytes_realized
        || bytes.len() as u64 > MAX_BYTES
        || format!("sha256:{:x}", Sha256::digest(bytes)) != expected.artifact_content_sha256
    {
        return Err("Seed package differs from selected exported bytes".into());
    }
    let mut entries = crate::read_stored_joining_zip(bytes)?;
    let index = entries
        .iter()
        .position(|entry| entry.name == PROVISION)
        .ok_or("Seed provision missing")?;
    let provision_entry = entries.remove(index);
    if provision_entry.bytes.len() > 64 * 1024 {
        return Err("Seed provision exceeds bound".into());
    }
    let p: PackageProvision =
        serde_json::from_slice(&provision_entry.bytes).map_err(|_| "Seed provision malformed")?;
    let invitation = &p.portable_invitation;
    let legacy = &p.invitation_provision;
    if p.schema != "conduit.spore/native-package-provision@1"
        || p.spore.spore_id != expected.spore_id
        || p.spore.target != expected.target_id
        || p.release_manifest.files.len() != entries.len()
        || legacy.invitation_id != invitation.claim.invitation_id.as_str()
        || legacy.nonce != invitation.claim.nonce
        || legacy.secret != invitation.secret
        || legacy.expires_at_millis != invitation.claim.expires_at_millis
        || legacy.rendezvous != invitation.rendezvous
        || p.host_configuration_source.len() > 16 * 1024
    {
        return Err("Seed provision differs from selected construction or invitation".into());
    }
    let configuration =
        conduit_host_make::parse_host_configuration_conduit(&p.host_configuration_source)
            .map_err(|_| "Seed Host source malformed")?;
    let checked = conduit_host_make::check_host_configuration(configuration, catalog, packages)
        .map_err(|_| "Seed Host source refused")?;
    if canonical_host_configuration_conduit(checked.configuration())
        .map_err(|_| "Seed Source encoding")?
        != p.host_configuration_source
        || checked.configuration_id() != p.spore.host_configuration_id
    {
        return Err("Seed Host Source differs from canonical configuration".into());
    }
    conduit_host_make::verify_configuration_image(&checked, &p.host_image, catalog)?;
    let manifest_bytes =
        serde_json::to_vec(&p.release_manifest).map_err(|_| "Seed release encoding")?;
    let manifest = &p.release_manifest;
    let release = conduit_host_make::verify_host_release_bytes(
        &manifest_bytes,
        &conduit_host_make::ReleaseTargetProfile {
            target_id: &manifest.target_id,
            package_id: &manifest.make_package_id,
            output: &manifest.output,
            builder_adapter: &manifest.builder_adapter,
            deployment_adapter: &manifest.deployment_adapter,
        },
        &p.host_image.manifest.source_identity,
        conduit_host_make::HostReleaseBounds {
            maximum_manifest_bytes: 65536,
            maximum_files: MAX_FILES - 1,
            maximum_file_bytes: MAX_BYTES,
            maximum_bundle_bytes: MAX_BYTES,
        },
        |name| {
            entries
                .iter()
                .find(|entry| entry.name == name)
                .map(|entry| std::io::Cursor::new(entry.bytes.as_slice()))
                .ok_or_else(|| "Seed release payload absent".into())
        },
        || false,
    )
    .map_err(|_| "Seed release bytes refused")?;
    check_binding(
        &release,
        &BuiltSpore {
            manifest: p.spore.clone(),
            image: p.host_image.clone(),
            image_bytes: serde_json::to_vec(&p.host_image).map_err(|_| "Seed IMAGE encoding")?,
        },
        invitation,
        now_millis,
    )?;
    Ok(VerifiedJoiningHostPackage {
        provision: p,
        configuration: checked,
        payloads: entries,
    })
}
