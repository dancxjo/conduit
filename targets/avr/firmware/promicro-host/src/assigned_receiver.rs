use conduit_assigned_plan::{
    decode_assigned_plan, AssignedIdentity, AssignedPlanMaxima, AssignedPlanRefusal,
    AssignedPlanRequirements, AssignedPlanView, ASSIGNED_HOST_OPERATION, ASSIGNED_PLAN_HEADER_BYTES,
    ASSIGNED_RESOURCE,
};

pub const HOST_ID: &str = "host/avr-promicro/create1";
pub const CONTACT_OPERATION: &str = "pete.host/create1-observe-contact@1";
const MAX_ENCODED_BYTES: usize = 544;
const RESOURCE_IDS: [u16; 3] = [0, 1, 2];
const MAXIMA: AssignedPlanMaxima = AssignedPlanMaxima {
    encoded_bytes: MAX_ENCODED_BYTES as u16,
    runtime_state_bytes: 192,
    counts: [1, 1, 0, 0, 0, 0, 1, 3, 4, 0, 1, 2],
};

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

    pub fn validate(
        &self,
        host: AssignedIdentity,
        boot: AssignedIdentity,
    ) -> Result<AssignedPlanView, ReceiveRefusal> {
        let bytes = self.complete_bytes()?;
        let operation = exact_inventory(bytes)?;
        decode_assigned_plan(
            bytes,
            MAXIMA,
            AssignedPlanRequirements {
                host,
                boot,
                operations: core::slice::from_ref(&operation),
                resources: &RESOURCE_IDS,
                remote_bindings: &[],
            },
        )
        .map_err(ReceiveRefusal::Assigned)
    }

    fn complete_bytes(&self) -> Result<&[u8], ReceiveRefusal> {
        match self.expected {
            Some(expected) if expected == self.len => Ok(&self.bytes[..self.len]),
            _ => Err(ReceiveRefusal::InvalidLength),
        }
    }
}

fn exact_inventory(bytes: &[u8]) -> Result<AssignedIdentity, ReceiveRefusal> {
    let supported = AssignedIdentity::from_text(CONTACT_OPERATION);
    let mut operation = None;
    let mut resources = [false; RESOURCE_IDS.len()];
    let mut cursor = ASSIGNED_PLAN_HEADER_BYTES;
    while cursor < bytes.len() {
        let tag = *bytes.get(cursor).ok_or(ReceiveRefusal::InvalidLength)?;
        let length = usize::from(u16::from_le_bytes([
            *bytes.get(cursor + 1).ok_or(ReceiveRefusal::InvalidLength)?,
            *bytes.get(cursor + 2).ok_or(ReceiveRefusal::InvalidLength)?,
        ]));
        let start = cursor + 3;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or(ReceiveRefusal::InvalidLength)?;
        if tag == ASSIGNED_HOST_OPERATION {
            let identity: [u8; 16] = bytes
                .get(start + 4..start + 20)
                .ok_or(ReceiveRefusal::InvalidLength)?
                .try_into()
                .map_err(|_| ReceiveRefusal::InvalidLength)?;
            let identity = AssignedIdentity(identity);
            if operation.replace(identity).is_some() || identity != supported {
                return Err(ReceiveRefusal::UnsupportedInventory);
            }
        } else if tag == ASSIGNED_RESOURCE {
            let id = u16::from_le_bytes([
                *bytes.get(start + 2).ok_or(ReceiveRefusal::InvalidLength)?,
                *bytes.get(start + 3).ok_or(ReceiveRefusal::InvalidLength)?,
            ]);
            let index = RESOURCE_IDS
                .iter()
                .position(|expected| *expected == id)
                .ok_or(ReceiveRefusal::UnsupportedInventory)?;
            if resources[index] {
                return Err(ReceiveRefusal::UnsupportedInventory);
            }
            resources[index] = true;
        }
        cursor = end;
    }
    if !resources.iter().all(|present| *present) {
        return Err(ReceiveRefusal::UnsupportedInventory);
    }
    operation.ok_or(ReceiveRefusal::UnsupportedInventory)
}
