//! Canonical bounded octets at the class-neutral control-transfer boundary.

use conduit_core::{StructuredInfoRefusal, validate_canonical_structured_value};

use super::control_request::{ControlRequestRefusal, ControlTransferRequest};

/// Native-owned finite storage. A canonical value supplies bytes, never an
/// address or ownership claim. The selected native owner must still admit the
/// transaction and constrain its actual DMA buffer independently.
pub struct ControlOutputPayload {
    bytes: [u8; 256],
    length: usize,
}

impl ControlOutputPayload {
    pub fn decode(encoded: &[u8]) -> Result<Self, StructuredInfoRefusal> {
        let value = validate_canonical_structured_value(encoded)?;
        let mut payload = Self {
            bytes: [0; 256],
            length: 0,
        };
        payload.length = value.copy_octet_collection("value/u8", &mut payload.bytes)?;
        Ok(payload)
    }

    pub fn request(
        &self,
        setup: [u8; 8],
        maximum_data_bytes: u16,
    ) -> Result<ControlTransferRequest<'_>, ControlRequestRefusal> {
        ControlTransferRequest::new(setup, &self.bytes[..self.length], maximum_data_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{PreparedLeafSequenceEncoder, kind_id};

    #[test]
    fn canonical_octets_cover_the_full_native_control_envelope() {
        let mut encoder = PreparedLeafSequenceEncoder::new(kind_id("value/u8"), 1, 256).unwrap();
        let octets: [[u8; 1]; 256] = core::array::from_fn(|index| [index as u8]);
        for length in 0..=256_u16 {
            let encoded = encoder
                .encode(
                    octets[..usize::from(length)]
                        .iter()
                        .map(|byte| byte.as_slice()),
                )
                .unwrap();
            let payload = ControlOutputPayload::decode(encoded).unwrap();
            let mut setup = [0; 8];
            setup[6..].copy_from_slice(&length.to_le_bytes());
            let request = payload.request(setup, 256).unwrap();
            assert_eq!(
                request.output(),
                &core::array::from_fn::<_, 256, _>(|i| i as u8)[..usize::from(length)]
            );
            assert_eq!(request.length(), length);
        }
    }

    #[test]
    fn canonical_identity_and_native_capacity_are_independent_checks() {
        let mut encoder = PreparedLeafSequenceEncoder::new(kind_id("value/u8"), 1, 257).unwrap();
        assert!(
            ControlOutputPayload::decode(encoder.encode([&[1][..]; 257].into_iter()).unwrap())
                .is_err()
        );
        let mut wrong = PreparedLeafSequenceEncoder::new(kind_id("value/i8"), 1, 1).unwrap();
        assert!(
            ControlOutputPayload::decode(wrong.encode([&[1][..]].into_iter()).unwrap()).is_err()
        );
        let valid = encoder.encode([&[1][..]].into_iter()).unwrap();
        for length in 0..valid.len() {
            assert!(ControlOutputPayload::decode(&valid[..length]).is_err());
        }
        let payload = ControlOutputPayload::decode(valid).unwrap();
        let mut setup = [0; 8];
        setup[6] = 1;
        assert!(payload.request(setup, 0).is_err());
        setup[0] = 128;
        assert!(payload.request(setup, 256).is_err());
    }
}
