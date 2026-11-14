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
    pins: Option<&Path>,
    retained_member: Option<&Path>,
    output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let package = bounded(package, 64 * 1024 * 1024)?;
    let exported: DeploymentRealizationReceipt =
        serde_json::from_slice(&bounded(exported, 8192)?.0)?;
    if pins.is_some() == retained_member.is_some() {
        return Err("Seed preparation requires exactly one membership input".into());
    }
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
    validate_package(&verified)?;
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
        let (host, entrance) = if let Some(input) = retained_member {
            use super::host_rp2040_membership_seed::{
                compose_retained_private_seed, PreparedMembershipBasis,
            };
            let basis = compose_retained_private_seed(
                verified.configuration(),
                &verified.image().manifest.image_id,
                &verified.spore().body_id,
                input,
                &output.join("bootstrap.uf2"),
            )?;
            let PreparedMembershipBasis::Retained(basis) = basis else {
                return Err("retained preparation cannot become first admission".into());
            };
            publish(
                &output.join("usb-return.json"),
                &serde_json::to_vec_pretty(&basis)?,
            )?;
            (basis.credential.host_id, "retained")
        } else {
            let pins: Pins =
                serde_json::from_slice(&bounded(pins.ok_or("missing physical pins")?, 4096)?.0)?;
            let (bootstrap, basis) = compose(&verified, &pins)?;
            publish(&output.join("bootstrap.uf2"), &bootstrap.0)?;
            publish(
                &output.join("usb-admission.json"),
                &serde_json::to_vec_pretty(&basis)?,
            )?;
            (basis.expected_host, "spawn")
        };
        publish(
            &output.join("preparation.json"),
            &serde_json::to_vec_pretty(&serde_json::json!({
                "schema":"conduit.rp2040/seed-installation-preparation@1",
                "spore_id":verified.spore().spore_id,"image_id":verified.image().manifest.image_id,
                "configuration_id":verified.configuration().configuration_id(),
                "source_identity":verified.image().manifest.source_identity,
                "physical_host":host,"membership_entrance":entrance,"application_bytes":payload.len(),
                "bootstrap_bytes":fs::metadata(output.join("bootstrap.uf2"))?.len(),"flash_written":false,"boot_observed":false,
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
fn application_files(files: &[conduit_host_make::HostReleaseFile]) -> bool {
    matches!(files, [app] if app.path == "host.uf2" && app.media_type == "application/x-uf2")
        || matches!(files, [app, recipe] if app.path == "host.uf2" && app.media_type == "application/x-uf2"
            && recipe.path == super::host_continuation_recipe::NAME && recipe.media_type == "application/json")
}
fn validate_package(package: &VerifiedJoiningHostPackage) -> Result<(), String> {
    let image = &package.image().manifest;
    let release = package.release();
    if image.target != conduit_host_rp2040::TARGET_ID
        || image.output != SporeOutputKind::Uf2
        || !application_files(&release.files)
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
    validate_package(package)?;
    let image = &package.image().manifest;
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
#[path = "host_rp2040_seed_tests.rs"]
mod tests;
