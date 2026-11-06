//! One cooperative IMAGE counter owns synchronous Root preparation and result.
use super::*;

/// Affine retained preparation, not Source, adoption, provider or Play authority.
/// Only borrowed access is exposed; owned data drops before its original charge.
/// Captures, shared stores and other caller metadata need independent custody.
pub struct ImagePreparedValue<T> {
    value: T,
    memory: ImageMemoryLease,
}
impl<T> ImagePreparedValue<T> {
    pub fn value(&self) -> &T {
        &self.value
    }

    pub(super) fn shrink_qualified_retention(&mut self, bytes: usize) {
        self.memory
            .retain_result(bytes + ImageMemoryLease::owned_backing_bytes());
    }
}
impl StdHost {
    /// Verify the original memory owner, rather than copied identity labels.
    /// This verifies cooperative backing custody only.
    pub fn verify_selected_image_prepared_value<T>(
        &self,
        value: &ImagePreparedValue<T>,
    ) -> Result<(), String> {
        self.kernel_resources
            .require_input_memory_owner(&value.memory)
    }

    /// Admit a complete synchronous preparation envelope before invoking Root's
    /// factory; retain its independently qualified result envelope until drop.
    /// Invalid bounds and pressure refuse before the factory runs. This uses
    /// the same counter as prepared inputs, providers and admitted execution.
    ///
    /// # Safety
    /// Root must independently qualify `preparation_bytes` for the complete
    /// factory peak and `retained_bytes` for all owned result backing, including
    /// allocated spare capacities and the value itself. The result must stay
    /// within that retained envelope for its entire lifetime, including bounded
    /// destruction; Root must constrain any interior or shared mutable backing.
    /// Shared backing and
    /// captures outside the factory need separate custody that outlives their
    /// use. All effects/workers must quiesce before return, refusal or unwinding.
    /// The envelope is cooperative admission, not allocator containment, process
    /// RSS, asynchronous retirement, or authority to check/adopt/execute Source.
    pub unsafe fn prepare_selected_image_value<T>(
        &mut self,
        preparation_bytes: usize,
        retained_bytes: usize,
        prepare: impl FnOnce() -> Result<T, String>,
    ) -> Result<ImagePreparedValue<T>, String> {
        if preparation_bytes == 0
            || retained_bytes > preparation_bytes
            || retained_bytes < core::mem::size_of::<T>()
        {
            return Err("IMAGE preparation/retention envelope is invalid".into());
        }
        let metadata = ImageMemoryLease::owned_backing_bytes();
        let admitted = preparation_bytes
            .checked_add(metadata)
            .ok_or("IMAGE preparation size overflow")?;
        let mut memory = self
            .kernel_resources
            .reserve_managed_base_memory(admitted)?;
        let value = prepare()?;
        // Retained <= preparation, so its already admitted metadata addition
        // cannot overflow. The value drops before this lease on every exit.
        memory.retain_result(retained_bytes + metadata);
        Ok(ImagePreparedValue { value, memory })
    }
}
