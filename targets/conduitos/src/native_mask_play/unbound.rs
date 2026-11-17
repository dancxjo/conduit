//! Actual plan-independent stores; bind an exact admitted routed Plan later.
use super::*;
use alloc::boxed::Box;
use conduit_kernel::scheduler::prepared_session::OptionalOutputStorage;
use core::mem::MaybeUninit;

/// Affine stores, not a fabricated scheduler, Plan or readiness advertisement.
/// Dropping before binding releases unused allocation; no Play has begun.
/// Metadata/lowering/stack preparation peak is not included in its heap receipt.
pub struct UnboundNativeMaskStorage {
    kernel: Box<MaybeUninit<Scheduler>>,
    values: HostedValueStore,
    signs: FixedSignLog<SIGNS>,
    optional: OptionalOutputStorage<CORDS>,
    profile: NativeMaskStorageProfile,
    backing: NativeMaskBackingReceipt,
}
impl UnboundNativeMaskStorage {
    pub fn prepare(
        profile: NativeMaskStorageProfile,
        maximum_bytes: usize,
    ) -> Result<Self, NativeMaskPlayError> {
        let required = RetainedNativeMaskStorage::required_store_bytes(profile)?;
        if required > maximum_bytes {
            return Err(NativeMaskPlayError::Pressure);
        }
        let live_bytes = profile
            .bytes()
            .checked_mul(6)
            .and_then(|bytes| bytes.checked_add(2 * INTERACTION_VALUE_BYTES as u32))
            .ok_or(NativeMaskPlayError::Pressure)?;
        let values = HostedValueStore::new(VALUES as u16, profile.bytes(), live_bytes)
            .map_err(|_| NativeMaskPlayError::Value)?;
        let value_storage_bytes = values.reserved_storage_bytes();
        let scheduler_inline_bytes = core::mem::size_of::<Scheduler>();
        let optional_output_storage_bytes = OptionalOutputStorage::<CORDS>::required_bytes();
        let managed_store_bytes = scheduler_inline_bytes
            .checked_add(value_storage_bytes)
            .and_then(|bytes| bytes.checked_add(optional_output_storage_bytes))
            .ok_or(NativeMaskPlayError::Pressure)?;
        if managed_store_bytes > maximum_bytes {
            return Err(NativeMaskPlayError::Pressure);
        }
        let kernel = Box::<Scheduler>::new_uninit();
        let signs = FixedSignLog::new_with_remote_storage(
            (SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
            32,
            conduit_kernel::remote_sign_storage_bytes(32).ok_or(NativeMaskPlayError::Kernel)?,
        )
        .map_err(|_| NativeMaskPlayError::Kernel)?;
        let optional = OptionalOutputStorage::prepare();
        Ok(Self {
            kernel,
            values,
            signs,
            optional,
            profile,
            backing: NativeMaskBackingReceipt {
                scheduler_inline_bytes,
                value_storage_bytes,
                optional_output_storage_bytes,
                managed_store_bytes,
            },
        })
    }
    pub fn backing(&self) -> NativeMaskBackingReceipt {
        self.backing
    }
    /// Bind only after the caller admits this exact routed Plan in its final
    /// ledger. No normalized substitute Plan is stored or executed here.
    pub fn bind(
        self,
        planned: &PlannedMaskPlot,
    ) -> Result<RetainedNativeMaskStorage, NativeMaskPlayError> {
        storage::validate_profile(planned, self.profile)?;
        let backing = self.backing();
        let fragment = planned
            .plan
            .fragments
            .first()
            .ok_or(NativeMaskPlayError::Plan)?;
        let lowered = lower_plan_fragment(fragment).map_err(|_| NativeMaskPlayError::Plan)?;
        let scheduler =
            scheduler_with_storage(fragment, &lowered, self.profile, self.values, self.signs)?;
        let scheduler = Box::write(self.kernel, scheduler);
        RetainedNativeMaskStorage::from_prepared_kernel(
            planned,
            &lowered,
            self.profile,
            scheduler,
            self.optional,
            backing,
        )
    }
    #[cfg(test)]
    pub(super) fn backing_identity(&self) -> usize {
        self.kernel.as_ptr() as usize
    }
}

#[cfg(test)]
#[path = "unbound/tests.rs"]
mod tests;
