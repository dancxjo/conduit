//! Retained finite ingress shares the selected IMAGE's existing memory owner.
use super::*;

/// Already prepared ingress, with its owned backing charged until actual drop.
/// Borrowing this input neither authorizes its content nor acknowledges delivery.
/// There is no conversion that separates its bytes from their memory custody.
pub type ImageForeInput = ImagePreparedValue<ExternalForeInput>;
impl ImagePreparedValue<ExternalForeInput> {
    pub fn input(&self) -> &ExternalForeInput {
        self.value()
    }
}
impl StdHost {
    /// Verify original cooperative backing custody without trusting copied
    /// Host/Boot/IMAGE labels. This grants no input, Source or execution authority.
    pub fn verify_selected_image_fore_input(&self, input: &ImageForeInput) -> Result<(), String> {
        self.verify_selected_image_prepared_value(input)
    }

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
        // SAFETY: the caller qualifies complete input preparation; this keeps
        // that entire envelope charged until actual capacities are checked.
        let mut input = unsafe {
            self.prepare_selected_image_value(preparation_bytes, preparation_bytes, prepare)
        }?;
        let retained = input_backing_bytes(input.input())?;
        if retained > preparation_bytes {
            return Err("IMAGE input backing exceeds admitted preparation envelope".into());
        }
        input.shrink_qualified_retention(retained);
        Ok(input)
    }
}
fn input_backing_bytes(input: &ExternalForeInput) -> Result<usize, String> {
    core::mem::size_of::<ExternalForeInput>()
        .checked_add(input.front_port_id.0.capacity())
        .and_then(|bytes| bytes.checked_add(input.bytes.capacity()))
        .ok_or_else(|| "IMAGE input backing size overflow".into())
}
