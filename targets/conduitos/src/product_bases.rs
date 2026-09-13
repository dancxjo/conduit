//! Narrow product-facing inspection and admission of external-effect Bases.
//!
//! This registry owns no devices and grants no bearer authority. It prevents
//! the product supervisor from treating co-resident machine mechanisms as one
//! ambient capability and records the currently explicit interim seams.

use alloc::{borrow::ToOwned, format, string::String};

use conduit_core::{BaseInstanceId, HostBaseId};

use crate::{arch::UsbDevice, identity, offer::HostOffer};

pub const EFFECT_BASE_COUNT: usize = 7;

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
pub enum EffectBaseState {
    Ready,
    InterimUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectBaseSeam {
    pub family: EffectFamily,
    pub base_id: Option<HostBaseId>,
    pub provider_instance_id: Option<BaseInstanceId>,
    pub provider_generation: Option<u64>,
    pub state: EffectBaseState,
    pub interim: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectBaseRefusal {
    Unavailable,
    InvalidProvider,
}

pub struct NativeProductBases {
    seams: [EffectBaseSeam; EFFECT_BASE_COUNT],
}

impl NativeProductBases {
    pub fn observe(
        offer: &HostOffer<'_>,
        framebuffer: &conduit_observatory::FramebufferBasis,
        usb_line: Option<&UsbDevice>,
    ) -> Result<Self, EffectBaseRefusal> {
        let keyboard = offer.keyboard.map(|value| {
            ready(
                EffectFamily::Keyboard,
                identity::hex(&value.realization.controller_id),
                identity::hex(&value.realization.endpoint_id),
                offer.generation,
            )
        });
        let pointer = offer.pointer.map(|value| {
            ready(
                EffectFamily::Pointer,
                identity::hex(&value.realization.controller_id),
                identity::hex(&value.realization.endpoint_id),
                offer.generation,
            )
        });
        let audio = offer.pc_speaker.map(|value| {
            let base = identity::hex(&value.realization.base_id);
            ready(
                EffectFamily::Audio,
                base.clone(),
                format!("{base}/provider/{}", offer.generation),
                offer.generation,
            )
        });
        let framebuffer_base = framebuffer.base_id.as_str().to_owned();
        let line = usb_line.map(|device| {
            ready(
                EffectFamily::Line,
                crate::usb_line_offer::USB_FTDI_BASE.into(),
                format!(
                    "conduitos/usb-line/{}/{}",
                    device.root_port, device.attachment_epoch
                ),
                u64::from(device.attachment_epoch),
            )
        });
        let seams = [
            keyboard
                .unwrap_or_else(|| unavailable(EffectFamily::Keyboard, "no current input Base")),
            pointer
                .unwrap_or_else(|| unavailable(EffectFamily::Pointer, "no current pointer Base")),
            audio.unwrap_or_else(|| unavailable(EffectFamily::Audio, "no current sound Base")),
            unavailable(
                EffectFamily::Storage,
                "native product storage Base not installed",
            ),
            line.unwrap_or_else(|| {
                unavailable(EffectFamily::Line, "no current transport Line Base")
            }),
            ready(
                EffectFamily::Framebuffer,
                framebuffer_base.clone(),
                format!("{framebuffer_base}/provider/1"),
                1,
            ),
            unavailable(
                EffectFamily::Network,
                "native product network Base not installed",
            ),
        ];
        let bases = Self { seams };
        for seam in &bases.seams {
            if seam.state == EffectBaseState::Ready
                && (seam
                    .base_id
                    .as_ref()
                    .is_none_or(|id| id.as_str().is_empty())
                    || seam
                        .provider_instance_id
                        .as_ref()
                        .is_none_or(|id| id.as_str().is_empty())
                    || seam
                        .provider_generation
                        .is_none_or(|generation| generation == 0))
            {
                return Err(EffectBaseRefusal::InvalidProvider);
            }
        }
        Ok(bases)
    }

    pub fn seams(&self) -> &[EffectBaseSeam] {
        &self.seams
    }

    pub fn require(&self, family: EffectFamily) -> Result<&EffectBaseSeam, EffectBaseRefusal> {
        self.seams
            .iter()
            .find(|seam| seam.family == family && seam.state == EffectBaseState::Ready)
            .ok_or(EffectBaseRefusal::Unavailable)
    }
}

fn ready(
    family: EffectFamily,
    base_id: String,
    provider_instance_id: String,
    provider_generation: u64,
) -> EffectBaseSeam {
    EffectBaseSeam {
        family,
        base_id: Some(HostBaseId::from(base_id)),
        provider_instance_id: Some(BaseInstanceId::from(provider_instance_id)),
        provider_generation: Some(provider_generation),
        state: EffectBaseState::Ready,
        interim: None,
    }
}

fn unavailable(family: EffectFamily, interim: &'static str) -> EffectBaseSeam {
    EffectBaseSeam {
        family,
        base_id: None,
        provider_instance_id: None,
        provider_generation: None,
        state: EffectBaseState::InterimUnavailable,
        interim: Some(interim),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_effect_family_is_explicit_and_authority_does_not_cross_seams() {
        let bases = NativeProductBases {
            seams: [
                ready(
                    EffectFamily::Keyboard,
                    "base/key".into(),
                    "provider/key".into(),
                    4,
                ),
                unavailable(EffectFamily::Pointer, "pointer absent"),
                unavailable(EffectFamily::Audio, "audio absent"),
                unavailable(EffectFamily::Storage, "storage interim"),
                unavailable(EffectFamily::Line, "line absent"),
                ready(
                    EffectFamily::Framebuffer,
                    "base/frame".into(),
                    "provider/frame".into(),
                    2,
                ),
                unavailable(EffectFamily::Network, "network interim"),
            ],
        };
        assert_eq!(bases.seams().len(), EFFECT_BASE_COUNT);
        assert_eq!(
            bases
                .require(EffectFamily::Framebuffer)
                .unwrap()
                .base_id
                .as_ref()
                .unwrap()
                .as_str(),
            "base/frame"
        );
        assert_eq!(
            bases
                .require(EffectFamily::Keyboard)
                .unwrap()
                .provider_generation,
            Some(4)
        );
        assert_eq!(
            bases.require(EffectFamily::Pointer),
            Err(EffectBaseRefusal::Unavailable)
        );
        assert!(
            bases
                .seams()
                .iter()
                .filter(|seam| seam.interim.is_some())
                .all(|seam| {
                    seam.state == EffectBaseState::InterimUnavailable
                        && seam.base_id.is_none()
                        && seam.provider_instance_id.is_none()
                })
        );
    }
}
