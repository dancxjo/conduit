//! Versioned, lossless finite transport for immutable Mask Plans.
//! Decode the complete ordinary Plan before admission; no digest substitutes for
//! configuration, and the outer carrier and route retain their separate bounds.
use super::{MaskPlot, PlannedMaskPlot};
use alloc::{string::String, vec, vec::Vec};
use conduit_core::{Plan, PlanId};
use miniz_oxide::inflate::{
    core::{decompress, inflate_flags, DecompressorOxide},
    TINFLStatus,
};
use serde::{de::Error as _, ser::Error as _, Deserialize, Deserializer, Serialize, Serializer};

mod codec;
pub(crate) const CODEC_WORKSPACE_BYTES: usize = codec::WORKSPACE_BYTES;

const LEGACY_ENCODING: &str = "conduit.face/mask-plan-deflate-json@1";
const ENCODING: &str = "conduit.face/mask-plan-deflate-json@2";
const MAX_EXPANDED_BYTES: usize = 256 * 1024;
const MAX_COMPRESSED_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonPlan {
    encoding: String,
    plan_id: PlanId,
    expanded_bytes: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    zlib_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    zlib_base64: Option<String>,
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
        json_mask(
            &self.mask,
            &self.plan.plan_id,
            &self.plan,
            MAX_COMPRESSED_BYTES,
        )
        .map_err(|_| S::Error::custom("Mask Plan encoding or capacity"))?
        .serialize(serializer)
    }
}

