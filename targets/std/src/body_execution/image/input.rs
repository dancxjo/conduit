//! Retained finite ingress shares the selected IMAGE's existing memory owner.
use super::*;

/// Already prepared ingress, with its owned backing charged until actual drop.
/// Borrowing this input neither authorizes its content nor acknowledges delivery.
/// There is no conversion that separates its bytes from their memory custody.
pub struct ImageForeInput {
    input: ExternalForeInput,
    _memory: ImageMemoryLease,
}
impl ImageForeInput {
    pub fn input(&self) -> &ExternalForeInput {
        &self.input
    }
}
impl StdHost {
    /// Admit retention of an already prepared finite input against the existing
    /// selected IMAGE heap. Counts allocated capacities, including spare payload
    /// space, rather than only the visible length. Refusal drops the input.
    ///
    /// Preparation effects and peak workspace must have independent custody
    /// before this call. This grants no Source access, provider or Plan authority
    /// and does not select an adapter or enlarge its sealed queue budgets.
    pub fn retain_selected_image_fore_input(
        &mut self,
        input: ExternalForeInput,
    ) -> Result<ImageForeInput, String> {
        let bytes = core::mem::size_of::<ExternalForeInput>()
            .checked_add(input.front_port_id.0.capacity())
            .and_then(|bytes| bytes.checked_add(input.bytes.capacity()))
            .and_then(|bytes| bytes.checked_add(ImageMemoryLease::owned_backing_bytes()))
            .ok_or("IMAGE input backing size overflow")?;
        let memory = self.kernel_resources.reserve_managed_base_memory(bytes)?;
        Ok(ImageForeInput {
            input,
            _memory: memory,
        })
    }
}
