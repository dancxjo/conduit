//! Transactional inventory and resource custody for Root-retained native workers.
//! Descriptors confer no possession; the unsafe installer requires actual ownership.
use conduit_core::*;
/// Affine reservation of one verified fragment on the actual installed Host.
pub struct NativeFragmentReservation {
    scope: (HostId, BootId, OfferGeneration),
    plan: PlanId,
    fragment: FragmentId,
    reservation: crate::kernel_preparation::KernelResourceReservation,
}
impl NativeFragmentReservation {
    pub fn matches_fragment(&self, fragment: &PlanFragment) -> bool {
        self.scope.0 == fragment.host_id
            && self.scope.1 == fragment.boot_id
            && self.scope.2 == fragment.offer_generation
            && self.plan == fragment.plan_id
            && self.fragment == fragment.fragment_id
            && verify_plan_fragment(fragment)
    }
}
impl crate::StdHost {
    /// True only when the actual global resource ledger has no outstanding leases.
    pub fn native_resources_idle(&self) -> bool {
        self.kernel_resources.is_idle()
    }

    /// # Safety
    /// Root must retain the actual initialized providers for every advertised
    /// capability and resource independently of received metadata. Installation
    /// occurs before publication; each mechanism family has at most four slots.
    /// This inventory entrance grants no execution and binds no received IDs.
    #[allow(clippy::result_large_err)]
    pub unsafe fn install_retained_native_base(
        &mut self,
        base: BaseProviderEntry,
        native_kind: Option<Kind>,
    ) -> Result<(), (BaseProviderEntry, String)> {
        let result = (|| -> Result<_, String> {
            if !self.kernel_resources.is_idle() || base.lifecycle != BaseLifecycle::Ready {
                return Err("retained native installation requires idle Ready ownership".into());
            }
            if self
                .base_registry
                .entries()
                .iter()
                .filter(|entry| entry.mechanism_family == base.mechanism_family)
                .count()
                >= 4
            {
                return Err("retained native mechanism capacity exhausted".into());
            }
            let native_call = if let Some(kind) = native_kind.as_ref() {
                kind.validate()
                    .map_err(|error| format!("retained native Kind: {error:?}"))?;
                let offers = base
                    .capabilities
                    .iter()
                    .filter(|offer| offer.kind_id == kind.kind_id)
                    .collect::<Vec<_>>();
                let [offer] = offers.as_slice() else {
                    return Err("native Kind needs one exact offered Back".into());
                };
                if self.retained_native_calls.len() >= 4
                    || offer.inputs != kind.inputs
                    || offer.outputs != kind.outputs
                    || offer.semantic_contract != kind.semantic_contract()
                    || offer.host_calls.len() != 1
                    || offer.host_calls[0].maximum_in_flight != 1
                {
                    return Err(
                        "retained native call differs from initialized Kind or finite capacity"
                            .into(),
                    );
                }
                Some((offer.capability_id.clone(), kind.clone()))
            } else {
                None
            };
            let mut registry = self.base_registry.clone();
            registry
                .register(base.clone())
                .map_err(|e| format!("retained native registry: {e:?}"))?;
            let mut advertisement = self.advertisement.clone();
            advertisement.offer_generation.0 = advertisement
                .offer_generation
                .0
                .checked_add(1)
                .ok_or("native inventory generation exhausted")?;
            registry
                .project_base_ready_into(&base.base_id, &mut advertisement)
                .map_err(|e| format!("retained native advertisement: {e:?}"))?;
            advertisement
                .resources
                .sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
            advertisement
                .capabilities
                .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
            let ledger = crate::kernel_preparation::KernelResourceLedger::new(&advertisement)?;
            Ok((registry, advertisement, ledger, native_call))
        })();
        match result {
            Ok((registry, advertisement, ledger, native_call)) => {
                if let Some(call) = native_call {
                    self.retained_native_calls.push(call);
                }
                self.base_registry = registry;
                self.advertisement = advertisement;
                self.kernel_resources = ledger;
                Ok(())
            }
            Err(error) => Err((base, error)),
        }
    }
    pub fn reserve_native_fragment(
        &mut self,
        fragment: &PlanFragment,
    ) -> Result<NativeFragmentReservation, String> {
        let reservation = self
            .kernel_resources
            .prepare_and_reserve(&self.advertisement, fragment)?;
        Ok(NativeFragmentReservation {
            plan: fragment.plan_id.clone(),
            fragment: fragment.fragment_id.clone(),
            scope: (
                self.advertisement.host_id.clone(),
                self.advertisement.boot_id.clone(),
                self.advertisement.offer_generation,
            ),
            reservation,
        })
    }
    pub fn release_native_fragment(
        &mut self,
        lease: NativeFragmentReservation,
    ) -> Result<(), String> {
        if lease.scope
            != (
                self.advertisement.host_id.clone(),
                self.advertisement.boot_id.clone(),
                self.advertisement.offer_generation,
            )
        {
            return Err("native reservation returned to another Host/Boot/inventory".into());
        }
        self.kernel_resources.release(lease.reservation)
    }
}
