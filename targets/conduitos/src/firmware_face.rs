//! Preparation of the canonical Crèche Face on one observed firmware scanout.
//! Owns no second Body model, renderer, input provider or execution kernel.
use alloc::{format, vec::Vec};
use conduit_core::{BootId, HostBaseId, HostId, OfferGeneration};
use serde_json::json;

use crate::{
    display::PixelTarget,
    front_door::FrontDoor,
    identity::{self, BootIdentities},
    keyboard_text_plan,
    make::{IMPL_NATIVE_PRESENTER, MakeRecord},
    native_compositor::{MAX_COMPOSITOR_PIXELS, MAX_COMPOSITOR_SURFACES},
    native_face_mask::NativeFaceMask,
    native_workset,
    offer::{CAPABILITY_COUNT, HostOffer},
    product_bases::NativeProductBases,
    product_journey::ProductJourney,
};

pub const SIGN_PREFIX: &[u8] = b"CONDUIT_FIRMWARE_FACE ";
const MAX_EVIDENCE_BYTES: usize = 96 * 1024;

/// Root retains the display, lifecycle owner and Mask while the image lives.
/// Local input remains unavailable until an independently admitted provider exists.
pub struct FirmwareFace<D> {
    _display: D,
    _mask: NativeFaceMask,
    _door: FrontDoor,
    _journey: ProductJourney,
    _bases: NativeProductBases,
    basis: conduit_observatory::FramebufferBasis,
    evidence: Vec<u8>,
}

impl<D: PixelTarget> FirmwareFace<D> {
    pub fn prepare(
        mut display: D,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        make: &MakeRecord,
        issuer_key: [u8; 32],
    ) -> Result<Self, &'static str> {
        if !make.includes(IMPL_NATIVE_PRESENTER) {
            return Err("firmware-face-not-selected");
        }
        let format = display.format().validate().map_err(|e| e.as_str())?;
        let bytes = format.byte_len().map_err(|e| e.as_str())?;
        let pixels = u64::from(format.width) * u64::from(format.height);
        if pixels > MAX_COMPOSITOR_PIXELS as u64
            || make.face_surface_slots == 0
            || make.face_surface_slots as usize > MAX_COMPOSITOR_SURFACES
            || (bytes as u64)
                .checked_mul(2)
                .is_none_or(|required| required > make.face_surface_bytes)
        {
            return Err("firmware-face-surface-budget-unavailable");
        }
        let base = identity::derive_base(&identities.boot, "conduitos/display/firmware/0");
        let basis = conduit_observatory::FramebufferBasis {
            base_id: HostBaseId::from(identity::hex(&base)),
            width: format.width,
            height: format.height,
            pitch_bytes: format.pitch,
            bits_per_pixel: format.bits_per_pixel,
        };
        let bases = NativeProductBases::observe_framebuffer(&basis)
            .map_err(|_| "firmware-face-base-invalid")?;
        let provider = bases
            .framebuffer_provider(issuer_key)
            .map_err(|_| "firmware-face-provider-invalid")?;
        let host = HostId::from(identity::hex(&identities.host));
        let boot = BootId::from(identity::hex(&identities.boot));
        let generation = OfferGeneration(offer.generation);
        let mut journey =
            ProductJourney::new(host.clone(), boot.clone(), generation).map_err(|e| e.as_str())?;
        journey.admit_surface_provider(provider.clone());
        let plot = keyboard_text_plan::checked_plot_identity().map_err(|e| e.as_str())?;
        let mut door = FrontDoor::new(
            host.clone(),
            boot.clone(),
            generation,
            make.profile_id,
            make.build_id,
            make.image_binding,
            plot.source_document_id,
            plot.checked_plot_id,
            CAPABILITY_COUNT as u64,
            true,
        );
        door.observe_journey(journey.projection())
            .map_err(|e| e.as_str())?;
        // The shared BirthDraft and ordinary native workset review remain the
        // semantic owners; preparation only composes those existing entrances.
        let hex = identity::hex(&identities.boot);
        let uuid = format!(
            "{}-{}-{}-{}-{}",
            &hex[..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        );
        let refusals = native_workset::inventory().map(|plot| {
            native_workset::review(plot, identities, offer, make.build_id)
                .err()
                .map(|error| error.as_str().into())
        });
        door.open_creche_reviewed(uuid, refusals)
            .map_err(|e| e.as_str())?;
        let mut mask = NativeFaceMask::prepare_creche(
            host,
            boot,
            generation,
            make.build_id,
            basis.base_id.clone(),
            "conduitos/creche/face",
            &provider,
            conduit_face::CrecheComponentRecipe::ResponsiveColumns,
        )
        .map_err(|e| e.as_str())?;
        let birth_basis = mask.birth_basis("conduitos/creche/current");
        let face = door.creche_face(&birth_basis).map_err(|e| e.as_str())?;
        let composition = mask
            .present_read_only(face.clone(), 1, 1, &mut display)
            .map_err(|e| e.as_str())?;
        let show = mask.show().ok_or("firmware-face-show-absent")?;
        let rendered = mask
            .render_receipt()
            .ok_or("firmware-face-render-receipt-absent")?;
        if rendered.scanout_pixels_written == 0
            || rendered.face_id != face.identity.as_str()
            || rendered.display_base_id != basis.base_id
        {
            return Err("firmware-face-scanout-mismatch");
        }
        let evidence = serde_json::to_vec(&json!({
            "schema":"conduit.conduitos/firmware-face@1", "status":"shown",
            "checkpoint":"creche-read-only", "target":make.target,
            "profile_id":make.profile_id,"build_id":make.build_id,"image_id":make.image_binding,
            "face":face,"publication":mask.publication(),"mask_render":rendered,
            "show":{"show_id":show.show_id,"mask_plot":show.mask_plot,
                "mask_plan_id":show.planned_mask.plan.plan_id,
                "mask_active_play_id":show.show.active_play_id,"face_id":show.face_id,
                "face_revision":show.face_revision,"manifestation":show.show},
            "provider":provider.entry,"framebuffer":{"base_id":basis.base_id,
                "width":format.width,"height":format.height,"pitch":format.pitch,
                "bits_per_pixel":format.bits_per_pixel,"red_shift":format.red_shift,
                "green_shift":format.green_shift,"blue_shift":format.blue_shift},
            "surface_slots":make.face_surface_slots,"surface_bytes":make.face_surface_bytes,
            "offscreen_pixels_written":composition.display.pixels_written,
            "component_configuration":mask.component_configuration(),
            "local_input_available":false,"interactions_admitted":false,
            "physical_scanout_proved":false,"gpu_acceleration_proved":false
        }))
        .map_err(|_| "firmware-face-evidence-encoding")?;
        if evidence.len() > MAX_EVIDENCE_BYTES {
            return Err("firmware-face-evidence-bound");
        }
        Ok(Self {
            _display: display,
            _mask: mask,
            _door: door,
            _journey: journey,
            _bases: bases,
            basis,
            evidence,
        })
    }
    pub fn framebuffer_basis(&self) -> &conduit_observatory::FramebufferBasis {
        &self.basis
    }
    pub fn evidence(&self) -> &[u8] {
        &self.evidence
    }
}
