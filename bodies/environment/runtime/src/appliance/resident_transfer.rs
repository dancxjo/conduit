//! Heap destination for transferring already initialized resident software.
//! This creates no native authority, Body start, scheduler or execution effect.
use super::*;
use core::mem::MaybeUninit;

impl<V: ValueStorage> PreparedResidentSoftware<V> {
    /// Refusal preserves both retained slots. The caller must admit this output
    /// box and transfer scratch before calling; allocation is preparation only.
    /// Drivers, selectors and existing value backing transfer without rebuilding.
    pub fn bind_boxed_retained(
        slot: &mut Option<Box<Self>>,
        selected: &mut Option<Box<ValidatedAssignedAppliance>>,
    ) -> Result<Box<PreparedAssignedAppliance<V>>, NativePreparationRefusal> {
        if slot
            .as_ref()
            .is_some_and(|software| software.values.compact_canonical_layout().is_some())
        {
            return Err(NativePreparationRefusal::Configuration);
        }
        slot.as_ref()
            .ok_or(NativePreparationRefusal::Initialization)?
            .check_assignment(
                selected
                    .as_ref()
                    .ok_or(NativePreparationRefusal::Initialization)?,
            )?;
        let mut output: Box<MaybeUninit<PreparedAssignedAppliance<V>>> = Box::new_uninit();
        // All refusal paths precede allocation/transfer. Private construction
        // invariants and the preflight guarantee complete drivers below.
        let Self {
            construction,
            drivers,
            selectors,
            values,
        } = *slot.take().expect("preflighted software");
        let selected = *selected.take().expect("preflighted assignment");
        let Construction {
            roles,
            count,
            period_ms,
            ..
        } = construction;
        let target = output.as_mut_ptr();
        // SAFETY: aligned exclusive destination. Every field is initialized once
        // without reading uninitialized storage; no fallible operation follows.
        // Both source boxes retire, while all initialized values move exactly once.
        unsafe {
            core::ptr::addr_of_mut!((*target).tables).write(selected.tables);
            core::ptr::addr_of_mut!((*target).drivers)
                .write((*drivers).map(|driver| driver.expect("preflighted complete driver set")));
            core::ptr::addr_of_mut!((*target).selectors).write(*selectors);
            core::ptr::addr_of_mut!((*target).values).write(values);
            core::ptr::addr_of_mut!((*target).roles).write(roles);
            core::ptr::addr_of_mut!((*target).count).write(count);
            core::ptr::addr_of_mut!((*target).period_ms).write(period_ms);
            Ok(output.assume_init())
        }
    }
}
