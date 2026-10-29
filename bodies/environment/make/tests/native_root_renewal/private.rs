//! Opt-in private flash-copy bootstrap. Prints no private values and never
//! modifies its input capture; the ordinary journal owns only the RAM copy.
use super::*;
pub(super) fn load() -> Option<(
    native_evidence_region::BodyRegion,
    conduit_body::BodyBiographyEvidence,
    c::HostId,
)> {
    let path = std::env::var_os("THERMO_PRIVATE_FLASH")?;
    let mut flash = std::fs::read(path).expect("private flash capture");
    assert_eq!(flash.len(), 4 * 1024 * 1024);
    let media = native_evidence_region::test_empty();
    media
        .media
        .lock()
        .unwrap()
        .0
        .copy_from_slice(&flash[0x3f0000..]);
    flash.fill(0);
    drop(flash);
    let mut journal = native_body_checkpoint::Checkpoint::open(media.clone()).unwrap();
    let state = media.media.lock().unwrap();
    let at = if u64::from_le_bytes(state.0[8..16].try_into().unwrap())
        > u64::from_le_bytes(state.0[32776..32784].try_into().unwrap())
    {
        0
    } else {
        32768
    };
    let binding = state.0[at + 24..at + 56].try_into().unwrap();
    drop(state);
    let mut encoded = vec![0; journal.payload_length(&binding).unwrap().unwrap()];
    let mut seed = [0; 32];
    journal.load(&binding, &mut seed, &mut encoded).unwrap();
    seed.fill(0);
    drop(journal);
    let mut offset = 0;
    #[path = "../../../../../targets/esp32/proof/tm1637/src/native_restart_compress.rs"]
    mod codec;
    let mut decoder = codec::Decoder::new(
        |out: &mut [u8; 128]| {
            let n = 128.min(encoded.len() - offset);
            out[..n].copy_from_slice(&encoded[offset..offset + n]);
            offset += n;
            Ok(n)
        },
        131072,
        32608,
    );
    let mut plain = Vec::new();
    let mut chunk = [0; 128];
    loop {
        let n = decoder.read(&mut chunk).unwrap();
        if n == 0 {
            break;
        }
        plain.extend_from_slice(&chunk[..n]);
    }
    drop(decoder);
    chunk.fill(0);
    let value: serde_json::Value = serde_json::from_slice(&plain).unwrap();
    let body_id = serde_json::from_value(value["evidence"]["body_id"].clone()).unwrap();
    drop(value);
    plain.fill(0);
    drop(plain);
    let retained = native_restart::payload::decode(&encoded, &body_id).unwrap();
    encoded.fill(0);
    drop(encoded);
    let host = retained
        .evidence
        .membership
        .parts
        .iter()
        .find_map(|p| p.current.as_ref())
        .unwrap()
        .host_id
        .clone();
    Some((media, retained.evidence, host))
}

pub(super) fn validate_final(
    media: native_evidence_region::BodyRegion,
    body: &conduit_body::BodyId,
    key: &[u8; 32],
) {
    use sha2::{Digest, Sha256};
    let binding: [u8; 32] = Sha256::digest(body.as_str().as_bytes()).into();
    let mut journal = native_body_checkpoint::Checkpoint::open(media.clone()).unwrap();
    let mut encoded = vec![0; journal.payload_length(&binding).unwrap().unwrap()];
    let mut seed = [0; 32];
    journal.load(&binding, &mut seed, &mut encoded).unwrap();
    assert_eq!(
        ed25519_dalek::SigningKey::from_bytes(&seed)
            .verifying_key()
            .as_bytes(),
        key
    );
    seed.fill(0);
    let retained = native_restart::payload::decode(&encoded, body).unwrap();
    native_restart::payload::validate(&retained.evidence, &retained.archives).unwrap();
    encoded.fill(0);
    let state = media.media.lock().unwrap();
    let lengths =
        [0usize, 32768].map(|at| u32::from_le_bytes(state.0[at + 16..at + 20].try_into().unwrap()));
    println!("ROOT_CPU_PRIVATE_FINAL encoded_lengths={lengths:?} archives={} complete_chain=true signer_preserved=true",retained.archives.len());
}
