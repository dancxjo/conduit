//! Idle, transactional admission of one actually initialized Linux SDL display.
//! The retained token is a lifetime observation, not a Mask issuer or authority.
use crate::{kernel_preparation::KernelResourceLedger, StdHost};
use conduit_core::{
    BaseEnforcementClass, BaseLifecycle, BaseProviderAdvertisement, BaseProviderBinding,
    BaseProviderEntry, BootId, HostAdvertisement, HostId, OfferGeneration, ResourceOffer,
};
use conduit_linux_display::{DisplayClaimRefusal, DisplayHostClaim, InitializedDisplay};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayAttachmentRefusal {
    HostActive,
    DisplayLost,
    DisplayAlreadyClaimed,
    UnsupportedBackend,
    InvalidGeometry,
    ExistingAttachment,
    RefreshRequired,
    GenerationExhausted,
    BindingChanged,
    Registry(String),
    Ledger(String),
}
enum DisplayPolicy {
    Kmsdrm,
    #[cfg(test)]
    DiagnosticDummy,
}

/// Constructible only from actual scanout initialization and an idle Host
/// transition. No deserializer or public descriptor-only constructor exists.
pub struct ReadyShowBase {
    claim: DisplayHostClaim,
    host_id: HostId,
    boot_id: BootId,
    entry: BaseProviderEntry,
}
impl ReadyShowBase {
    pub fn host_id(&self) -> &HostId {
        &self.host_id
    }
    pub fn boot_id(&self) -> &BootId {
        &self.boot_id
    }
    pub fn entry(&self) -> &BaseProviderEntry {
        &self.entry
    }
    pub fn binding(&self) -> BaseProviderBinding {
        descriptor(&self.entry).binding()
    }
    pub fn resource(&self) -> &ResourceOffer {
        &self.entry.resources[0]
    }
    pub fn initialized(&self) -> &InitializedDisplay {
        self.claim.initialized()
    }
    pub fn geometry(&self) -> (u32, u32) {
        (self.initialized().width(), self.initialized().height())
    }
    fn matches(&self, host: &StdHost) -> bool {
        self.initialized().is_live() && self.binding_matches(host)
    }
    fn binding_matches(&self, host: &StdHost) -> bool {
        self.host_id == host.advertisement.host_id
            && self.boot_id == host.advertisement.boot_id
            && self.entry.lifecycle == BaseLifecycle::Ready
            && host
                .base_registry
                .entries()
                .iter()
                .any(|entry| entry == &self.entry)
            && host
                .advertisement
                .bases
                .iter()
                .any(|base| descriptor_matches(base, &self.entry))
            && self.entry.resources.iter().all(|resource| {
                host.advertisement
                    .resources
                    .iter()
                    .filter(|r| r.pool_id == resource.pool_id)
                    .eq(std::iter::once(resource))
            })
            && self.entry.capabilities.iter().all(|capability| {
                host.advertisement
                    .capabilities
                    .iter()
                    .filter(|c| c.capability_id == capability.capability_id)
                    .eq(std::iter::once(capability))
            })
    }
}
impl StdHost {
    /// Requires actual SDL kmsdrm initialization. It does not independently
    /// prove physical HDMI presentation or issue possession/Mask capabilities.
    pub fn attach_initialized_display(
        &mut self,
        initialized: InitializedDisplay,
    ) -> Result<&ReadyShowBase, DisplayAttachmentRefusal> {
        self.attach_display(initialized, DisplayPolicy::Kmsdrm)
    }
    fn attach_display(
        &mut self,
        initialized: InitializedDisplay,
        policy: DisplayPolicy,
    ) -> Result<&ReadyShowBase, DisplayAttachmentRefusal> {
        use DisplayAttachmentRefusal as E;
        if !self.kernel_resources.is_idle() {
            return Err(E::HostActive);
        }
        if !initialized.is_live() {
            return Err(E::DisplayLost);
        }
        let backend_accepted = match policy {
            DisplayPolicy::Kmsdrm => initialized.is_kmsdrm(),
            #[cfg(test)]
            DisplayPolicy::DiagnosticDummy => initialized.backend() == "dummy",
        };
        if !backend_accepted {
            return Err(E::UnsupportedBackend);
        }
        validate_geometry(&initialized)?;
        if let Some(prior) = &self.native_display {
            return Err(if prior.initialized().is_live() {
                E::ExistingAttachment
            } else {
                E::RefreshRequired
            });
        }
        let mut advertisement = self.advertisement.clone();
        advertisement.offer_generation = next_generation(&advertisement)?;
        let nonce = initialized.instance_nonce();
        let base = format!("std/base/linux-display/{nonce}");
        let entry = BaseProviderEntry {
            base_id: base.as_str().into(),
            provider_instance_id: format!("std/linux-display/{nonce}").as_str().into(),
            provider_generation: advertisement.offer_generation.0,
            implementation_id: "std/linux-sdl-display@1".into(),
            mechanism_family: "std.base/linux-display@1".into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![],
            resources: vec![ResourceOffer {
                pool_id: format!("{base}/surface").as_str().into(),
                class_id: conduit_face::SHOW_RESOURCE_CLASS.into(),
                capacity_units: 1,
                compute: None,
                content: None,
            }],
        };
        let mut registry = self.base_registry.clone();
        registry
            .register(entry.clone())
            .map_err(|e| E::Registry(format!("{e:?}")))?;
        registry
            .project_base_ready_into(&entry.base_id, &mut advertisement)
            .map_err(|e| E::Registry(format!("{e:?}")))?;
        advertisement.resources.sort();
        let ledger = KernelResourceLedger::new(&advertisement).map_err(E::Ledger)?;
        // Recheck the shared lifetime immediately before committing staged state.
        if !initialized.is_live() {
            return Err(E::DisplayLost);
        }
        let claim = initialized.claim_host().map_err(|error| match error {
            DisplayClaimRefusal::Lost => E::DisplayLost,
            DisplayClaimRefusal::AlreadyClaimed => E::DisplayAlreadyClaimed,
        })?;
        let ready = ReadyShowBase {
            claim,
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            entry,
        };
        self.advertisement = advertisement;
        self.base_registry = registry;
        self.kernel_resources = ledger;
        self.native_display = Some(ready);
        Ok(self.native_display.as_ref().expect("attachment committed"))
    }
    /// Borrow only the live exact current registry/resource binding. A retained
    /// descriptor or serialized advertisement cannot manufacture this reference.
    pub fn ready_native_display(&self) -> Option<&ReadyShowBase> {
        self.native_display
            .as_ref()
            .filter(|ready| ready.matches(self))
    }
    /// Retract only this lost display's offers, after ordinary Stop/drain has
    /// made the resource ledger idle. Loss never resurrects the old token.
    pub fn refresh_native_display(&mut self) -> Result<bool, DisplayAttachmentRefusal> {
        use DisplayAttachmentRefusal as E;
        let Some(ready) = &self.native_display else {
            return Ok(false);
        };
        if ready.initialized().is_live() {
            return if ready.matches(self) {
                Ok(false)
            } else {
                Err(E::BindingChanged)
            };
        }
        if !self.kernel_resources.is_idle() {
            return Err(E::HostActive);
        }
        if !ready.binding_matches(self) {
            return Err(E::BindingChanged);
        }
        let mut registry = self.base_registry.clone();
        let entry = &ready.entry;
        registry
            .set_lifecycle(
                &entry.base_id,
                &entry.provider_instance_id,
                entry.provider_generation,
                BaseLifecycle::Lost,
            )
            .map_err(|e| E::Registry(format!("{e:?}")))?;
        let mut advertisement = self.advertisement.clone();
        advertisement.offer_generation = next_generation(&advertisement)?;
        advertisement
            .bases
            .retain(|base| base.base_id != entry.base_id);
        advertisement.resources.retain(|resource| {
            !entry
                .resources
                .iter()
                .any(|own| own.pool_id == resource.pool_id)
        });
        advertisement.capabilities.retain(|capability| {
            !entry
                .capabilities
                .iter()
                .any(|own| own.capability_id == capability.capability_id)
        });
        advertisement.resources.sort();
        let ledger = KernelResourceLedger::new(&advertisement).map_err(E::Ledger)?;
        self.advertisement = advertisement;
        self.base_registry = registry;
        self.kernel_resources = ledger;
        self.native_display = None;
        Ok(true)
    }
}
fn validate_geometry(display: &InitializedDisplay) -> Result<(), DisplayAttachmentRefusal> {
    let (width, height) = (display.width(), display.height());
    let (window_width, window_height) = display.window_size();
    if !(320..=16_384).contains(&width)
        || !(240..=16_384).contains(&height)
        || width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(4))
            .is_none_or(|n| n > 8 * 1024 * 1024)
        || window_width == 0
        || window_height == 0
        || window_width > 16_384
        || window_height > 16_384
    {
        return Err(DisplayAttachmentRefusal::InvalidGeometry);
    }
    Ok(())
}
fn next_generation(
    advertisement: &HostAdvertisement,
) -> Result<OfferGeneration, DisplayAttachmentRefusal> {
    advertisement
        .offer_generation
        .0
        .checked_add(1)
        .map(OfferGeneration)
        .ok_or(DisplayAttachmentRefusal::GenerationExhausted)
}
fn descriptor_matches(base: &BaseProviderAdvertisement, entry: &BaseProviderEntry) -> bool {
    base.base_id == entry.base_id
        && base.provider_instance_id == entry.provider_instance_id
        && base.provider_generation == entry.provider_generation
        && base.implementation_id == entry.implementation_id
        && base.mechanism_family == entry.mechanism_family
        && base.enforcement_class == entry.enforcement_class
        && base.lifecycle == entry.lifecycle
        && base
            .capability_ids
            .iter()
            .eq(entry.capabilities.iter().map(|offer| &offer.capability_id))
        && base
            .resource_pool_ids
            .iter()
            .eq(entry.resources.iter().map(|offer| &offer.pool_id))
}
fn descriptor(entry: &BaseProviderEntry) -> BaseProviderAdvertisement {
    BaseProviderAdvertisement {
        base_id: entry.base_id.clone(),
        provider_instance_id: entry.provider_instance_id.clone(),
        provider_generation: entry.provider_generation,
        implementation_id: entry.implementation_id.clone(),
        mechanism_family: entry.mechanism_family.clone(),
        enforcement_class: entry.enforcement_class,
        lifecycle: entry.lifecycle,
        capability_ids: entry
            .capabilities
            .iter()
            .map(|cap| cap.capability_id.clone())
            .collect(),
        resource_pool_ids: entry
            .resources
            .iter()
            .map(|resource| resource.pool_id.clone())
            .collect(),
    }
}
#[cfg(test)]
mod tests;
