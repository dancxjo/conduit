use crate::{BleGattProfile, BLE_FRAGMENT_HEADER_BYTES};

pub const MAXIMUM_BLE_FRAME_BYTES: usize = 2_048;
pub const MAXIMUM_BLE_GATT_PACKET_BYTES: usize = 182;
const MAGIC: u8 = 0xcb;
const REVISION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BleFramingError {
    EmptyFrame,
    OversizedFrame,
    OutputTooSmall,
    InvalidHeader,
    WrongSequence,
    ReorderedFragment,
    InconsistentFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BleFragment<'a> {
    pub sequence: u8,
    pub index: u8,
    pub count: u8,
    pub frame_length: u16,
    pub payload: &'a [u8],
}

pub fn fragment_count(frame_length: usize, profile: BleGattProfile) -> Result<u8, BleFramingError> {
    if frame_length == 0 {
        return Err(BleFramingError::EmptyFrame);
    }
    if frame_length > usize::try_from(profile.maximum_frame_bytes).unwrap_or(usize::MAX) {
        return Err(BleFramingError::OversizedFrame);
    }
    let payload = usize::from(
        profile
            .maximum_gatt_packet_bytes
            .saturating_sub(BLE_FRAGMENT_HEADER_BYTES),
    );
    if payload == 0 {
        return Err(BleFramingError::OversizedFrame);
    }
    let count = frame_length.div_ceil(payload);
    let count = u8::try_from(count).map_err(|_| BleFramingError::OversizedFrame)?;
    if count == 0 || count > profile.maximum_fragments_per_frame {
        return Err(BleFramingError::OversizedFrame);
    }
    Ok(count)
}

pub fn encode_fragment(
    frame: &[u8],
    sequence: u8,
    index: u8,
    profile: BleGattProfile,
    output: &mut [u8],
) -> Result<usize, BleFramingError> {
    let count = fragment_count(frame.len(), profile)?;
    if index >= count {
        return Err(BleFramingError::ReorderedFragment);
    }
    let payload_limit = usize::from(
        profile
            .maximum_gatt_packet_bytes
            .saturating_sub(BLE_FRAGMENT_HEADER_BYTES),
    );
    let start = usize::from(index) * payload_limit;
    let end = frame.len().min(start.saturating_add(payload_limit));
    let packet_length = usize::from(BLE_FRAGMENT_HEADER_BYTES) + end.saturating_sub(start);
    if output.len() < packet_length {
        return Err(BleFramingError::OutputTooSmall);
    }
    output[0] = MAGIC;
    output[1] = REVISION;
    output[2] = sequence;
    output[3] = index;
    output[4] = count;
    output[5..7].copy_from_slice(
        &u16::try_from(frame.len())
            .map_err(|_| BleFramingError::OversizedFrame)?
            .to_be_bytes(),
    );
    output[7..packet_length].copy_from_slice(&frame[start..end]);
    Ok(packet_length)
}

pub fn decode_fragment(packet: &[u8]) -> Result<BleFragment<'_>, BleFramingError> {
    if packet.len() < usize::from(BLE_FRAGMENT_HEADER_BYTES)
        || packet[0] != MAGIC
        || packet[1] != REVISION
        || packet[4] == 0
        || packet[3] >= packet[4]
    {
        return Err(BleFramingError::InvalidHeader);
    }
    Ok(BleFragment {
        sequence: packet[2],
        index: packet[3],
        count: packet[4],
        frame_length: u16::from_be_bytes([packet[5], packet[6]]),
        payload: &packet[7..],
    })
}

pub struct BleReassembler {
    profile: BleGattProfile,
    next_sequence: u8,
    active_sequence: Option<u8>,
    expected_index: u8,
    fragment_count: u8,
    frame_length: usize,
    retained: usize,
    bytes: [u8; MAXIMUM_BLE_FRAME_BYTES],
}

impl BleReassembler {
    pub fn new(profile: BleGattProfile) -> Self {
        Self {
            profile,
            next_sequence: 0,
            active_sequence: None,
            expected_index: 0,
            fragment_count: 0,
            frame_length: 0,
            retained: 0,
            bytes: [0; MAXIMUM_BLE_FRAME_BYTES],
        }
    }

    pub fn admit<'a>(&'a mut self, packet: &[u8]) -> Result<Option<&'a [u8]>, BleFramingError> {
        if packet.len() > usize::from(self.profile.maximum_gatt_packet_bytes) {
            return Err(BleFramingError::OversizedFrame);
        }
        let fragment = decode_fragment(packet)?;
        if fragment.sequence != self.next_sequence {
            return Err(BleFramingError::WrongSequence);
        }
        if fragment.index != self.expected_index {
            return Err(BleFramingError::ReorderedFragment);
        }
        if fragment.count > self.profile.maximum_fragments_per_frame
            || usize::from(fragment.frame_length)
                > usize::try_from(self.profile.maximum_frame_bytes).unwrap_or(usize::MAX)
        {
            return Err(BleFramingError::OversizedFrame);
        }
        if let Some(active) = self.active_sequence {
            if active != fragment.sequence
                || self.fragment_count != fragment.count
                || self.frame_length != usize::from(fragment.frame_length)
            {
                return Err(BleFramingError::InconsistentFrame);
            }
        } else {
            self.active_sequence = Some(fragment.sequence);
            self.fragment_count = fragment.count;
            self.frame_length = usize::from(fragment.frame_length);
        }
        let end = self.retained.saturating_add(fragment.payload.len());
        if end > self.frame_length || end > self.bytes.len() {
            return Err(BleFramingError::InconsistentFrame);
        }
        self.bytes[self.retained..end].copy_from_slice(fragment.payload);
        self.retained = end;
        self.expected_index = self.expected_index.saturating_add(1);
        if self.expected_index != self.fragment_count {
            return Ok(None);
        }
        if self.retained != self.frame_length {
            return Err(BleFramingError::InconsistentFrame);
        }
        let completed = &self.bytes[..self.retained];
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.active_sequence = None;
        self.expected_index = 0;
        self.fragment_count = 0;
        self.frame_length = 0;
        self.retained = 0;
        Ok(Some(completed))
    }
}
