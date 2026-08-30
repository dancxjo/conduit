use conduit_assigned_plan::{
    decode_assigned_single_source, AssignedIdentity, AssignedPlanMaxima, AssignedPlanRefusal,
    AssignedSingleSourceRequirements, AssignedSingleSourceView, ASSIGNED_PLAN_HEADER_BYTES,
};

pub const CONTACT_OPERATION: &str = "pete.host/create1-observe-contact@1";
pub const HOST_IDENTITY: AssignedIdentity = AssignedIdentity([
    0xb8, 0xe0, 0x1c, 0x63, 0xb5, 0x99, 0x17, 0x31, 0x79, 0x49, 0xaf, 0x70, 0x80, 0xbd,
    0xe0, 0x01,
]);
const CONTACT_OPERATION_IDENTITY: AssignedIdentity = AssignedIdentity([
    0x1f, 0x0a, 0x63, 0xdc, 0x4d, 0x76, 0x71, 0x9c, 0x37, 0x09, 0x52, 0x83, 0xca, 0xad,
    0xdc, 0x91,
]);
const MAX_ENCODED_BYTES: usize = 544;
const RESOURCE_IDS: [u16; 3] = [0, 1, 2];
const MAXIMA: AssignedPlanMaxima = AssignedPlanMaxima::SINGLE_SOURCE;
const EXACT_COUNTS: [u8; 12] = [1, 1, 0, 0, 0, 0, 1, 3, 4, 0, 1, 2];

pub type ValidatedContactPlan = AssignedSingleSourceView;

pub struct AssignedReceiver {
    bytes: [u8; MAX_ENCODED_BYTES],
    len: usize,
    expected: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiveRefusal {
    Capacity,
    InvalidLength,
    UnsupportedInventory,
    Assigned(AssignedPlanRefusal),
}

impl AssignedReceiver {
    pub const fn new() -> Self {
        Self {
            bytes: [0; MAX_ENCODED_BYTES],
            len: 0,
            expected: None,
        }
    }

    #[inline(never)]
    pub fn push(&mut self, input: &[u8]) -> Result<Option<&[u8]>, ReceiveRefusal> {
        let new_len = self
            .len
            .checked_add(input.len())
            .filter(|length| *length <= MAX_ENCODED_BYTES)
            .ok_or(ReceiveRefusal::Capacity)?;
        self.bytes[self.len..new_len].copy_from_slice(input);
        self.len = new_len;
        if self.expected.is_none() && self.len >= 12 {
            let encoded = usize::from(u16::from_le_bytes([self.bytes[10], self.bytes[11]]));
            if !(ASSIGNED_PLAN_HEADER_BYTES..=MAX_ENCODED_BYTES).contains(&encoded) {
                return Err(ReceiveRefusal::InvalidLength);
            }
            self.expected = Some(encoded);
        }
        match self.expected {
            Some(expected) if self.len == expected => Ok(Some(&self.bytes[..self.len])),
            Some(expected) if self.len > expected => Err(ReceiveRefusal::InvalidLength),
            _ => Ok(None),
        }
    }

    #[inline(never)]
    pub fn validate(
        &self,
        host: AssignedIdentity,
        boot: AssignedIdentity,
    ) -> Result<ValidatedContactPlan, ReceiveRefusal> {
        let bytes = self.complete_bytes()?;
        let plan = decode_assigned_single_source(
            bytes,
            MAXIMA,
            AssignedSingleSourceRequirements {
                host,
                boot,
                counts: EXACT_COUNTS,
                operation: CONTACT_OPERATION_IDENTITY,
                resources: &RESOURCE_IDS,
            },
        )
        .map_err(ReceiveRefusal::Assigned)?;
        if plan.maximum_step_work < 3
            || plan.maximum_output_bytes != 1
            || plan.output_port != 0
        {
            return Err(ReceiveRefusal::UnsupportedInventory);
        }
        Ok(plan)
    }

    pub fn bytes(&self) -> Result<&[u8], ReceiveRefusal> {
        self.complete_bytes()
    }

    pub fn reset(&mut self) {
        self.len = 0;
        self.expected = None;
    }

    fn complete_bytes(&self) -> Result<&[u8], ReceiveRefusal> {
        match self.expected {
            Some(expected) if expected == self.len => Ok(&self.bytes[..self.len]),
            _ => Err(ReceiveRefusal::InvalidLength),
        }
    }
}
