//! Lossless storage for the actual authored observation producer's output.
//! No public constructor narrows the generic publication Type domain.
use super::source_back::material;
use crate::observation::PreparedObservationQuantityForms as Forms;
use conduit_core::{
    validate_canonical_structured_value as validate, Quantity, StructuredInfoRefusal as R,
};
#[derive(Clone, Copy)]
pub(crate) enum PackedReading {
    Observation([u8; 4]),
    Failure {
        length: usize,
        bytes: [u8; material::FAILURE_NODE_MAXIMUM],
    },
}
#[derive(Default)]
pub(super) struct CompactReading {
    pub current: Option<PackedReading>,
    pub pending: Option<PackedReading>,
}
pub(crate) const COMPACT_READING_BYTES: usize = 1 + material::FAILURE_NODE_MAXIMUM;
pub(crate) fn compact_layout() -> conduit_kernel::CompactCanonicalLayout {
    // SAFETY: generated identity binds the exact canonical Result Type, both
    // authored Forms and this codec version. The opaque local specialization
    // separately checks the actual Source/Plan and initialized endpoint owners.
    unsafe {
        conduit_kernel::CompactCanonicalLayout::from_trusted_make(
            material::COMPACT_LAYOUT_ID,
            COMPACT_READING_BYTES as u16,
            material::RESULT_MAXIMUM as u32,
        )
        .expect("checked linked compact reading bounds")
    }
}
impl PackedReading {
    pub(crate) fn from_compact(bytes: &[u8]) -> Result<Self, R> {
        let Some((&tag, payload)) = bytes.split_first() else {
            return Err(R::MalformedCanonicalEncoding);
        };
        match tag {
            0 => {
                let packed: [u8; 4] = payload
                    .try_into()
                    .map_err(|_| R::MalformedCanonicalEncoding)?;
                let (t, h) =
                    Forms::unpack_quantities(&packed).map_err(|_| R::MalformedCanonicalEncoding)?;
                if Forms::pack_quantities(t, h).map_err(|_| R::MalformedCanonicalEncoding)?
                    != packed
                {
                    return Err(R::MalformedCanonicalEncoding);
                }
                Ok(Self::Observation(packed))
            }
            1 => {
                if payload.len() > material::FAILURE_NODE_MAXIMUM {
                    return Err(R::CanonicalEncodingTooLarge);
                }
                // Validate the full failure Type domain using only its small
                // immutable schema plus bounded node, not a full Result scratch.
                let mut canonical =
                    [0; material::FAILURE_TYPE.len() + material::FAILURE_NODE_MAXIMUM];
                let (schema, node) = canonical.split_at_mut(material::FAILURE_TYPE.len());
                schema.copy_from_slice(material::FAILURE_TYPE);
                node[..payload.len()].copy_from_slice(payload);
                validate(&canonical[..material::FAILURE_TYPE.len() + payload.len()])?;
                let mut bytes = [0; material::FAILURE_NODE_MAXIMUM];
                bytes[..payload.len()].copy_from_slice(payload);
                Ok(Self::Failure {
                    length: payload.len(),
                    bytes,
                })
            }
            _ => Err(R::MalformedCanonicalEncoding),
        }
    }
    pub(crate) fn canonical_length(&self) -> usize {
        match self {
            Self::Observation(_) => material::RESULT.len(),
            Self::Failure { length, .. } => material::UNAVAILABLE_PREFIX.len() + length,
        }
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, R> {
        let value = validate(bytes)?;
        let packed = if let Some(observation) = value.variant_payload("observation")? {
            let quantity = |name| -> Result<Quantity, R> {
                let leaf = observation
                    .record_field(name)?
                    .ok_or(R::WrongRecordFields)?;
                Quantity::decode(
                    leaf.value_node()
                        .get(5..)
                        .ok_or(R::MalformedCanonicalEncoding)?,
                )
                .map_err(|_| R::MalformedCanonicalEncoding)
            };
            Self::Observation(
                Forms::pack_quantities(quantity("temperature")?, quantity("humidity")?)
                    .map_err(|_| R::MalformedCanonicalEncoding)?,
            )
        } else {
            let failure = value.variant_payload("unavailable")?.ok_or(R::WrongType)?;
            if failure.type_bytes() != material::FAILURE_TYPE {
                return Err(R::WrongType);
            }
            let node = failure.value_node();
            let mut bytes = [0; material::FAILURE_NODE_MAXIMUM];
            bytes
                .get_mut(..node.len())
                .ok_or(R::CanonicalEncodingTooLarge)?
                .copy_from_slice(node);
            Self::Failure {
                length: node.len(),
                bytes,
            }
        };
        // Forms may normalize compatible Units. This specialized storage must
        // retain the producer's exact original bytes, so reject any difference.
        // The generic constructor remains available for the complete Type.
        let mut canonical = [0; material::RESULT_MAXIMUM];
        let length = packed.encode(&mut canonical)?;
        if canonical[..length] != *bytes {
            return Err(R::MalformedCanonicalEncoding);
        }
        Ok(packed)
    }
    pub fn encode(&self, output: &mut [u8]) -> Result<usize, R> {
        match self {
            Self::Observation(packed) => {
                let (t, h) =
                    Forms::unpack_quantities(packed).map_err(|_| R::MalformedCanonicalEncoding)?;
                let out = output
                    .get_mut(..material::RESULT.len())
                    .ok_or(R::CanonicalEncodingTooLarge)?;
                out.copy_from_slice(material::RESULT);
                out[material::RESULT_TEMPERATURE].copy_from_slice(&t.encode());
                out[material::RESULT_HUMIDITY].copy_from_slice(&h.encode());
                Ok(material::RESULT.len())
            }
            Self::Failure { length, bytes } => {
                let length = material::UNAVAILABLE_PREFIX.len() + length;
                let out = output
                    .get_mut(..length)
                    .ok_or(R::CanonicalEncodingTooLarge)?;
                let (prefix, node) = out.split_at_mut(material::UNAVAILABLE_PREFIX.len());
                prefix.copy_from_slice(material::UNAVAILABLE_PREFIX);
                node.copy_from_slice(&bytes[..node.len()]);
                Ok(length)
            }
        }
    }
}
