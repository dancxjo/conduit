//! First noninteractive Crèche frame through the ordinary producer and Mask.
//! Machine code supplies the actual discovered display and private issuer key.
use crate::{
    display::PixelTarget,
    identity::{self, BootIdentities},
    make::{FACILITY_NATIVE_COMPOSITOR, IMPL_NATIVE_PRESENTER, MakeRecord},
    native_compositor::CompositionReceipt,
    native_face_mask::NativeFaceMask,
    offer_make::ImageBoundHostOffer,
    product_bases::NativeProductBases,
};
use conduit_birth_plot::{BirthDraft, BirthPlotChoice};
use conduit_core::{BootId, HostBaseId, HostId, OfferGeneration};
use conduit_face::{Face, MaskShow};
use conduit_observatory::FramebufferBasis;
const ENCOUNTER: &str = "conduitos/arrival/first-frame";

/// Retain the real producer, checked Mask and available Show for the frame's life.
pub struct NativeFirstFrame {
    mask: NativeFaceMask,
    _draft: BirthDraft,
    pub framebuffer: FramebufferBasis,
    pub composition: CompositionReceipt,
}

impl NativeFirstFrame {
    pub fn prepare(
        identities: &BootIdentities,
        offer: &ImageBoundHostOffer<'_>,
        make: &MakeRecord,
        display: &mut impl PixelTarget,
        mut issuer_key: [u8; 32],
    ) -> Result<Self, &'static str> {
        make.validate(offer.runtime_arena_bytes)
            .map_err(|error| error.as_str())?;
        offer.validate().map_err(|error| error.as_str())?;
        if offer.host_id != identities.host
            || offer.boot_id != identities.boot
            || offer.profile_id != make.profile_id
            || offer.image_binding != make.image_binding
            || offer
                .capabilities
                .iter()
                .any(|back| back.artifact_build != make.build_id)
            || !make.includes(IMPL_NATIVE_PRESENTER)
            || !make.includes_facility(FACILITY_NATIVE_COMPOSITOR)
        {
            return Err("first-frame-image-offer-mismatch");
        }
        let format = display
            .format()
            .validate()
            .map_err(|error| error.as_str())?;
        let bytes = format.byte_len().map_err(|error| error.as_str())?;
        if u64::try_from(bytes).map_err(|_| "first-frame-display-bound")? > make.face_surface_bytes
            || make.face_surface_slots == 0
        {
            return Err("first-frame-display-bound");
        }
        let framebuffer = FramebufferBasis {
            base_id: HostBaseId::from(identity::hex(&identity::derive_base(
                &identities.boot,
                "conduitos/display/limine/0",
            ))),
            width: format.width,
            height: format.height,
            pitch_bytes: format.pitch,
            bits_per_pixel: format.bits_per_pixel,
        };
        let bases = NativeProductBases::observe_framebuffer(&framebuffer)
            .map_err(|_| "first-frame-display-provider-invalid")?;
        let provider = bases
            .framebuffer_provider(issuer_key)
            .map_err(|_| "first-frame-display-authority-invalid")?;
        issuer_key.fill(0);
        let mut mask = NativeFaceMask::prepare_creche(
            HostId::from(identity::hex(&identities.host)),
            BootId::from(identity::hex(&identities.boot)),
            OfferGeneration(offer.generation),
            make.build_id,
            framebuffer.base_id.clone(),
            "conduitos/first-frame/surface",
            &provider,
            conduit_face::CrecheComponentRecipe::ResponsiveColumns,
        )
        .map_err(|error| error.as_str())?;
        // This is an actual zero-Body Birth encounter, not an inventory Face
        // relabelled with producer identities. No lifecycle or input is admitted.
        let hex = identity::hex(&identities.boot);
        let uuid = alloc::format!(
            "{}-{}-{}-{}-{}",
            &hex[..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        );
        let choices = crate::native_workset::candidate_inventory()
            .into_iter()
            .map(|plot| {
                Ok(BirthPlotChoice {
                    title: plot.title().into(),
                    search_text: plot.title().into(),
                    plot: crate::native_workset::resident(plot).map_err(|error| error.as_str())?,
                    refusal: Some("Body lifecycle is not admitted on this Host.".into()),
                    selected: false,
                })
            })
            .collect::<Result<alloc::vec::Vec<_>, &'static str>>()?;
        let draft = BirthDraft::new(uuid, choices).map_err(|_| "first-frame-draft-invalid")?;
        let face = draft
            .face(&mask.birth_basis(ENCOUNTER))
            .map_err(|_| "first-frame-draft-face-invalid")?;
        mask.present_read_only(face, 1, 1, display)
            .map_err(|error| error.as_str())?;
        // Local Mask reading keys need a real input reader too. This Host has
        // none; preserve the owner's Face and qualify the existing Mask chrome.
        let composition = mask
            .show_local_notice("Read only: no local input", display)
            .map_err(|error| error.as_str())?;
        Ok(Self {
            mask,
            _draft: draft,
            framebuffer,
            composition,
        })
    }

    pub fn show(&self) -> Option<&MaskShow> {
        self.mask.show()
    }

    pub fn face(&self) -> Option<&Face> {
        self.mask.scene().map(|scene| scene.face())
    }

    pub fn publication(&self) -> Option<&crate::native_face_snapshot::FaceSnapshotReceipt> {
        self.mask.publication()
    }
}

#[cfg(test)]
mod tests;
