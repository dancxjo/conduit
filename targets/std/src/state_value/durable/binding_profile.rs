//! Exact finite residence envelopes; the ordinary durable profile stays unchanged.
use super::{DurableStateBinding, DurableStateRefusal, MAXIMUM_DURABLE_STATE_BYTES};
impl DurableStateBinding {
    pub fn validate(&self) -> Result<(), DurableStateRefusal> {
        self.validate_prepared(MAXIMUM_DURABLE_STATE_BYTES)
    }
    pub(super) fn validate_prepared(&self, maximum: u32) -> Result<(), DurableStateRefusal> {
        if maximum == 0
            || maximum > 8192
            || self.body.is_empty()
            || self.state.as_str().is_empty()
            || self.value_kind.as_str().is_empty()
            || self.maximum_value_bytes == 0
            || self.maximum_value_bytes > maximum
        {
            return Err(DurableStateRefusal::InvalidBinding);
        }
        Ok(())
    }
}
