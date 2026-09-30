//! Narrow product-facing adapter over the canonical registry of external-effect Bases.

use alloc::{borrow::ToOwned, format, vec, vec::Vec};

use conduit_core::{
    BaseEnforcementClass, BaseImplementationId, BaseInstanceId, BaseLifecycle, BaseProviderEntry,
    BaseRegistry, BaseRegistryLimits, HostBaseId, HostBaseKindId, ResourceClassId, ResourceOffer,
    ResourcePoolId,
};

use crate::{arch::UsbDevice, identity, offer::HostOffer};

const MAXIMUM_EFFECT_BASES: u16 = 5;
const INPUT_CONTROLLER_FAMILY: &str = "conduitos.base/input-controller@1";
pub const FRAMEBUFFER_RESOURCE_CLASS: &str = conduit_presentation::SHOW_RESOURCE_CLASS;

/// Exact discovered surface provider plus issuer-private material. The key is
/// never descriptive Base truth and is deliberately not serializable or
/// exposed through inspection.
#[derive(Clone)]
pub struct NativeSurfaceProvider {
    pub entry: BaseProviderEntry,
    pub(crate) issuer_key: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectFamily {
    Keyboard,
    Pointer,
    Audio,
    Storage,
    Line,
    Framebuffer,
    Network,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectBaseRefusal {
    Unavailable,
    InvalidProvider,
}

pub struct NativeProductBases {
    registry: BaseRegistry,
    effects: Vec<EffectBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EffectBinding {
    family: EffectFamily,
    base_id: HostBaseId,
}

impl NativeProductBases {
    pub fn observe(
        offer: &HostOffer<'_>,
        framebuffer: &conduit_observatory::FramebufferBasis,
        usb_line: Option<&UsbDevice>,
    ) -> Result<Self, EffectBaseRefusal> {
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: MAXIMUM_EFFECT_BASES,
            maximum_capabilities_per_base: 0,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: 1,
            maximum_advertised_resources: 1,
        })
        .map_err(|_| EffectBaseRefusal::InvalidProvider)?;
        let mut effects = Vec::with_capacity(usize::from(MAXIMUM_EFFECT_BASES));
        if let Some(value) = offer.keyboard {
            register_effect(
                &mut registry,
                &mut effects,
                EffectFamily::Keyboard,
                identity::hex(&value.realization.controller_id),
                identity::hex(&value.realization.controller_id),
                offer.generation,
                value.realization.mechanism.base_implementation(),
                INPUT_CONTROLLER_FAMILY,
                Vec::new(),
            )?;
        }
        if let Some(value) = offer.pointer {
            register_effect(
                &mut registry,
                &mut effects,
                EffectFamily::Pointer,
                identity::hex(&value.realization.controller_id),
                identity::hex(&value.realization.controller_id),
                offer.generation,
                value.realization.mechanism.base_implementation(),
                INPUT_CONTROLLER_FAMILY,
                Vec::new(),
            )?;
        }
        if let Some(value) = offer.pc_speaker {
            let base = identity::hex(&value.realization.base_id);
            register_effect(
                &mut registry,
                &mut effects,
                EffectFamily::Audio,
                base.clone(),
                format!("{base}/provider/{}", offer.generation),
                offer.generation,
                crate::pc_speaker_offer::PC_SPEAKER_IMPLEMENTATION,
                family_kind(EffectFamily::Audio),
                Vec::new(),
            )?;
        }
        let framebuffer_base = framebuffer.base_id.as_str().to_owned();
        if let Some(device) = usb_line {
            register_effect(
                &mut registry,
                &mut effects,
                EffectFamily::Line,
                crate::usb_line_offer::USB_FTDI_BASE.into(),
                format!(
                    "conduitos/usb-line/{}/{}",
                    device.root_port, device.attachment_epoch
                ),
                u64::from(device.attachment_epoch),
                crate::usb_line_offer::USB_FTDI_BASE,
                family_kind(EffectFamily::Line),
                Vec::new(),
            )?;
        }
        register_effect(
            &mut registry,
            &mut effects,
            EffectFamily::Framebuffer,
            framebuffer_base.clone(),
            format!("{framebuffer_base}/provider/1"),
            1,
            "conduitos/framebuffer@1",
            family_kind(EffectFamily::Framebuffer),
            vec![framebuffer_resource(&framebuffer_base)],
        )?;
        Ok(Self { registry, effects })
    }

    pub fn entries(&self) -> &[BaseProviderEntry] {
        self.registry.entries()
    }

    pub fn require(&self, family: EffectFamily) -> Result<&BaseProviderEntry, EffectBaseRefusal> {
        let binding = self
            .effects
            .iter()
            .find(|binding| binding.family == family)
            .ok_or(EffectBaseRefusal::Unavailable)?;
        self.registry
            .entries()
            .iter()
            .find(|entry| entry.base_id == binding.base_id)
            .ok_or(EffectBaseRefusal::Unavailable)
    }

    /// Returns current typed resource truth owned by the discovered Base.
    ///
    /// A matching class elsewhere in the registry is deliberately insufficient:
    /// the resource must belong to the exact Base selected for this effect family.
    pub fn require_resource(
        &self,
        family: EffectFamily,
        class_id: &str,
    ) -> Result<&ResourceOffer, EffectBaseRefusal> {
        let entry = self.require(family)?;
        let mut resources = entry
            .resources
            .iter()
            .filter(|resource| resource.class_id.as_str() == class_id);
        let resource = resources.next().ok_or(EffectBaseRefusal::Unavailable)?;
        if resources.next().is_some() {
            return Err(EffectBaseRefusal::InvalidProvider);
        }
        Ok(resource)
    }

    pub fn framebuffer_provider(
        &self,
        issuer_key: [u8; 32],
    ) -> Result<NativeSurfaceProvider, EffectBaseRefusal> {
        if issuer_key == [0; 32] {
            return Err(EffectBaseRefusal::InvalidProvider);
        }
        let entry = self.require(EffectFamily::Framebuffer)?;
        self.require_resource(EffectFamily::Framebuffer, FRAMEBUFFER_RESOURCE_CLASS)?;
        Ok(NativeSurfaceProvider {
            entry: entry.clone(),
            issuer_key,
        })
    }
}

fn framebuffer_resource(base_id: &str) -> ResourceOffer {
    ResourceOffer {
        pool_id: ResourcePoolId::from(format!("{base_id}/surface")),
        class_id: ResourceClassId::from(FRAMEBUFFER_RESOURCE_CLASS),
        capacity_units: 1,
        compute: None,
        content: None,
    }
}

#[cfg(test)]
pub(crate) fn fixture_surface_provider() -> NativeSurfaceProvider {
    NativeSurfaceProvider {
        entry: BaseProviderEntry {
            base_id: HostBaseId::from("conduitos/test/framebuffer"),
            provider_instance_id: BaseInstanceId::from("conduitos/test/framebuffer/provider/1"),
            provider_generation: 1,
            implementation_id: BaseImplementationId::from("conduitos/framebuffer@1"),
            mechanism_family: HostBaseKindId::from(family_kind(EffectFamily::Framebuffer)),
            enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
            lifecycle: BaseLifecycle::Ready,
            capabilities: Vec::new(),
            resources: vec![framebuffer_resource("conduitos/test/framebuffer")],
        },
        issuer_key: [7; 32],
    }
}

#[allow(clippy::too_many_arguments)]
fn register_effect(
    registry: &mut BaseRegistry,
    effects: &mut Vec<EffectBinding>,
    family: EffectFamily,
    base_id: alloc::string::String,
    provider_instance_id: alloc::string::String,
    provider_generation: u64,
    implementation_id: &'static str,
    mechanism_family: &'static str,
    resources: Vec<ResourceOffer>,
) -> Result<(), EffectBaseRefusal> {
    let entry = BaseProviderEntry {
        base_id: HostBaseId::from(base_id),
        provider_instance_id: BaseInstanceId::from(provider_instance_id),
        provider_generation,
        implementation_id: BaseImplementationId::from(implementation_id),
        mechanism_family: HostBaseKindId::from(mechanism_family),
        enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: BaseLifecycle::Ready,
        capabilities: Vec::new(),
        resources,
    };
    if let Some(current) = registry
        .entries()
        .iter()
        .find(|current| current.base_id == entry.base_id)
    {
        if current.provider_instance_id != entry.provider_instance_id
            || current.provider_generation != entry.provider_generation
            || current.implementation_id != entry.implementation_id
            || current.mechanism_family != entry.mechanism_family
            || current.enforcement_class != entry.enforcement_class
            || current.lifecycle != entry.lifecycle
        {
            return Err(EffectBaseRefusal::InvalidProvider);
        }
    } else {
        registry
            .register(entry.clone())
            .map_err(|_| EffectBaseRefusal::InvalidProvider)?;
    }
    effects.push(EffectBinding {
        family,
        base_id: entry.base_id,
    });
    Ok(())
}

const fn family_kind(family: EffectFamily) -> &'static str {
    match family {
        EffectFamily::Keyboard => "conduitos.base/keyboard-input@1",
        EffectFamily::Pointer => "conduitos.base/pointer-input@1",
        EffectFamily::Audio => "conduitos.base/audio-output@1",
        EffectFamily::Storage => "conduitos.base/storage@1",
        EffectFamily::Line => "conduitos.base/line@1",
        EffectFamily::Framebuffer => "conduitos.base/framebuffer@1",
        EffectFamily::Network => "conduitos.base/network@1",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framebuffer_resource_identity_is_derived_from_the_discovered_base() {
        let resource = framebuffer_resource("boot-7/framebuffer-0");
        assert_eq!(resource.pool_id.as_str(), "boot-7/framebuffer-0/surface");
        assert_eq!(resource.class_id.as_str(), FRAMEBUFFER_RESOURCE_CLASS);
        assert_eq!(resource.capacity_units, 1);
    }

    #[test]
    fn unavailable_families_are_absent_and_authority_does_not_cross_entries() {
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 2,
            maximum_capabilities_per_base: 0,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: 1,
            maximum_advertised_resources: 1,
        })
        .unwrap();
        let mut effects = Vec::new();
        register_effect(
            &mut registry,
            &mut effects,
            EffectFamily::Keyboard,
            "base/key".into(),
            "provider/key".into(),
            4,
            "conduitos/key@1",
            family_kind(EffectFamily::Keyboard),
            Vec::new(),
        )
        .unwrap();
        register_effect(
            &mut registry,
            &mut effects,
            EffectFamily::Framebuffer,
            "base/frame".into(),
            "provider/frame".into(),
            2,
            "conduitos/frame@1",
            family_kind(EffectFamily::Framebuffer),
            vec![ResourceOffer {
                pool_id: ResourcePoolId::from("pool/frame"),
                class_id: ResourceClassId::from(FRAMEBUFFER_RESOURCE_CLASS),
                capacity_units: 1,
                compute: None,
                content: None,
            }],
        )
        .unwrap();
        let bases = NativeProductBases { registry, effects };
        assert_eq!(bases.entries().len(), 2);
        assert_eq!(
            bases
                .require(EffectFamily::Framebuffer)
                .unwrap()
                .base_id
                .as_str(),
            "base/frame"
        );
        assert_eq!(
            bases
                .require(EffectFamily::Keyboard)
                .unwrap()
                .provider_generation,
            4
        );
        assert_eq!(
            bases.require(EffectFamily::Pointer),
            Err(EffectBaseRefusal::Unavailable)
        );
        assert_eq!(
            bases.require(EffectFamily::Storage),
            Err(EffectBaseRefusal::Unavailable)
        );
        let framebuffer = bases
            .require_resource(EffectFamily::Framebuffer, FRAMEBUFFER_RESOURCE_CLASS)
            .unwrap();
        assert_eq!(framebuffer.pool_id.as_str(), "pool/frame");
        assert_eq!(framebuffer.capacity_units, 1);
        assert_eq!(
            bases.require_resource(EffectFamily::Keyboard, FRAMEBUFFER_RESOURCE_CLASS),
            Err(EffectBaseRefusal::Unavailable)
        );
    }

    #[test]
    fn one_input_controller_can_back_keyboard_and_pointer_without_duplicate_base_truth() {
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 0,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: 1,
            maximum_advertised_resources: 1,
        })
        .unwrap();
        let mut effects = Vec::new();
        for family in [EffectFamily::Keyboard, EffectFamily::Pointer] {
            register_effect(
                &mut registry,
                &mut effects,
                family,
                "base/xhci".into(),
                "provider/xhci".into(),
                7,
                crate::keyboard_offer::XHCI_BASE_IMPLEMENTATION,
                INPUT_CONTROLLER_FAMILY,
                Vec::new(),
            )
            .unwrap();
        }
        let bases = NativeProductBases { registry, effects };
        assert_eq!(bases.entries().len(), 1);
        assert_eq!(
            bases.require(EffectFamily::Keyboard).unwrap().base_id,
            bases.require(EffectFamily::Pointer).unwrap().base_id
        );
    }
}
