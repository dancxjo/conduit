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
        let bytes = input_backing_bytes(&input)?;
        // SAFETY: no allocation or effect occurs in this transfer-only factory.
        // Its existing preparation/caller custody remains independently required.
        unsafe { self.prepare_selected_image_fore_input(bytes, || Ok(input)) }
    }

    /// Reserve the complete cooperative preparation envelope before invoking
    /// Root's factory, then keep the actual input backing charged until drop.
    /// Captures and caller metadata outside the factory need separate custody.
    /// Refusal before reservation never invokes the factory; failed preparation
    /// drops its result before releasing this reservation.
    ///
    /// # Safety
    /// Root must qualify the complete synchronous factory peak against
    /// `preparation_bytes`, including temporary storage and the returned input's
    /// capacities. Effects/workers must quiesce before returning, including on
    /// failure or unwinding. This is logical cooperative admission, not an
    /// allocator sandbox, Source authority, asynchronous retirement or RSS proof.
    pub unsafe fn prepare_selected_image_fore_input(
        &mut self,
        preparation_bytes: usize,
        prepare: impl FnOnce() -> Result<ExternalForeInput, String>,
    ) -> Result<ImageForeInput, String> {
        if preparation_bytes == 0 {
            return Err("IMAGE input preparation envelope is empty".into());
        }
        let admitted = preparation_bytes
            .checked_add(ImageMemoryLease::owned_backing_bytes())
            .ok_or("IMAGE input preparation size overflow")?;
        let mut memory = self
            .kernel_resources
            .reserve_managed_base_memory(admitted)?;
        let input = prepare()?;
        let retained = input_backing_bytes(&input)?;
        if retained > preparation_bytes {
            return Err("IMAGE input backing exceeds admitted preparation envelope".into());
        }
        memory.retain_result(
            retained
                .checked_add(ImageMemoryLease::owned_backing_bytes())
                .ok_or("IMAGE input retention size overflow")?,
        );
        Ok(ImageForeInput {
            input,
            _memory: memory,
        })
    }
}
fn input_backing_bytes(input: &ExternalForeInput) -> Result<usize, String> {
    core::mem::size_of::<ExternalForeInput>()
        .checked_add(input.front_port_id.0.capacity())
        .and_then(|bytes| bytes.checked_add(input.bytes.capacity()))
        .ok_or_else(|| "IMAGE input backing size overflow".into())
}
