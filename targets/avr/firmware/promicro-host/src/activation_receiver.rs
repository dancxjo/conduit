use conduit_assigned_plan::{
    decode_assigned_activation, AssignedActivation, AssignedExecutionRefusal,
    ASSIGNED_ACTIVATION_BYTES,
};

pub struct ActivationReceiver {
    len: usize,
}

impl ActivationReceiver {
    pub const fn new() -> Self {
        Self { len: 0 }
    }

    #[inline(never)]
    pub fn push(
        &mut self,
        bytes: &mut [u8],
        input: &[u8],
    ) -> Result<Option<AssignedActivation>, AssignedExecutionRefusal> {
        let end = self
            .len
            .checked_add(input.len())
            .filter(|end| *end <= ASSIGNED_ACTIVATION_BYTES)
            .ok_or(AssignedExecutionRefusal::WrongLength)?;
        bytes[self.len..end].copy_from_slice(input);
        self.len = end;
        if self.len == ASSIGNED_ACTIVATION_BYTES {
            decode_assigned_activation(&bytes[..ASSIGNED_ACTIVATION_BYTES]).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn reset(&mut self) {
        self.len = 0;
    }
}
