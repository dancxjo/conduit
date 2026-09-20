//! Narrow product-facing adapter over the canonical registry of external-effect Bases.

use alloc::{borrow::ToOwned, format, vec::Vec};

use conduit_core::{
    BaseEnforcementClass, BaseImplementationId, BaseInstanceId, BaseLifecycle, BaseProviderEntry,
    BaseRegistry, BaseRegistryLimits, HostBaseId, HostBaseKindId,
};

use crate::{arch::UsbDevice, identity, offer::HostOffer};

const MAXIMUM_EFFECT_BASES: u16 = 5;

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
            maximum_resources_per_base: 0,
            maximum_advertised_capabilities: 1,
            maximum_advertised_resources: 1,
        })
        .map_err(|_| EffectBaseRefusal::InvalidProvider)?;
        if let Some(value) = offer.keyboard {
            register(
                &mut registry,
                identity::hex(&value.realization.controller_id),
                identity::hex(&value.realization.endpoint_id),
                offer.generation,
                value.realization.mechanism.implementation(),
                family_kind(EffectFamily::Keyboard),
            )?;
        }
        if let Some(value) = offer.pointer {
            register(
                &mut registry,
                identity::hex(&value.realization.controller_id),
                identity::hex(&value.realization.endpoint_id),
                offer.generation,
                value.realization.mechanism.implementation(),
                family_kind(EffectFamily::Pointer),
            )?;
        }
        if let Some(value) = offer.pc_speaker {
            let base = identity::hex(&value.realization.base_id);
            register(
                &mut registry,
                base.clone(),
                format!("{base}/provider/{}", offer.generation),
                offer.generation,
                crate::pc_speaker_offer::PC_SPEAKER_IMPLEMENTATION,
                family_kind(EffectFamily::Audio),
            )?;
        }
        let framebuffer_base = framebuffer.base_id.as_str().to_owned();
        if let Some(device) = usb_line {
            register(
                &mut registry,
                crate::usb_line_offer::USB_FTDI_BASE.into(),
                format!(
                    "conduitos/usb-line/{}/{}",
                    device.root_port, device.attachment_epoch
                ),
                u64::from(device.attachment_epoch),
                crate::usb_line_offer::USB_FTDI_BASE,
                family_kind(EffectFamily::Line),
            )?;
        }
        register(
            &mut registry,
            framebuffer_base.clone(),
            format!("{framebuffer_base}/provider/1"),
            1,
            "conduitos/framebuffer@1",
            family_kind(EffectFamily::Framebuffer),
        )?;
        Ok(Self { registry })
    }

    pub fn entries(&self) -> &[BaseProviderEntry] {
        self.registry.entries()
    }

    pub fn require(&self, family: EffectFamily) -> Result<&BaseProviderEntry, EffectBaseRefusal> {
        let kind = family_kind(family);
        self.registry
            .entries()
            .iter()
            .find(|entry| entry.mechanism_family.as_str() == kind)
            .ok_or(EffectBaseRefusal::Unavailable)
    }
}

fn register(
    registry: &mut BaseRegistry,
    base_id: alloc::string::String,
    provider_instance_id: alloc::string::String,
    provider_generation: u64,
    implementation_id: &'static str,
    mechanism_family: &'static str,
) -> Result<(), EffectBaseRefusal> {
    registry
        .register(BaseProviderEntry {
            base_id: HostBaseId::from(base_id),
            provider_instance_id: BaseInstanceId::from(provider_instance_id),
            provider_generation,
            implementation_id: BaseImplementationId::from(implementation_id),
            mechanism_family: HostBaseKindId::from(mechanism_family),
            enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
            lifecycle: BaseLifecycle::Ready,
            capabilities: Vec::new(),
            resources: Vec::new(),
        })
        .map_err(|_| EffectBaseRefusal::InvalidProvider)
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
    fn unavailable_families_are_absent_and_authority_does_not_cross_entries() {
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 2,
            maximum_capabilities_per_base: 0,
            maximum_resources_per_base: 0,
            maximum_advertised_capabilities: 1,
            maximum_advertised_resources: 1,
        })
        .unwrap();
        register(
            &mut registry,
            "base/key".into(),
            "provider/key".into(),
            4,
            "conduitos/key@1",
            family_kind(EffectFamily::Keyboard),
        )
        .unwrap();
        register(
            &mut registry,
            "base/frame".into(),
            "provider/frame".into(),
            2,
            "conduitos/frame@1",
            family_kind(EffectFamily::Framebuffer),
        )
        .unwrap();
        let bases = NativeProductBases { registry };
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
    }
}
