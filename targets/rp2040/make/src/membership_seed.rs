//! Private immutable native Seed composition, separate from public firmware.
use conduit_body::{
    validate_pico_spawn_provision, BodyId, FederationPeer, MemberBootstrap,
    MemberBootstrapMaterial, MembershipCredential, PicoSpawnProvision, MEMBER_BOOTSTRAP_SCHEMA,
};
use conduit_host_make::CheckedHostConfiguration;
pub const MEMBERSHIP_SEED_MAGIC: &[u8; 16] = b"CONDUIT_SEED@2\0\0";
pub const MEMBERSHIP_SEED_SCHEMA: &str = MEMBER_BOOTSTRAP_SCHEMA;
const BYTES: usize = conduit_rp2040_flash_layout::BOOTSTRAP.bytes() as usize;
const HEADER: usize = 20;
/// This intentionally returns private bytes without a Debug/log representation.
/// The caller separately reviews any write to the reserved bootstrap sector.
/// Application-only updates must leave this sector and credentials untouched.
pub fn encode_spawn_membership_seed_sector(
    checked: &CheckedHostConfiguration,
    expected_image_id: &str,
    physical_uid: [u8; 8],
    pinned_owner: &FederationPeer,
    provision: &PicoSpawnProvision<'_>,
) -> Result<[u8; BYTES], String> {
    if !validate_pico_spawn_provision(provision) || provision.image_id != expected_image_id {
        return Err("fresh native Seed requires an exact reviewed invitation image".into());
    }
    let material = MemberBootstrapMaterial::Spawn {
        provision: PicoSpawnProvision {
            protocol: provision.protocol,
            spore_id: provision.spore_id,
            image_id: provision.image_id,
            invitation_id: provision.invitation_id,
            body_id: provision.body_id,
            nonce: provision.nonce,
            expires_at_millis: provision.expires_at_millis,
            secret: provision.secret,
        },
    };
    encode_material(
        checked,
        expected_image_id,
        physical_uid,
        pinned_owner,
        serde_json::from_value(serde_json::Value::String(provision.body_id.into()))
            .map_err(|_| "native Seed Body identity refused")?,
        material,
    )
}

