//! Target installation preparation from an exported Body-produced Seed.
//! No new compilation, invitation, configuration or admission is synthesized.
use conduit_body::{FederationPeer, PicoSpawnProvision, SpawnInvitationSecret};
use conduit_body_make::{verify_joining_host_package, VerifiedJoiningHostPackage};
use conduit_host_make::{DeploymentRealizationReceipt, SporeOutputKind};
use serde::Deserialize;
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pins {
    physical_uid: [u8; 8],
    pinned_owner: FederationPeer,
}
struct PrivateBytes(Vec<u8>);
impl Drop for PrivateBytes {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
fn bounded(path: &Path, maximum: u64) -> Result<PrivateBytes, Box<dyn std::error::Error>> {
    let mut bytes = PrivateBytes(Vec::new());
    fs::File::open(path)?
        .take(maximum + 1)
        .read_to_end(&mut bytes.0)?;
    if bytes.0.len() as u64 > maximum {
        return Err("Seed installation input exceeds bound".into());
    }
    Ok(bytes)
}

pub(super) fn prepare(
    package: &Path,
    exported: &Path,
    pins: &Path,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let package = bounded(package, 64 * 1024 * 1024)?;
    let exported: DeploymentRealizationReceipt =
        serde_json::from_slice(&bounded(exported, 8192)?.0)?;
    let pins: Pins = serde_json::from_slice(&bounded(pins, 4096)?.0)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis()
        .try_into()?;
    let verified = verify_joining_host_package(
        &package.0,
        &exported,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        now,
    )?;
    let (bootstrap, basis) = compose(&verified, &pins)?;
    let payload = verified
        .payload("host.uf2")
        .ok_or("RP2040 Seed has no application UF2")?;
    // The existing geometry checker refuses credential/bootstrap writes in app bytes.
    super::host_rp2040::validate_application_uf2(payload)?;
    let mut directory = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory.create(output)?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        publish(&output.join("application.uf2"), payload)?;
        publish(&output.join("bootstrap.uf2"), &bootstrap.0)?;
        publish(
            &output.join("usb-admission.json"),
            &serde_json::to_vec_pretty(&basis)?,
        )?;
        publish(
            &output.join("preparation.json"),
            &serde_json::to_vec_pretty(&serde_json::json!({
                "schema":"conduit.rp2040/seed-installation-preparation@1",
                "spore_id":verified.spore().spore_id,"image_id":verified.image().manifest.image_id,
                "configuration_id":verified.configuration().configuration_id(),
                "source_identity":verified.image().manifest.source_identity,
                "physical_host":basis.expected_host,"application_bytes":payload.len(),
                "bootstrap_bytes":bootstrap.0.len(),"flash_written":false,"boot_observed":false,
                "membership_admitted":false,"play_created":false
            }))?,
        )?;
        fs::File::open(output)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(output);
    }
    result?;
    println!("Prepared exact application, private bootstrap and public admission pins in {}. No flash write or admission performed.", output.display());
    Ok(())
}
fn compose(
    package: &VerifiedJoiningHostPackage,
    pins: &Pins,
) -> Result<
    (
        PrivateBytes,
        conduit_std_host::pico_spawn::PicoSpawnAdmissionBasis,
    ),
    String,
