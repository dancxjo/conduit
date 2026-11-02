//! Private immutable native Seed composition, separate from public firmware.
use conduit_body::{
    validate_pico_spawn_provision, FederationPeer, MembershipCredential, PicoSpawnProvision,
};
use conduit_host_make::CheckedHostConfiguration;
use serde::Serialize;
pub const MEMBERSHIP_SEED_MAGIC: &[u8; 16] = b"CONDUIT_SEED@1\0\0";
pub const MEMBERSHIP_SEED_SCHEMA: &str = "conduit.body/embedded-membership-seed@1";
const BYTES: usize = conduit_rp2040_flash_layout::BOOTSTRAP.bytes() as usize;
const HEADER: usize = 20;
#[derive(Serialize)]
struct PrivateSeed<'a, 'b> {
    schema: &'static str,
    body_id: &'a str,
    host_id: &'a str,
    configuration_id: &'a str,
    owner: &'a FederationPeer,
    spawn: &'a PicoSpawnProvision<'b>,
    credential: Option<&'a MembershipCredential>,
}

/// This intentionally returns private bytes without a Debug/log representation.
/// The caller separately reviews any write to the reserved bootstrap sector.
/// Application-only updates must leave this sector and credentials untouched.
pub fn encode_membership_seed_sector(
    checked: &CheckedHostConfiguration,
    physical_uid: [u8; 8],
    pinned_owner: &FederationPeer,
    provision: &PicoSpawnProvision<'_>,
    credential: Option<&MembershipCredential>,
) -> Result<[u8; BYTES], String> {
    if checked.configuration().target.machine != "pico-w"
        || physical_uid == [0; 8]
        || physical_uid == [255; 8]
        || !validate_pico_spawn_provision(provision)
        || pinned_owner.verifying_key == [0; 32]
        || pinned_owner.host_id.as_str().is_empty()
        || pinned_owner.boot_id.as_str().is_empty()
    {
        return Err("private native Seed requires exact physical target, invitation and independent owner pin".into());
    }
    let host_id = format!(
        "rp2040/flash/{}",
        physical_uid
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    if credential.is_some_and(|credential| {
        credential.host_id.as_str() != host_id || credential.body_id.as_str() != provision.body_id
    }) {
        return Err("continuation credential differs from exact physical Host or Body".into());
    }
    let seed = PrivateSeed {
        schema: MEMBERSHIP_SEED_SCHEMA,
        body_id: provision.body_id,
        host_id: &host_id,
        configuration_id: checked.configuration_id(),
        owner: pinned_owner,
        spawn: provision,
        credential,
    };
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
    for (index, payload) in sector.chunks_exact(PAGE).enumerate() {
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

    #[test]
    fn private_seed_is_exact_physical_source_and_only_bootstrap_pages() {
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
        let sector = encode_membership_seed_sector(
            &checked,
            [1, 2, 3, 4, 5, 6, 7, 8],
            &owner,
            &provision,
            None,
        )
        .unwrap();
        assert_eq!(&sector[..16], MEMBERSHIP_SEED_MAGIC);
        let length = u32::from_le_bytes(sector[16..20].try_into().unwrap()) as usize;
        let value: serde_json::Value = serde_json::from_slice(&sector[20..20 + length]).unwrap();
        assert_eq!(value["host_id"], "rp2040/flash/0102030405060708");
        assert_eq!(value["configuration_id"], checked.configuration_id());
        assert_eq!(
            value["spawn"]["secret"],
            serde_json::to_value([9; 32]).unwrap()
        );
        assert!(encode_membership_seed_sector(&checked, [0; 8], &owner, &provision, None).is_err());
        let uf2 = membership_seed_sector_uf2(&sector);
        let mut validator = Uf2Validator::new(uf2.len() as u64).unwrap();
        for block in uf2.chunks_exact(UF2_BLOCK_BYTES) {
            let block: &[u8; UF2_BLOCK_BYTES] = block.try_into().unwrap();
            validator.push(block).unwrap();
            let address = u32::from_le_bytes(block[12..16].try_into().unwrap());
            assert!(BOOTSTRAP.contains(address - conduit_rp2040_flash_layout::FLASH_START, 256));
        }
        validator.finish().unwrap();
    }
}
