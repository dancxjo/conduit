//! Lossless advertisement field profile for version-two admission carriers.
//!
//! These are independent bounds: the expanded public JSON document may contain
//! at most 64 KiB, its zlib stream at most 24 KiB. Every carrier additionally
//! enforces its own complete wire bound (native serial/WebSocket: 48 KiB).
//! Neither bound is a global HostAdvertisement semantic limit. No capabilities,
//! defaults, canonical bytes or identities are elided, and decoding grants no
//! admission. Standalone requests and receipts retain their existing JSON.
use alloc::{string::String, vec, vec::Vec};
use conduit_core::HostAdvertisement;
use miniz_oxide::{
    deflate::compress_to_vec_zlib,
    inflate::{
        core::{decompress, inflate_flags, DecompressorOxide},
        TINFLStatus,
    },
};
use serde::{de::Error as _, ser::Error as _, Deserialize, Deserializer, Serialize, Serializer};

pub const ENCODING: &str = "conduit.body/admission-advertisement-deflate-json@1";
pub const MAXIMUM_EXPANDED_BYTES: usize = 64 * 1024;
pub const MAXIMUM_COMPRESSED_BYTES: usize = 24 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedAdvertisement {
    encoding: String,
    expanded_bytes: usize,
    zlib_hex: String,
}

/// Serialize one complete ordinary public advertisement JSON document.
pub fn serialize<S: Serializer>(
    advertisement: &HostAdvertisement,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let bytes = serde_json::to_vec(advertisement).map_err(S::Error::custom)?;
    if bytes.is_empty() || bytes.len() > MAXIMUM_EXPANDED_BYTES {
        return Err(S::Error::custom(
            "expanded admission advertisement capacity",
        ));
    }
    let compressed = compress_to_vec_zlib(&bytes, 6);
    if compressed.len() > MAXIMUM_COMPRESSED_BYTES {
        return Err(S::Error::custom(
            "compressed admission advertisement capacity",
        ));
    }
    let mut zlib_hex = String::with_capacity(compressed.len() * 2);
    const DIGITS: &[u8] = b"0123456789abcdef";
    for byte in compressed {
        zlib_hex.push(DIGITS[usize::from(byte >> 4)] as char);
        zlib_hex.push(DIGITS[usize::from(byte & 15)] as char);
    }
    EncodedAdvertisement {
        encoding: ENCODING.into(),
        expanded_bytes: bytes.len(),
        zlib_hex,
    }
    .serialize(serializer)
}

/// Accept only this explicit profile. Legacy raw fields are selected by the
/// enclosing legacy carrier version, never by a permissive fallback here.
pub fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<HostAdvertisement, D::Error> {
    let encoded = EncodedAdvertisement::deserialize(deserializer)?;
    if encoded.encoding != ENCODING
        || encoded.expanded_bytes == 0
        || encoded.expanded_bytes > MAXIMUM_EXPANDED_BYTES
        || encoded.zlib_hex.is_empty()
        || encoded.zlib_hex.len() % 2 != 0
        || encoded.zlib_hex.len() / 2 > MAXIMUM_COMPRESSED_BYTES
    {
        return Err(D::Error::custom(
            "admission advertisement encoding or capacity",
        ));
    }
    let mut compressed = Vec::with_capacity(encoded.zlib_hex.len() / 2);
    for pair in encoded.zlib_hex.as_bytes().as_chunks::<2>().0 {
        let nibble = |byte| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        let high =
            nibble(pair[0]).ok_or_else(|| D::Error::custom("admission advertisement hex"))?;
        let low = nibble(pair[1]).ok_or_else(|| D::Error::custom("admission advertisement hex"))?;
        compressed.push(high * 16 + low);
    }
    // Allocation follows the declared expanded cap. Exact stream consumption
    // rejects suffixes/concatenation; exact output rejects bombs and truncation.
    let mut bytes = vec![0; encoded.expanded_bytes];
    let (status, consumed, written) = decompress(
        &mut DecompressorOxide::new(),
        &compressed,
        &mut bytes,
        0,
        inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
            | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
    );
    if status != TINFLStatus::Done
        || consumed != compressed.len()
        || written != encoded.expanded_bytes
    {
        return Err(D::Error::custom(
            "admission advertisement compressed document",
        ));
    }
    serde_json::from_slice(&bytes).map_err(D::Error::custom)
}

