//! Proof-only fixed compositor-window descriptor; no product protocol/authority.
pub const CAPACITY: usize = 65536;
pub const WINDOW: u64 = 0x0060_3000;
pub const BYTES: usize = 64;
pub const TO_GUEST: u32 = 1;
pub const TO_ROOT: u32 = 2;
pub const XOR: u8 = 0xa5;
const MAGIC: u64 = 0x5354_4147_4544_0001;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Refusal {
    Length = 1,
    Overflow = 2,
    Range = 3,
    Direction = 4,
    Instance = 5,
    Request = 6,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Descriptor {
    pub direction: u32,
    pub instance: u64,
    pub request: u64,
    pub start: u64,
    pub offset: u64,
    pub length: u64,
    pub capacity: u64,
}
impl Descriptor {
    #[allow(dead_code)] // Root constructs requests; the linked guest only decodes them.
    pub fn new(instance: u64, request: u64, length: usize) -> Self {
        Self {
            direction: TO_GUEST,
            instance,
            request,
            start: WINDOW,
            offset: 0,
            length: length as u64,
            capacity: CAPACITY as u64,
        }
    }
    pub fn encode(self) -> [u8; BYTES] {
        let mut output = [0; BYTES];
        output[..8].copy_from_slice(&MAGIC.to_le_bytes());
        output[8..12].copy_from_slice(&1u32.to_le_bytes());
        output[12..16].copy_from_slice(&self.direction.to_le_bytes());
        for (chunk, value) in output[16..].chunks_exact_mut(8).zip([
            self.instance,
            self.request,
            self.start,
            self.offset,
            self.length,
            self.capacity,
        ]) {
            chunk.copy_from_slice(&value.to_le_bytes());
        }
        output
    }
    pub fn decode(input: &[u8]) -> Result<Self, Refusal> {
        if input.len() != BYTES {
            return Err(Refusal::Length);
        }
        let word = |offset| {
            let mut bytes = [0; 8];
            bytes.copy_from_slice(&input[offset..offset + 8]);
            u64::from_le_bytes(bytes)
        };
        if word(0) != MAGIC || input[8..12] != 1u32.to_le_bytes() {
            return Err(Refusal::Range);
        }
        let mut direction = [0; 4];
        direction.copy_from_slice(&input[12..16]);
        Ok(Self {
            direction: u32::from_le_bytes(direction),
            instance: word(16),
            request: word(24),
            start: word(32),
            offset: word(40),
            length: word(48),
            capacity: word(56),
        })
    }
    pub fn validate(self) -> Result<(), Refusal> {
        if self.direction != TO_GUEST {
            return Err(Refusal::Direction);
        }
        if self.instance == 0 {
            return Err(Refusal::Instance);
        }
        if self.request == 0 {
            return Err(Refusal::Request);
        }
        if self.start != WINDOW || self.capacity != CAPACITY as u64 {
            return Err(Refusal::Range);
        }
        let end = self
            .offset
            .checked_add(self.length)
            .ok_or(Refusal::Overflow)?;
        self.start.checked_add(end).ok_or(Refusal::Overflow)?;
        if end > self.capacity {
            return Err(Refusal::Length);
        }
        Ok(())
    }
    /// Root correlates its current actual domain and next request. The guest
    /// only echoes supplied identity; it has no independent instance authority.
    #[allow(dead_code)] // Root owns actual current-instance/request correlation.
    pub fn admit(self, instance: u64, request: u64) -> Result<(), Refusal> {
        self.validate()?;
        if self.instance != instance {
            return Err(Refusal::Instance);
        }
        if self.request != request {
            return Err(Refusal::Request);
        }
        Ok(())
    }
    pub fn returned(self) -> Self {
        Self {
            direction: TO_ROOT,
            ..self
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_lengths_identity_direction_and_overflow_are_finite() {
        for length in [0, 256, 4096, 16384, 65536] {
            let descriptor = Descriptor::new(7, 9, length);
            assert_eq!(Descriptor::decode(&descriptor.encode()), Ok(descriptor));
            assert_eq!(descriptor.admit(7, 9), Ok(()));
        }
        let d = Descriptor::new(7, 9, 0);
        for (changed, reason) in [
            (Descriptor { length: 65537, ..d }, Refusal::Length),
            (
                Descriptor {
                    offset: u64::MAX,
                    length: 1,
                    ..d
                },
                Refusal::Overflow,
            ),
            (
                Descriptor {
                    start: WINDOW + 1,
                    ..d
                },
                Refusal::Range,
            ),
            (d.returned(), Refusal::Direction),
            (Descriptor { instance: 6, ..d }, Refusal::Instance),
            (Descriptor { request: 8, ..d }, Refusal::Request),
        ] {
            assert_eq!(changed.admit(7, 9), Err(reason));
        }
        assert_eq!(Descriptor::decode(&d.encode()[..63]), Err(Refusal::Length));
    }
}
