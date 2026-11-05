//! Private target Seed composition through the existing checked Make build.
use conduit_body::{FederationPeer, MemberBootstrapMaterial, SpawnInvitationSecret};
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
    physical_uid: [u8; 8],
    pinned_owner: FederationPeer,
    #[serde(borrow)]
    admission: MemberBootstrapMaterial<'a>,
}
pub(super) enum PreparedMembershipBasis {
    Spawn(PicoSpawnAdmissionBasis),
    Retained(PicoMemberReturnBasis),
}
impl PreparedMembershipBasis {
    pub(super) fn publish(&self, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let (name, bytes) = match self {
            Self::Spawn(basis) => ("usb-admission.json", serde_json::to_vec_pretty(basis)?),
            Self::Retained(basis) => ("usb-return.json", serde_json::to_vec_pretty(basis)?),
        };
        fs::write(output.join(name), bytes)?;
        Ok(())
    }
}
struct PrivateBytes(Vec<u8>);
impl Drop for PrivateBytes {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}
fn wipe(bytes: &mut [u8]) {
    for byte in bytes {
        unsafe { core::ptr::write_volatile(byte, 0) }
    }
}
pub(super) fn compose_private_seed(
    checked: &CheckedHostConfiguration,
    image: &str,
    input: &Path,
    destination: &Path,
) -> Result<PreparedMembershipBasis, Box<dyn std::error::Error>> {
    compose_scoped_private_seed(checked, image, input, destination, None)
}

pub(super) fn compose_retained_private_seed(
    checked: &CheckedHostConfiguration,
    image: &str,
    body: &str,
    input: &Path,
    destination: &Path,
) -> Result<PreparedMembershipBasis, Box<dyn std::error::Error>> {
    compose_scoped_private_seed(checked, image, input, destination, Some(body))
}

fn compose_scoped_private_seed(
    checked: &CheckedHostConfiguration,
    image: &str,
    input: &Path,
    destination: &Path,
    retained_body: Option<&str>,
) -> Result<PreparedMembershipBasis, Box<dyn std::error::Error>> {
    let mut bytes = PrivateBytes(Vec::new());
    fs::File::open(input)?
        .take(4097)
        .read_to_end(&mut bytes.0)?;
    if bytes.0.len() > 4096 {
        return Err("private Seed input exceeds finite bound".into());
    }
    let input: PrivateInput<'_> =
        serde_json::from_slice(&bytes.0).map_err(|_| "invalid private Seed input")?;
    if let Some(body) = retained_body {
        match &input.admission {
            MemberBootstrapMaterial::Retained { credential, .. }
                if credential.body_id.as_str() == body => {}
            _ => return Err("Seed update requires retained membership of the same Body".into()),
        }
    }
    let (mut sector, basis) = compose_material(checked, image, &input)?;
    let private = PrivateBytes(conduit_host_rp2040::membership_seed_sector_uf2(&sector));
    wipe(&mut sector);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(destination)
        .map_err(|_| "private Seed publication refused")?;
    file.write_all(&private.0)?;
    file.sync_all()?;
    Ok(basis)
}
fn compose_material(
    checked: &CheckedHostConfiguration,
    image: &str,
    input: &PrivateInput<'_>,
) -> Result<([u8; 4096], PreparedMembershipBasis), Box<dyn std::error::Error>> {
    let key = SpawnInvitationSecret::from_csprng_bytes(*input.admission.secret())
        .map_err(|_| "private Seed member key refused")?;
    match &input.admission {
        MemberBootstrapMaterial::Retained { credential, secret } => {
            let basis = PicoMemberReturnBasis {
                credential: credential.clone(),
                expected_profile: checked.configuration_id().into(),
                member_verifying_key: key.verifying_key(),
                owner: input.pinned_owner.clone(),
            };
            basis
                .validate()
                .map_err(|e| format!("return basis: {e:?}"))?;
            let sector = conduit_host_rp2040::encode_retained_membership_seed_sector(
                checked,
                image,
                input.physical_uid,
                &input.pinned_owner,
                credential,
                *secret,
            )?;
            Ok((sector, PreparedMembershipBasis::Retained(basis)))
        }
        MemberBootstrapMaterial::Spawn { provision } => {
            let basis = PicoSpawnAdmissionBasis {
                spore_id: provision.spore_id.into(),
                image_id: provision.image_id.into(),
                invitation: serde_json::from_value(
                    serde_json::json!({"invitation_id":provision.invitation_id,
                    "body_id":provision.body_id,"nonce":provision.nonce,"expires_at_millis":provision.expires_at_millis}),
                )?,
                expected_host: format!(
                    "rp2040/flash/{}",
                    input
                        .physical_uid
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                )
                .into(),
                expected_profile: checked.configuration_id().into(),
                member_verifying_key: key.verifying_key(),
                owner: input.pinned_owner.clone(),
            };
            let sector = conduit_host_rp2040::encode_spawn_membership_seed_sector(
                checked,
                image,
                input.physical_uid,
                &input.pinned_owner,
                provision,
            )?;
            Ok((sector, PreparedMembershipBasis::Spawn(basis)))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_make_input_has_no_invitation_and_cannot_change_the_physical_host() {
        let checked = super::super::load_configuration(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../targets/rp2040/profiles/pico-w-house-green.host.conduit"),
        )
        .unwrap();
        let key = SpawnInvitationSecret::from_csprng_bytes([9; 32]).unwrap();
        let mut value = serde_json::json!({"physical_uid":[1,2,3,4,5,6,7,8],
            "pinned_owner":{"host_id":"owner/1","boot_id":"owner/boot","offer_generation":1,"verifying_key":key.verifying_key()},
            "admission":{"retained":{"credential":{"credential_id":"credential/1","body_id":"body/house","part_id":"part/1","host_id":"rp2040/flash/0102030405060708","boot_id":"boot/old","issued_at_millis":1},"secret":([9u8;32])}}});
        let bytes = serde_json::to_vec(&value).unwrap();
        let input: PrivateInput<'_> = serde_json::from_slice(&bytes).unwrap();
        let (sector, basis) = compose_material(&checked, "image/new", &input).unwrap();
        let PreparedMembershipBasis::Retained(basis) = basis else {
            panic!("retained input cannot become spawn")
        };
        assert_eq!(basis.credential.part_id.as_str(), "part/1");
        assert_eq!(basis.expected_profile.as_str(), checked.configuration_id());
        assert_eq!(&sector[..16], conduit_host_rp2040::MEMBERSHIP_SEED_MAGIC);
        value["physical_uid"] = serde_json::json!([8, 7, 6, 5, 4, 3, 2, 1]);
        let bytes = serde_json::to_vec(&value).unwrap();
        let input: PrivateInput<'_> = serde_json::from_slice(&bytes).unwrap();
        assert!(compose_material(&checked, "image/new", &input).is_err());
    }
}