#[derive(Serialize, Deserialize)]
pub struct CompactAdvertisement(#[serde(with = "self")] pub HostAdvertisement);

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::borrow::ToOwned;
    use conduit_core::OfferGeneration;
    use serde_json::{json, Value};

    fn advertisement() -> HostAdvertisement {
        HostAdvertisement {
            declared_name: Some(conduit_core::HostDeclaredName::new("Yellow").unwrap()),
            protocol_version: conduit_core::PROTOCOL_VERSION,
            host_id: "host/complete".into(),
            boot_id: "boot/complete".into(),
            offer_generation: OfferGeneration(u64::MAX),
            profile: "profile/complete".into(),
            bases: Vec::new(),
            resources: Vec::new(),
            capabilities: Vec::new(),
            planner_capabilities: Vec::new(),
        }
    }
    fn wire() -> Value {
        serde_json::to_value(CompactAdvertisement(advertisement())).unwrap()
    }
    fn refused(value: Value) {
        assert!(serde_json::from_value::<CompactAdvertisement>(value).is_err());
    }
    fn hex(bytes: &[u8]) -> String {
        use alloc::format;
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
    #[test]
    fn full_public_json_roundtrip_preserves_exact_high_generation_and_fields() {
        let original = advertisement();
        let raw = serde_json::to_vec(&original).unwrap();
        let encoded = wire();
        assert_eq!(encoded["expanded_bytes"], raw.len());
        let decoded: CompactAdvertisement = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.0, original);
        assert_eq!(serde_json::to_vec(&decoded.0).unwrap(), raw);
    }
    #[test]
    fn malformed_profile_lengths_streams_and_expansion_bombs_refuse() {
        for length in [0, MAXIMUM_EXPANDED_BYTES + 1, 1] {
            let mut value = wire();
            value["expanded_bytes"] = json!(length);
            refused(value);
        }
        let mut value = wire();
        value["encoding"] = json!("unknown");
        refused(value);
        let mut value = wire();
        value["unknown"] = json!(true);
        refused(value);
        for payload in [
            "g0".into(),
            "0".into(),
            "00".repeat(MAXIMUM_COMPRESSED_BYTES + 1),
        ] {
            let mut value = wire();
            value["zlib_hex"] = json!(payload);
            refused(value);
        }
        let original = wire();
        for suffix in [
            "00".to_owned(),
            original["zlib_hex"].as_str().unwrap().to_owned(),
        ] {
            let mut value = original.clone();
            value["zlib_hex"] = json!(alloc::format!(
                "{}{suffix}",
                original["zlib_hex"].as_str().unwrap()
            ));
            refused(value);
        }
        let mut truncated = wire();
        let payload = truncated["zlib_hex"].as_str().unwrap();
        truncated["zlib_hex"] = json!(&payload[..payload.len() - 2]);
        refused(truncated);
        let compressed = compress_to_vec_zlib(&vec![b' '; MAXIMUM_EXPANDED_BYTES + 1], 6);
        refused(
            json!({"encoding":ENCODING,"expanded_bytes":MAXIMUM_EXPANDED_BYTES,"zlib_hex":hex(&compressed)}),
        );
        refused(
            json!({"encoding":ENCODING,"expanded_bytes":1,"zlib_hex":hex(&compress_to_vec_zlib(b"x",6))}),
        );
        refused(serde_json::to_value(advertisement()).unwrap());
    }
    #[test]
    fn oversized_complete_document_refuses_instead_of_dropping_fields() {
        let mut original = advertisement();
        original.host_id = "h".repeat(MAXIMUM_EXPANDED_BYTES).into();
        assert!(serde_json::to_vec(&CompactAdvertisement(original)).is_err());
    }
}
