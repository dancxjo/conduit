//! Explicit owner accounting for a prepared worker Back.
use crate::prelude::*;
use conduit_kernel::scheduler::StepBack;
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS;

pub struct PreparedWorkerBack {
    back: Box<dyn StepBack<PORTS> + Send>,
    retained_bytes: usize,
}
impl PreparedWorkerBack {
    /// The implementation owner reports every additional owned allocation.
    /// The Box's concrete inline pointee is counted here. Shared resources
    /// require separate admission; this receipt never grants their authority.
    pub fn new(
        back: Box<dyn StepBack<PORTS> + Send>,
        owned_heap_bytes: usize,
    ) -> Result<Self, String> {
        let retained_bytes = core::mem::size_of_val(&*back)
            .checked_add(owned_heap_bytes)
            .ok_or_else(|| "worker Back memory overflow".to_string())?;
        Ok(Self {
            back,
            retained_bytes,
        })
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub(crate) fn into_parts(self) -> (Box<dyn StepBack<PORTS> + Send>, usize) {
        (self.back, self.retained_bytes)
    }
}
