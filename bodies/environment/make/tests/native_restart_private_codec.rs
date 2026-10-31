//! Opt-in private backup codec measurement; never prints payload or key material.
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart_compress.rs"]
mod codec;
use sha2::{Digest, Sha256};
#[test]
#[ignore = "requires local private flash backup via CONDUIT_PRIVATE_FLASH"]
fn authenticated_private_bank_roundtrip() {
    let path = std::env::var("CONDUIT_PRIVATE_FLASH").expect("private backup path required");
    let flash = std::fs::read(path).unwrap();
    assert_eq!(flash.len(), 4194304);
    for bank in 0..2 {
        let base = 0x3f0000 + bank * 32768;
        let h = &flash[base..base + 160];
        assert_eq!(&h[..8], b"BDYCST01");
        assert_eq!(&h[152..156], b"done");
        let n = u32::from_le_bytes(h[16..20].try_into().unwrap()) as usize;
        let encoded = &flash[base + 160..base + 160 + n];
        let mut hash = Sha256::new();
        hash.update(&h[..120]);
        hash.update(encoded);
        assert_eq!(&hash.finalize()[..], &h[120..152]);
        let mut at = 0;
        let mut decoder = codec::Decoder::new(
            |out: &mut [u8; 128]| {
                let n = 128.min(encoded.len() - at);
                out[..n].copy_from_slice(&encoded[at..at + n]);
                at += n;
                Ok(n)
            },
            131072,
            32608,
        );
        let mut plain = Vec::new();
        let mut out = [0; 128];
        loop {
            let n = decoder.read(&mut out).unwrap();
            if n == 0 {
                break;
            }
            plain.extend_from_slice(&out[..n]);
        }
        drop(decoder);
        let _: serde_json::Value = serde_json::from_slice(&plain).unwrap();
        let mut repacked = Vec::new();
        let mut enc = codec::Encoder::new(
            |b: &[u8]| {
                repacked.extend_from_slice(b);
                Ok(())
            },
            131072,
            200000,
        );
        enc.write(&plain).unwrap();
        let lengths = enc.finish().unwrap();
        drop(enc);
        eprintln!(
            "bank={bank} plain={} old_encoded={} new_encoded={}",
            plain.len(),
            n,
            lengths.encoded
        );
        assert!(repacked.len() <= encoded.len());
    }
}
