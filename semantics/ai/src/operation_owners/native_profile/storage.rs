//! Private selected-factory storage; shared profile/offer roots counted once.
//! Requested payload storage excludes Arc headers, root factory structure,
//! allocator bookkeeping and stack; the final runtime must charge those too.
use super::*;
use core::mem::size_of;
#[derive(Debug, Clone, Copy)]
pub struct NativeFactoryStorage {
    pub selected_array_capacity_bytes: usize,
    pub shared_profiles: usize,
    pub shared_offers: usize,
    pub requested_heap_payload_bytes: usize,
}
impl NativeProfileOperationFactory {
    pub fn storage(&self) -> Result<NativeFactoryStorage, conduit_core::PlanStorageRefusal> {
        let array = self
            .selected
            .capacity()
            .checked_mul(size_of::<(PlacementId, SelectedProfile)>())
            .ok_or(conduit_core::PlanStorageRefusal::Overflow)?;
        let mut bytes = array
            .checked_add(self.implementation.owned_heap_bytes())
            .ok_or(conduit_core::PlanStorageRefusal::Overflow)?;
        let mut profiles = 0;
        let mut offers = 0;
        for (index, (placement, selected)) in self.selected.iter().enumerate() {
            bytes = bytes
                .checked_add(placement.owned_heap_bytes())
                .ok_or(conduit_core::PlanStorageRefusal::Overflow)?;
            if !self.selected[..index]
                .iter()
                .any(|(_, prior)| Arc::ptr_eq(&prior.profile, &selected.profile))
            {
                profiles += 1;
                bytes = bytes
                    .checked_add(size_of::<PreparedNativeProfile>())
                    .and_then(|b| b.checked_add(selected.profile.owned_heap_bytes()))
                    .ok_or(conduit_core::PlanStorageRefusal::Overflow)?;
            }
            if !self.selected[..index]
                .iter()
                .any(|(_, prior)| Arc::ptr_eq(&prior.offer, &selected.offer))
            {
                offers += 1;
                let offer_bytes = conduit_core::capability_offer_owned_heap_bytes(&selected.offer)?;
                bytes = bytes
                    .checked_add(size_of::<CapabilityOffer>())
                    .and_then(|b| b.checked_add(offer_bytes))
                    .ok_or(conduit_core::PlanStorageRefusal::Overflow)?;
            }
        }
        Ok(NativeFactoryStorage {
            selected_array_capacity_bytes: array,
            shared_profiles: profiles,
            shared_offers: offers,
            requested_heap_payload_bytes: bytes,
        })
    }
}
#[derive(Debug)]
pub enum NativeFactoryPreparationRefusal {
    Capacity,
    Selection(String),
    Preparation(crate::native_profile::NativeProfilePreparationRefusal),
}
impl NativeProfileOperationFactory {
    /// Explicit bounded entrance; the legacy KernelOperationFactory entrance is
    /// unchanged. Final aggregate admission must select this entrance deliberately.
    pub fn prepare_with_storage_limits(
        &self,
        gear: &PlannedGear,
        mut limits: crate::native_profile::NativeProfilePreparationLimits,
    ) -> Result<
        (
            Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>,
            crate::native_profile::NativeProfilePreparationReceipt,
        ),
        NativeFactoryPreparationRefusal,
    > {
        let selected = self
            .selected(gear)
            .map_err(NativeFactoryPreparationRefusal::Selection)?;
        let root_bytes = size_of::<NativeProfileBack>();
        limits.maximum_preparation_requested_bytes = limits
            .maximum_preparation_requested_bytes
            .checked_sub(root_bytes)
            .ok_or(NativeFactoryPreparationRefusal::Capacity)?;
        limits.maximum_retained_heap_bytes = limits
            .maximum_retained_heap_bytes
            .checked_sub(root_bytes)
            .ok_or(NativeFactoryPreparationRefusal::Capacity)?;
        let (back, mut receipt) = NativeProfileBack::prepare_selected_with_storage_limits(
            &selected.profile,
            selected.flow,
            limits,
        )
        .map_err(NativeFactoryPreparationRefusal::Preparation)?;
        receipt.preparation_requested_bytes_bound = receipt
            .preparation_requested_bytes_bound
            .checked_add(root_bytes)
            .ok_or(NativeFactoryPreparationRefusal::Capacity)?;
        receipt.retained_heap_bytes_bound = receipt
            .retained_heap_bytes_bound
            .checked_add(root_bytes)
            .ok_or(NativeFactoryPreparationRefusal::Capacity)?;
        receipt.retained_accounted_heap_bytes = receipt
            .retained_accounted_heap_bytes
            .checked_add(root_bytes)
            .ok_or(NativeFactoryPreparationRefusal::Capacity)?;
        Ok((Box::new(back), receipt))
    }
}
