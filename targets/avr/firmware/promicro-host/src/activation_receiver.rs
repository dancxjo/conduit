use conduit_assigned_plan::{
    decode_assigned_activation, AssignedActivation, AssignedExecutionRefusal,
    ASSIGNED_ACTIVATION_BYTES,
};

pub struct ActivationReceiver {
    bytes: [u8; ASSIGNED_ACTIVATION_BYTES],
    len: usize,
}

impl ActivationReceiver {
    pub const fn new() -> Self {
        Self {
            bytes: [0; ASSIGNED_ACTIVATION_BYTES],
            len: 0,
        }
    }

    #[inline(never)]
    pub fn push(
        &mut self,
        input: &[u8],
    ) -> Result<Option<AssignedActivation>, AssignedExecutionRefusal> {
        let end = self
            .len
            .checked_add(input.len())
            .filter(|end| *end <= self.bytes.len())
            .ok_or(AssignedExecutionRefusal::WrongLength)?;
        self.bytes[self.len..end].copy_from_slice(input);
        self.len = end;
        if self.len == self.bytes.len() {
            decode_assigned_activation(&self.bytes).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn reset(&mut self) {
        self.len = 0;
    }
}
