pub const MAXIMUM_RELAY_ROUTE_BYTES: usize = 128;
const HEADER_BYTES: usize = 4 + 1 + 2 + 4;
const MAGIC: [u8; 4] = *b"CNDR";
const VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayEnvelopeError {
    EmptyRoute,
    RouteTooLarge,
    FrameTooLarge,
    OutputTooSmall,
    Malformed,
    InvalidRouteEncoding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelayEnvelope<'a> {
    pub route_id: &'a str,
    pub protected_frame: &'a [u8],
}

impl RelayEnvelope<'_> {
    pub fn encode(&self, output: &mut [u8]) -> Result<usize, RelayEnvelopeError> {
        if self.route_id.is_empty() {
            return Err(RelayEnvelopeError::EmptyRoute);
        }
        if self.route_id.len() > MAXIMUM_RELAY_ROUTE_BYTES {
            return Err(RelayEnvelopeError::RouteTooLarge);
        }
        let frame_bytes: u32 = self
            .protected_frame
            .len()
            .try_into()
            .map_err(|_| RelayEnvelopeError::FrameTooLarge)?;
        let length = HEADER_BYTES
            .checked_add(self.route_id.len())
            .and_then(|value| value.checked_add(self.protected_frame.len()))
            .ok_or(RelayEnvelopeError::FrameTooLarge)?;
        if output.len() < length {
            return Err(RelayEnvelopeError::OutputTooSmall);
        }
        output[..4].copy_from_slice(&MAGIC);
        output[4] = VERSION;
        output[5..7].copy_from_slice(&(self.route_id.len() as u16).to_le_bytes());
        output[7..11].copy_from_slice(&frame_bytes.to_le_bytes());
        output[11..11 + self.route_id.len()].copy_from_slice(self.route_id.as_bytes());
        output[11 + self.route_id.len()..length].copy_from_slice(self.protected_frame);
        Ok(length)
    }
}

pub fn decode_relay_envelope(
    encoded: &[u8],
    maximum_protected_frame_bytes: usize,
) -> Result<RelayEnvelope<'_>, RelayEnvelopeError> {
    if encoded.len() < HEADER_BYTES || encoded[..4] != MAGIC || encoded[4] != VERSION {
        return Err(RelayEnvelopeError::Malformed);
    }
    let route_bytes = u16::from_le_bytes([encoded[5], encoded[6]]) as usize;
    if route_bytes == 0 {
        return Err(RelayEnvelopeError::EmptyRoute);
    }
    if route_bytes > MAXIMUM_RELAY_ROUTE_BYTES {
        return Err(RelayEnvelopeError::RouteTooLarge);
    }
    let frame_bytes =
        u32::from_le_bytes([encoded[7], encoded[8], encoded[9], encoded[10]]) as usize;
    if frame_bytes > maximum_protected_frame_bytes {
        return Err(RelayEnvelopeError::FrameTooLarge);
    }
    let expected = HEADER_BYTES
        .checked_add(route_bytes)
        .and_then(|value| value.checked_add(frame_bytes))
        .ok_or(RelayEnvelopeError::Malformed)?;
    if encoded.len() != expected {
        return Err(RelayEnvelopeError::Malformed);
    }
    let route = core::str::from_utf8(&encoded[HEADER_BYTES..HEADER_BYTES + route_bytes])
        .map_err(|_| RelayEnvelopeError::InvalidRouteEncoding)?;
    Ok(RelayEnvelope {
        route_id: route,
        protected_frame: &encoded[HEADER_BYTES + route_bytes..],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_sees_only_one_bounded_route_and_opaque_frame() {
        let frame = [0xa5; 42];
        let envelope = RelayEnvelope {
            route_id: "route/7",
            protected_frame: &frame,
        };
        let mut encoded = [0; 80];
        let length = envelope.encode(&mut encoded).unwrap();
        let decoded = decode_relay_envelope(&encoded[..length], 42).unwrap();
        assert_eq!(decoded, envelope);
        assert_eq!(
            decode_relay_envelope(&encoded[..length], 41),
            Err(RelayEnvelopeError::FrameTooLarge)
        );
    }
}
