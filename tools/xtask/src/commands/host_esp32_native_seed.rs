//! Private ESP32 bootstrap carrier, separate from every public firmware artifact.
use conduit_body::{
    FederationPeer, MemberBootstrap, MemberBootstrapMaterial, SpawnInvitationSecret,
    MEMBER_BOOTSTRAP_SCHEMA,
};
use conduit_host_make::CheckedHostConfiguration;
use conduit_std_host::pico_spawn::{PicoMemberReturnBasis, PicoSpawnAdmissionBasis};
use serde::Deserialize;
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrivateInput<'a> {
    physical_uid: [u8; 6],
    pinned_owner: FederationPeer,
    #[serde(borrow)]
    admission: MemberBootstrapMaterial<'a>,
}
struct PrivateBytes(Vec<u8>);
impl Drop for PrivateBytes {
    fn drop(&mut self) {
        for b in &mut self.0 {
            unsafe { core::ptr::write_volatile(b, 0) }
        }
    }
}
pub(super) fn compose(
    checked: &CheckedHostConfiguration,
    image: &str,
    input: &Path,
    destination: &Path,
    _output: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = PrivateBytes(Vec::new());
    fs::File::open(input)?
        .take(4097)
        .read_to_end(&mut bytes.0)?;
    if bytes.0.len() > 4096 {
        return Err("private input exceeds finite bound".into());
    }
    let input: PrivateInput<'_> =
        serde_json::from_slice(&bytes.0).map_err(|_| "invalid private bootstrap input")?;
    if input.physical_uid == [0; 6] || input.physical_uid == [255; 6] {
        return Err("actual factory UID required".into());
    }
    let host = format!(
        "esp32/efuse/{}",
        input
            .physical_uid
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let key = SpawnInvitationSecret::from_csprng_bytes(*input.admission.secret())
        .map_err(|_| "private member key refused")?;
    let (body, basis_name, basis) = match &input.admission {
        MemberBootstrapMaterial::Retained { credential, .. } => {
            let basis = PicoMemberReturnBasis {
                credential: credential.clone(),
                expected_profile: checked.configuration_id().into(),
                member_verifying_key: key.verifying_key(),
                owner: input.pinned_owner.clone(),
            };
            basis.validate().map_err(|_| "return basis refused")?;
            (
                credential.body_id.clone(),
                "usb-return.json",
                serde_json::to_vec_pretty(&basis)?,
            )
        }
        MemberBootstrapMaterial::Spawn { provision } => {
            let basis = PicoSpawnAdmissionBasis {
                spore_id: provision.spore_id.into(),
                image_id: provision.image_id.into(),
                invitation: serde_json::from_value(
                    serde_json::json!({"invitation_id":provision.invitation_id,"body_id":provision.body_id,"nonce":provision.nonce,"expires_at_millis":provision.expires_at_millis}),
                )?,
                expected_host: host.clone().into(),
                expected_profile: checked.configuration_id().into(),
                member_verifying_key: key.verifying_key(),
                owner: input.pinned_owner.clone(),
            };
            (
                serde_json::from_value(serde_json::Value::String(provision.body_id.into()))?,
                "usb-admission.json",
                serde_json::to_vec_pretty(&basis)?,
            )
        }
    };
    let bootstrap = MemberBootstrap {
        schema: MEMBER_BOOTSTRAP_SCHEMA.into(),
        body_id: body,
        host_id: host,
        image_id: image.into(),
        configuration_id: checked.configuration_id().into(),
        owner: input.pinned_owner,
        admission: input.admission,
    };
    if !bootstrap.matches_native(&bootstrap.host_id, image, checked.configuration_id()) {
        return Err("bootstrap native/image/body scope refused".into());
    }
    let payload = PrivateBytes(serde_json::to_vec(&bootstrap)?);
    if payload.0.len() > 4096 - 20 {
        return Err("bootstrap exceeds reserved carrier".into());
    }
    let mut sector = PrivateBytes(vec![0xff; 4096]);
    sector.0[..16].copy_from_slice(b"CONDUIT_SEED@2\0\0");
    sector.0[16..20].copy_from_slice(&(payload.0.len() as u32).to_le_bytes());
    sector.0[20..20 + payload.0.len()].copy_from_slice(&payload.0);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(destination)
        .map_err(|_| "private bootstrap publication refused")?;
    file.write_all(&sector.0)?;
    file.sync_all()?;
    // The owner transport basis includes the invitation nonce. Retain it beside
    // the private bootstrap, never in the public artifact directory.
    let basis_path = destination.with_extension(basis_name);
    let mut basis_options = fs::OpenOptions::new();
    basis_options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        basis_options.mode(0o600);
    }
    let mut basis_file = basis_options
        .open(basis_path)
        .map_err(|_| "private owner transport basis publication refused")?;
    basis_file.write_all(&basis)?;
    basis_file.sync_all()?;
    Ok(())
}
