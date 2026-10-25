//! Versioned, lossless finite transport for immutable Mask Plans.
//! Decode the complete ordinary Plan before admission; no digest substitutes for
//! configuration, and the outer carrier and route retain their separate bounds.
use super::{MaskPlot, PlannedMaskPlot};
use alloc::{string::String, vec, vec::Vec};
use conduit_core::{Plan, PlanId};
use miniz_oxide::{
    deflate::compress_to_vec_zlib,
    inflate::{
        core::{decompress, inflate_flags, DecompressorOxide},
        TINFLStatus,
    },
};
use serde::{de::Error as _, ser::Error as _, Deserialize, Deserializer, Serialize, Serializer};

const ENCODING: &str = "conduit.face/mask-plan-deflate-json@1";
const MAX_EXPANDED_BYTES: usize = 256 * 1024;
const MAX_COMPRESSED_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonPlan {
    encoding: String,
    plan_id: PlanId,
    expanded_bytes: usize,
    zlib_hex: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonMask {
    mask: MaskPlot,
    plan: JsonPlan,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BinaryMask {
    mask: MaskPlot,
    plan: Plan,
}

impl Serialize for PlannedMaskPlot {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if !serializer.is_human_readable() {
            return BinaryMask {
                mask: self.mask.clone(),
                plan: self.plan.clone(),
            }
            .serialize(serializer);
        }
        let bytes = serde_json::to_vec(&self.plan).map_err(S::Error::custom)?;
        if bytes.is_empty() || bytes.len() > MAX_EXPANDED_BYTES {
            return Err(S::Error::custom("expanded Mask Plan capacity"));
        }
        let compressed = compress_to_vec_zlib(&bytes, 6);
        if compressed.len() > MAX_COMPRESSED_BYTES {
            return Err(S::Error::custom("compressed Mask Plan capacity"));
        }
        let mut zlib_hex = String::with_capacity(compressed.len() * 2);
        const DIGITS: &[u8] = b"0123456789abcdef";
        for byte in compressed {
            zlib_hex.push(DIGITS[usize::from(byte >> 4)] as char);
            zlib_hex.push(DIGITS[usize::from(byte & 15)] as char);
        }
        JsonMask {
            mask: self.mask.clone(),
            plan: JsonPlan {
                encoding: ENCODING.into(),
                plan_id: self.plan.plan_id.clone(),
                expanded_bytes: bytes.len(),
                zlib_hex,
            },
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PlannedMaskPlot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if !deserializer.is_human_readable() {
            let value = BinaryMask::deserialize(deserializer)?;
            return Ok(Self {
                mask: value.mask,
                plan: value.plan,
            });
        }
        let value = JsonMask::deserialize(deserializer)?;
        let encoded = value.plan;
        if encoded.encoding != ENCODING
            || encoded.expanded_bytes == 0
            || encoded.expanded_bytes > MAX_EXPANDED_BYTES
            || encoded.zlib_hex.is_empty()
            || encoded.zlib_hex.len() % 2 != 0
            || encoded.zlib_hex.len() / 2 > MAX_COMPRESSED_BYTES
        {
            return Err(D::Error::custom("Mask Plan encoding or capacity"));
        }
        let mut compressed = Vec::with_capacity(encoded.zlib_hex.len() / 2);
        for pair in encoded.zlib_hex.as_bytes().as_chunks::<2>().0 {
            let nibble = |byte| match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                _ => None,
            };
            let high = nibble(pair[0]).ok_or_else(|| D::Error::custom("Mask Plan hex"))?;
            let low = nibble(pair[1]).ok_or_else(|| D::Error::custom("Mask Plan hex"))?;
            compressed.push(high * 16 + low);
        }
        // The declared finite extent is checked before allocation. Exact input
        // consumption rejects trailing streams; exact output rejects truncation
        // and a forged smaller length, including decompression bombs.
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
            return Err(D::Error::custom("Mask Plan compressed document"));
        }
        let plan: Plan = serde_json::from_slice(&bytes).map_err(D::Error::custom)?;
        if plan.plan_id != encoded.plan_id {
            return Err(D::Error::custom("Mask Plan identity"));
        }
        Ok(Self {
            mask: value.mask,
            plan,
        })
    }
}
