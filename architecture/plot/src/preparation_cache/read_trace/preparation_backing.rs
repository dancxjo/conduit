use super::*;
impl TraceHandle {
    pub(crate) fn owned_preparation_backing_bytes(&self) -> Option<usize> {
        let Some(trace) = &self.0 else {
            return Some(0);
        };
        core::mem::size_of::<ReadTrace>()
            .checked_add(2 * core::mem::size_of::<usize>())?
            .checked_add(
                trace
                    .slots
                    .capacity()
                    .checked_mul(core::mem::size_of::<Slot>())?,
            )?
            .checked_add(
                trace
                    .bytes
                    .capacity()
                    .checked_mul(core::mem::size_of::<AtomicU8>())?,
            )
    }
}