> {
    let image = &package.image().manifest;
    let release = package.release();
    if image.target != conduit_host_rp2040::TARGET_ID
        || image.output != SporeOutputKind::Uf2
        || release.files.len() != 1
        || release.files[0].path != "host.uf2"
        || release.files[0].media_type != "application/x-uf2"
    {
        return Err("Seed is not the reviewed RP2040 application closure".into());
    }
    let support = release
        .target_metadata
        .get(conduit_host_rp2040::BOOTSTRAP_CONTRACT_METADATA)
        .ok_or("reviewed firmware has no bootstrap codec contract")?;
    let support: conduit_host_rp2040::BootstrapContract =
        serde_json::from_value(support.clone())
            .map_err(|_| "reviewed bootstrap contract encoding refused")?;
    if !support.matches_image(&image.image_id, package.configuration().configuration_id()) {
        return Err("reviewed firmware/bootstrap codec or IMAGE differs".into());
    }
    let invitation = package.invitation();
    let provision = PicoSpawnProvision {
        protocol: 2,
        spore_id: &package.spore().spore_id,
        image_id: &image.image_id,
        invitation_id: invitation.claim.invitation_id.as_str(),
        body_id: invitation.claim.body_id.as_str(),
        nonce: invitation.claim.nonce,
        expires_at_millis: invitation.claim.expires_at_millis,
        secret: invitation.secret,
    };
    let mut sector = conduit_host_rp2040::encode_spawn_membership_seed_sector(
        package.configuration(),
        &image.image_id,
        pins.physical_uid,
        &pins.pinned_owner,
        &provision,
    )?;
    let bootstrap = PrivateBytes(conduit_host_rp2040::membership_seed_sector_uf2(&sector));
    sector.fill(0);
    let key = SpawnInvitationSecret::from_csprng_bytes(invitation.secret)
        .map_err(|_| "Seed member signing key refused")?;
    let basis = conduit_std_host::pico_spawn::PicoSpawnAdmissionBasis {
        spore_id: package.spore().spore_id.clone(),
        image_id: image.image_id.clone(),
        invitation: invitation.claim.clone(),
        expected_host: format!(
            "rp2040/flash/{}",
            pins.physical_uid
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        )
        .into(),
        expected_profile: package.configuration().configuration_id().into(),
        member_verifying_key: key.verifying_key(),
        owner: pins.pinned_owner.clone(),
    };
    Ok((bootstrap, basis))
}
fn publish(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../../make/body/tests/common/joining_host_package.rs"]
mod fixture;

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_make::{
        check_host_configuration, parse_host_configuration_conduit, CarrierTerminal,
    };
    use std::io::Cursor;
    #[test]
    fn exported_package_composes_only_exact_private_bootstrap_and_public_basis() {
        let catalog = conduit_workspace_make::catalog();
        let packages = conduit_workspace_make::package_set();
        let checked = check_host_configuration(
            parse_host_configuration_conduit(include_str!(
                "../../../../targets/rp2040/profiles/pico-w-house-green.host.conduit"
            ))
            .unwrap(),
            &catalog,
            &packages,
        )
        .unwrap();
        let (release, spore, invitation, payloads) =
            fixture::fixture_with_output_at(&["host.uf2"], SporeOutputKind::Uf2, 0, &checked);
        let mut bytes = Vec::new();
        let receipt = conduit_body_make::write_joining_host_package(
            &release,
            &spore,
            &checked,
            &invitation,
            1,
            |name| Ok(Cursor::new(payloads[name].as_slice())),
            &mut bytes,
            || false,
        )
        .unwrap();
        let exported = DeploymentRealizationReceipt {
            schema: "conduit.carrier/realization@1".into(),
            carrier_id: "fixture/download".into(),
            implementation_id: "conduit/download@1".into(),
            target_id: spore.manifest.target.clone(),
            spore_id: spore.manifest.spore_id.clone(),
            artifact_content_sha256: receipt.artifact_content_sha256,
            terminal: CarrierTerminal::Completed,
            bytes_realized: bytes.len() as u64,
            byte_verification_completed: true,
            explicit_authority_observed: false,
            boot_observed: false,
            join_observed: false,
            membership_admitted: false,
        };
        let verified =
            verify_joining_host_package(&bytes, &exported, &catalog, &packages, 1).unwrap();
        let mut pins = Pins {
            physical_uid: [1, 2, 3, 4, 5, 6, 7, 8],
            pinned_owner: FederationPeer {
                host_id: "fixture/owner".into(),
                boot_id: "fixture/owner/boot".into(),
                offer_generation: conduit_core::OfferGeneration(1),
                verifying_key: [9; 32],
            },
        };
        let (bootstrap, basis) = compose(&verified, &pins).unwrap();
        assert_eq!(
            basis.expected_host.as_str(),
            "rp2040/flash/0102030405060708"
        );
        assert_eq!(basis.expected_profile.as_str(), checked.configuration_id());
        assert_eq!(basis.invitation, invitation.claim);
        assert_eq!(basis.spore_id, spore.manifest.spore_id);
        let mut validator =
            conduit_rp2040_flash_layout::Uf2Validator::new(bootstrap.0.len() as u64).unwrap();
        for block in bootstrap.0.chunks_exact(512) {
            validator.push(block.try_into().unwrap()).unwrap();
            let address = u32::from_le_bytes(block[12..16].try_into().unwrap());
            assert_eq!(
                conduit_rp2040_flash_layout::classify_image_page(address, 256).unwrap(),
                conduit_rp2040_flash_layout::ImageRegion::BodyBootstrap
            );
        }
        validator.finish().unwrap();
        pins.physical_uid = [0; 8];
        assert!(compose(&verified, &pins).is_err());
        pins.physical_uid = [1; 8];
        pins.pinned_owner.verifying_key = [0; 32];
        assert!(compose(&verified, &pins).is_err());
    }
}