/// Rebuild a member's native Seed without creating invitation or Part authority.
/// The current owner must still acknowledge the canonical retained-member return.
pub fn encode_retained_membership_seed_sector(
    checked: &CheckedHostConfiguration,
    expected_image_id: &str,
    physical_uid: [u8; 8],
    pinned_owner: &FederationPeer,
    credential: &MembershipCredential,
    secret: [u8; 32],
) -> Result<[u8; BYTES], String> {
    encode_material(
        checked,
        expected_image_id,
        physical_uid,
        pinned_owner,
        credential.body_id.clone(),
        MemberBootstrapMaterial::Retained {
            credential: credential.clone(),
            secret,
        },
    )
}
fn encode_material(
    checked: &CheckedHostConfiguration,
    expected_image_id: &str,
    physical_uid: [u8; 8],
    pinned_owner: &FederationPeer,
    body_id: BodyId,
    admission: MemberBootstrapMaterial<'_>,
) -> Result<[u8; BYTES], String> {
    if checked.configuration().target.machine != "pico-w"
        || physical_uid == [0; 8]
        || physical_uid == [255; 8]
    {
        return Err("private native Seed requires exact physical target".into());
    }
    let host_id = format!(
        "rp2040/flash/{}",
        physical_uid
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let seed = MemberBootstrap {
        schema: MEMBERSHIP_SEED_SCHEMA.into(),
        body_id,
        host_id,
        image_id: expected_image_id.into(),
        configuration_id: checked.configuration_id().into(),
        owner: pinned_owner.clone(),
        admission,
    };
    if !seed.matches_native(&seed.host_id, expected_image_id, checked.configuration_id()) {
        return Err("native membership Seed scope or private material refused".into());
    }
    let mut bytes = serde_json::to_vec(&seed).map_err(|_| "private Seed encoding refused")?;
    if bytes.is_empty() || bytes.len() > BYTES - HEADER {
        wipe(&mut bytes);
        return Err("private Seed exceeds reserved sector".into());
    }
    let mut sector = [255; BYTES];
    sector[..16].copy_from_slice(MEMBERSHIP_SEED_MAGIC);
    sector[16..HEADER].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
    sector[HEADER..HEADER + bytes.len()].copy_from_slice(&bytes);
    wipe(&mut bytes);
    Ok(sector)
}
fn wipe(bytes: &mut [u8]) {
    for byte in bytes {
        unsafe { core::ptr::write_volatile(byte, 0) }
    }
}

/// Wrap only the reviewed private bootstrap sector; no application/credential pages.
pub fn membership_seed_sector_uf2(sector: &[u8; BYTES]) -> Vec<u8> {
    const PAGE: usize = 256;
    const BLOCK: usize = 512;
    let count = BYTES / PAGE;
    let mut out = vec![0; count * BLOCK];
    for (index, payload) in sector.as_chunks::<PAGE>().0.iter().enumerate() {
        let block = &mut out[index * BLOCK..(index + 1) * BLOCK];
        for (at, value) in [
            (0, 0x0a324655),
            (4, 0x9e5d5157),
            (8, 0x2000),
            (
                12,
                conduit_rp2040_flash_layout::BOOTSTRAP.address() + (index * PAGE) as u32,
            ),
            (16, PAGE as u32),
            (20, index as u32),
            (24, count as u32),
            (28, 0xe48bff56),
            (508, 0x0ab16f30),
        ] {
            block[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        block[32..32 + PAGE].copy_from_slice(payload);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_make::{
        check_host_configuration, parse_host_configuration_conduit, MakeCatalog, MakePackageSet,
    };
    use conduit_rp2040_flash_layout::{Uf2Validator, BOOTSTRAP, UF2_BLOCK_BYTES};

    fn fixture() -> (CheckedHostConfiguration, FederationPeer) {
        let packages = MakePackageSet::compose(&[&crate::Rp2040MakePackage]).unwrap();
        let configuration = parse_host_configuration_conduit(include_str!(
            "../../profiles/pico-w-house-green.host.conduit"
        ))
        .unwrap();
        let checked = check_host_configuration(
            configuration,
            &MakeCatalog::canonical().with_packages(&packages),
            &packages,
        )
        .unwrap();
        let owner:FederationPeer=serde_json::from_str(r#"{"host_id":"owner/1","boot_id":"boot/1","offer_generation":1,"verifying_key":[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1]}"#).unwrap();
        (checked, owner)
    }
    #[test]
    fn retained_seed_uses_the_actual_embedded_decoder_without_new_invitation() {
        let (checked, owner) = fixture();
        let credential: MembershipCredential = serde_json::from_str(
            r#"{
            "credential_id":"credential/1","body_id":"body/house","part_id":"part/1",
            "host_id":"rp2040/flash/0102030405060708","boot_id":"boot/old","issued_at_millis":7
        }"#,
        )
        .unwrap();
        let sector = encode_retained_membership_seed_sector(
            &checked,
            "image/new",
            [1, 2, 3, 4, 5, 6, 7, 8],
            &owner,
            &credential,
            [9; 32],
        )
        .unwrap();
        let length = u32::from_le_bytes(sector[16..20].try_into().unwrap()) as usize;
        let (decoded, used) =
            serde_json_core::from_slice::<MemberBootstrap<'_>>(&sector[20..20 + length]).unwrap();
        assert_eq!(used, length);
        assert!(decoded.matches_native(
            "rp2040/flash/0102030405060708",
            "image/new",
            checked.configuration_id()
        ));
        assert!(decoded.admission.spawn().is_none());
        assert_eq!(decoded.admission.credential(), Some(&credential));
        assert_eq!(decoded.admission.secret(), &[9; 32]);
        assert!(encode_retained_membership_seed_sector(
            &checked,
            "image/new",
            [8, 7, 6, 5, 4, 3, 2, 1],
            &owner,
            &credential,
            [9; 32]
        )
        .is_err());
    }
    #[test]
    fn private_seed_is_exact_physical_source_and_only_bootstrap_pages() {
        let (checked, owner) = fixture();
        let provision = PicoSpawnProvision {
            protocol: 2,
            spore_id: "spore/1",
            image_id: "image/1",
            invitation_id: "invitation/1",
            body_id: "body/house",
            nonce: [3; 32],
            expires_at_millis: 50,
            secret: [9; 32],
        };
        let sector = encode_spawn_membership_seed_sector(
            &checked,
            "image/1",
            [1, 2, 3, 4, 5, 6, 7, 8],
            &owner,
            &provision,
        )
        .unwrap();
        assert_eq!(&sector[..16], MEMBERSHIP_SEED_MAGIC);
        let length = u32::from_le_bytes(sector[16..20].try_into().unwrap()) as usize;
        let value: serde_json::Value = serde_json::from_slice(&sector[20..20 + length]).unwrap();
        assert_eq!(value["host_id"], "rp2040/flash/0102030405060708");
        assert_eq!(value["configuration_id"], checked.configuration_id());
        let (decoded, used) =
            serde_json_core::from_slice::<MemberBootstrap<'_>>(&sector[20..20 + length]).unwrap();
        assert_eq!(used, length);
        assert!(decoded.matches_native(
            "rp2040/flash/0102030405060708",
            "image/1",
            checked.configuration_id()
        ));
        assert!(decoded.admission.credential().is_none());
        assert_eq!(
            value["admission"]["spawn"]["provision"]["secret"],
            serde_json::to_value([9; 32]).unwrap()
        );
        assert!(encode_spawn_membership_seed_sector(
            &checked, "image/1", [0; 8], &owner, &provision
        )
        .is_err());
        assert!(encode_spawn_membership_seed_sector(
            &checked,
            "other-image",
            [1, 2, 3, 4, 5, 6, 7, 8],
            &owner,
            &provision
        )
        .is_err());
        let uf2 = membership_seed_sector_uf2(&sector);
        let mut validator = Uf2Validator::new(uf2.len() as u64).unwrap();
        for block in uf2.as_chunks::<UF2_BLOCK_BYTES>().0 {
            validator.push(block).unwrap();
            let address = u32::from_le_bytes(block[12..16].try_into().unwrap());
            assert!(BOOTSTRAP.contains(address - conduit_rp2040_flash_layout::FLASH_START, 256));
        }
        validator.finish().unwrap();
    }
}