/// Same canonical lossless envelope, supplied by any exact complete Plan view.
/// This stays private: verification and role admission are caller obligations.
pub(crate) fn json_mask_wire_bounded(
    mask: &MaskPlot,
    plan_id: &PlanId,
    plan: &impl Serialize,
    maximum_wire_bytes: usize,
) -> Result<Vec<u8>, crate::MaskJsonRefusal> {
    json_mask_wire_bounded_with_workspace(mask, plan_id, plan, maximum_wire_bytes, &mut [])
}
pub(crate) fn json_mask_wire_bounded_with_workspace(
    mask: &MaskPlot,
    plan_id: &PlanId,
    plan: &impl Serialize,
    maximum_wire_bytes: usize,
    workspace: &mut [&mut [u8]],
) -> Result<Vec<u8>, crate::MaskJsonRefusal> {
    if maximum_wire_bytes == 0 {
        return Err(crate::MaskJsonRefusal::Capacity);
    }
    // Base64 alone cannot exceed the complete admitted envelope ceiling.
    let mut envelope = json_mask_with_workspace(
        mask,
        plan_id,
        plan,
        MAX_COMPRESSED_BYTES.min(maximum_wire_bytes / 4 * 3),
        workspace,
    )?;
    let encoded = envelope
        .plan
        .zlib_base64
        .take()
        .ok_or(crate::MaskJsonRefusal::InvalidPlan)?;
    envelope.plan.zlib_base64 = Some(String::new());
    let empty = serde_json::to_vec(&envelope).map_err(|_| crate::MaskJsonRefusal::Json)?;
    let exact = empty
        .len()
        .checked_add(encoded.len())
        .filter(|&len| len <= maximum_wire_bytes)
        .ok_or(crate::MaskJsonRefusal::Capacity)?;
    // Derive the entire prefix/field order from the canonical schema. Only the
    // base64 alphabet is inserted verbatim; it never requires JSON escaping.
    const SUFFIX: &[u8] = b"\"}}";
    let prefix = empty
        .strip_suffix(SUFFIX)
        .filter(|prefix| prefix.ends_with(b"\"zlib_base64\":\""))
        .ok_or(crate::MaskJsonRefusal::Json)?;
    let mut wire = Vec::with_capacity(exact);
    wire.extend_from_slice(prefix);
    wire.extend_from_slice(encoded.as_bytes());
    wire.extend_from_slice(SUFFIX);
    Ok(wire)
}
fn json_mask(
    mask: &MaskPlot,
    plan_id: &PlanId,
    plan: &impl Serialize,
    maximum_compressed_bytes: usize,
) -> Result<JsonMask, crate::MaskJsonRefusal> {
    json_mask_with_workspace(mask, plan_id, plan, maximum_compressed_bytes, &mut [])
}
fn json_mask_with_workspace(
    mask: &MaskPlot,
    plan_id: &PlanId,
    plan: &impl Serialize,
    maximum_compressed_bytes: usize,
    workspace: &mut [&mut [u8]],
) -> Result<JsonMask, crate::MaskJsonRefusal> {
    use crate::MaskJsonRefusal as E;
    let (zlib_base64, expanded_bytes) = (if workspace.is_empty() {
        codec::encode(plan, MAX_EXPANDED_BYTES, maximum_compressed_bytes)
    } else {
        codec::encode_with_workspace(
            plan,
            MAX_EXPANDED_BYTES,
            maximum_compressed_bytes,
            workspace,
        )
    })
    .map_err(|error| match error {
        codec::CodecError::Capacity => E::Capacity,
        codec::CodecError::Invalid => E::InvalidPlan,
    })?;
    Ok(JsonMask {
        mask: mask.clone(),
        plan: JsonPlan {
            encoding: ENCODING.into(),
            plan_id: plan_id.clone(),
            expanded_bytes,
            zlib_hex: None,
            zlib_base64: Some(zlib_base64),
        },
    })
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
        if encoded.expanded_bytes == 0 || encoded.expanded_bytes > MAX_EXPANDED_BYTES {
            return Err(D::Error::custom("Mask Plan encoding or capacity"));
        }
        let compressed = match (
            encoded.encoding.as_str(),
            encoded.zlib_hex.as_deref(),
            encoded.zlib_base64.as_deref(),
        ) {
            (LEGACY_ENCODING, Some(hex), None) => decode_hex(hex),
            (ENCODING, None, Some(base64)) => decode_base64(base64),
            _ => None,
        }
        .ok_or_else(|| D::Error::custom("Mask Plan encoding or capacity"))?;
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

fn decode_hex(encoded: &str) -> Option<Vec<u8>> {
    if encoded.is_empty()
        || !encoded.len().is_multiple_of(2)
        || encoded.len() / 2 > MAX_COMPRESSED_BYTES
    {
        return None;
    }
    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    let nibble = |byte| match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    };
    for pair in encoded.as_bytes().as_chunks::<2>().0 {
        bytes.push(nibble(pair[0])? * 16 + nibble(pair[1])?);
    }
    Some(bytes)
}
fn decode_base64(encoded: &str) -> Option<Vec<u8>> {
    if encoded.is_empty()
        || !encoded.len().is_multiple_of(4)
        || encoded.len() > MAX_COMPRESSED_BYTES.div_ceil(3) * 4
    {
        return None;
    }
    let padding = encoded
        .as_bytes()
        .iter()
        .rev()
        .take_while(|&&byte| byte == b'=')
        .count();
    if padding > 2 {
        return None;
    }
    let decoded = encoded.len() / 4 * 3 - padding;
    if decoded > MAX_COMPRESSED_BYTES {
        return None;
    }
    let value = |byte| match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let mut bytes = Vec::with_capacity(decoded);
    for (index, chunk) in encoded.as_bytes().as_chunks::<4>().0.iter().enumerate() {
        let a = value(chunk[0])?;
        let b = value(chunk[1])?;
        let last = index + 1 == encoded.len() / 4;
        bytes.push((a << 2) | (b >> 4));
        if chunk[2] == b'=' {
            if !last || chunk[3] != b'=' || b & 15 != 0 {
                return None;
            }
            continue;
        }
        let c = value(chunk[2])?;
        bytes.push((b << 4) | (c >> 2));
        if chunk[3] == b'=' {
            if !last || c & 3 != 0 {
                return None;
            }
            continue;
        }
        bytes.push((c << 6) | value(chunk[3])?);
    }
    Some(bytes)
}
