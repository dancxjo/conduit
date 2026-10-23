//! Actual Mask/Show boundary after Root copy failure, using the product renderer.
//! Actual emulator framebuffer copy with an intentionally failing wrapper.
#[path = "compositor_semantic_recovery/cancellation.rs"]
mod cancellation;
#[path = "compositor_semantic_recovery/retained.rs"]
mod retained;

pub(super) fn run_matched_pairs() {
    retained::run();
}
#[path = "compositor_semantic_recovery/pressure.rs"]
mod pressure;
#[path = "compositor_semantic_recovery/stale.rs"]
mod stale;

pub(super) fn run_lifecycle_supplement() {
    cancellation::run();
}

use super::refuse;
use crate::{
    arch,
    display::{DisplayError, DisplayFormat, PixelTarget, RawDisplay},
    native_compositor::NativeCompositorError,
    native_face_mask::{NativeFaceMask, NativeFaceMaskError},
    native_face_scene::FaceControl,
    product_bases::NativeSurfaceProvider,
    protected_compositor::{
        display_grant::{DisplayGrant, SharedDisplayGrant},
        display_owner::{DisplayOwner, SharedDisplayOwner},
    },
};
use alloc::{rc::Rc, vec};
use conduit_birth_plot::{BirthDraft, BirthPlotChoice};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_face::{Face, FaceInteraction, ManifestationLifecycle, MaskShow};

const HOST: &str = "fixture/semantic-copy-recovery/host";
const BOOT: &str = "fixture/semantic-copy-recovery/boot";
const SURFACE: &str = "fixture/semantic-copy-recovery/surface";
struct Target {
    display: RawDisplay,
    fail: bool,
    writes: u32,
    refused: u32,
}
impl PixelTarget for Target {
    fn format(&self) -> DisplayFormat {
        self.display.format()
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        if self.fail {
            self.refused += 1;
            return Err(DisplayError::Lost);
        }
        self.display.write_pixel(x, y, pixel)?;
        self.writes = self
            .writes
            .checked_add(1)
            .ok_or(DisplayError::InvalidExtent)?;
        Ok(())
    }
}
fn draft() -> BirthDraft {
    // Same canonical constructors and admitted resident choices as the product
    // NativeFaceMask tests; no synthetic parallel Face or interaction model.
    let choices = crate::native_workset::profile()
        .installed()
        .iter()
        .map(|plot| BirthPlotChoice {
            title: plot.title().into(),
            search_text: plot.title().into(),
            plot: crate::native_workset::resident(*plot)
                .unwrap_or_else(|_| refuse("semantic-recovery-resident")),
            refusal: None,
            selected: true,
        })
        .collect();
    BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices)
        .unwrap_or_else(|_| refuse("semantic-recovery-draft"))
}
fn mask(provider: &NativeSurfaceProvider) -> NativeFaceMask {
    NativeFaceMask::prepare(
        HostId::from(HOST),
        BootId::from(BOOT),
        OfferGeneration(1),
        "fixture/semantic-copy-recovery",
        provider.entry.base_id.clone(),
        SURFACE,
        provider,
    )
    .unwrap_or_else(|_| refuse("semantic-recovery-mask"))
}
fn interaction(
    mask: &NativeFaceMask,
    face: &Face,
    show: &MaskShow,
    sequence: u64,
) -> FaceInteraction {
    let action = face
        .actions
        .iter()
        .position(|action| action.identity == "creche.suggest")
        .unwrap_or_else(|| refuse("semantic-recovery-action"));
    mask.scene()
        .unwrap_or_else(|| refuse("semantic-recovery-scene"))
        .interaction(
            &face.identity,
            face.revision,
            FaceControl {
                action,
                argument: None,
            },
            show,
            vec![],
            sequence,
        )
        .unwrap_or_else(|_| refuse("semantic-recovery-interaction"))
}
fn fresh_owner(grant: &SharedDisplayGrant) -> SharedDisplayOwner {
    DisplayOwner::prepare_shared(grant.clone())
        .unwrap_or_else(|_| refuse("semantic-recovery-owner"))
}
pub(super) fn run() {
    let display = crate::boot::framebuffer_display()
        .unwrap_or_else(|_| refuse("semantic-recovery-framebuffer"));
    let format = display.format();
    let mut provider = crate::product_bases::fixture_surface_provider();
    let grant: SharedDisplayGrant = Rc::new(
        DisplayGrant::new(
            HostId::from(HOST),
            BootId::from(BOOT),
            OfferGeneration(1),
            provider.entry.base_id.clone(),
            format,
        )
        .unwrap_or_else(|_| refuse("semantic-recovery-grant")),
    );
    let old_owner = fresh_owner(&grant);
    let old_instance = old_owner.borrow().sessions.service_instance();
    provider.display_owner = Some(old_owner.clone());
    let mut old = mask(&provider);
    let mut draft = draft();
    let basis = old.birth_basis("fixture/semantic-copy-recovery/encounter");
    let face = draft
        .face(&basis)
        .unwrap_or_else(|_| refuse("semantic-recovery-face"));
    let mut target = Target {
        display,
        fail: false,
        writes: 0,
        refused: 0,
    };
    if old.show().is_some() {
        refuse("pressure-diagnostic-show");
    }
    let scene_pressure = pressure::scene(&old_owner);
    if old.show().is_some() {
        refuse("pressure-diagnostic-show");
    }
    old.present(face.clone(), 1, 1, &mut target)
        .unwrap_or_else(|_| refuse("semantic-recovery-initial-present"));
    let held_show = old
        .show()
        .unwrap_or_else(|| refuse("semantic-recovery-initial-show"))
        .clone();
    if held_show.show.lifecycle != ManifestationLifecycle::Available
        || target.writes == 0
        || old.route_keyboard() != Ok(true)
    {
        refuse("semantic-recovery-initial-ack");
    }
    let surface_pressure =
        pressure::surfaces(&old_owner, &mut old, &face, &held_show, &draft, &mut target);
    // All held values, including the consumed rejected copy, exist before the
    // staging arena is released. Recovery clones only the pre-admitted Rc grant.
    let held_input = interaction(&old, &face, &held_show, 1);
    let rejected_input = held_input.clone();
    let semantic_revision = draft.revision();
    target.fail = true;
    if old.show_local_notice("Fixture copy failure", &mut target)
        != Err(NativeFaceMaskError::Compositor(
            NativeCompositorError::Display(DisplayError::Lost),
        ))
        || target.refused == 0
        || old.show().is_some()
        || old.route_keyboard().is_ok()
        || old.submit(rejected_input)
            != Err(NativeFaceMaskError::Compositor(
                NativeCompositorError::ServiceLost,
            ))
        || draft.revision() != semantic_revision
    {
        refuse("semantic-recovery-old-refusal");
    }
    let replacement = fresh_owner(&grant); // reserve before new small admissions
    if replacement.borrow().sessions.service_instance() <= old_instance {
        refuse("semantic-recovery-instance");
    }
    // Release old semantic scheduling state, while retaining the inert owner,
    // held Show and typed input. No old session is revived by fresh preparation.
    drop(old);
    provider.display_owner = Some(replacement.clone());
    let mut fresh = mask(&provider);
    if fresh.show().is_some() || fresh.route_keyboard().is_ok() {
        refuse("semantic-recovery-input-before-present");
    }
    let fresh_basis = fresh.birth_basis("fixture/semantic-copy-recovery/encounter");
    let fresh_face = draft
        .face(&fresh_basis)
        .unwrap_or_else(|_| refuse("semantic-recovery-fresh-face"));
    if fresh_face != face {
        refuse("semantic-recovery-same-source-face");
    }
    target.fail = false;
    let before = target.writes;
    // present is the public atomic boundary: it returns only after actual
    // compose/copy, borrowed scanout acknowledgement, and complete_render.
    fresh
        .present(fresh_face.clone(), 2, 2, &mut target)
        .unwrap_or_else(|_| refuse("semantic-recovery-fresh-present"));
    let show = fresh
        .show()
        .unwrap_or_else(|| refuse("semantic-recovery-fresh-show"))
        .clone();
    if show.show.lifecycle != ManifestationLifecycle::Available
        || show.show_id == held_show.show_id
        || show.show.active_play_id == held_show.show.active_play_id
        || show.show.play_sequence != 2
        || target.writes <= before
        || fresh.route_keyboard() != Ok(true)
        || held_input.validate_against(&fresh_face, &show).is_ok()
        || draft.revision() != semantic_revision
    {
        refuse("semantic-recovery-fresh-ack");
    }
    let input = interaction(&fresh, &fresh_face, &show, 2);
    stale::capture_refusal(
        &mut fresh,
        &fresh_face,
        &show,
        &held_input,
        &draft,
        &mut target,
    );
    let correlation = fresh
        .submit(input.clone())
        .unwrap_or_else(|_| refuse("semantic-recovery-submit"));
    if correlation.show_id != show.show_id
        || correlation.interaction != input
        || correlation.show_id == held_show.show_id
        || fresh.show().is_some()
    {
        refuse("semantic-recovery-correlation");
    }
    draft
        .apply_face_interaction(&fresh_basis, &show, &input)
        .unwrap_or_else(|_| refuse("semantic-recovery-apply"));
    if draft.revision() <= semantic_revision {
        refuse("semantic-recovery-semantic-progress");
    }
    // Still no old authority after the independent fresh semantic interaction.
    if old_owner.borrow().sessions.active_session().is_some() {
        refuse("semantic-recovery-old-session");
    }
    pressure::emit(scene_pressure, surface_pressure);
    arch::early_write(b"CONDUIT_COMPOSITOR_SEMANTIC_RECOVERY_PROOF {\"schema\":\"conduit.conduitos/compositor-semantic-copy-recovery-fixture@1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"fixture\":true,\"production_mask_proxy\":true,\"initial_show_acknowledged\":true,\"root_copy_failure\":true,\"old_submit_refused\":true,\"stale_input_preserves_semantic_revision\":true,\"fresh_service_instance\":true,\"no_show_or_keyboard_before_present\":true,\"atomic_present_acknowledgement\":true,\"fresh_show_and_play\":true,\"held_input_refused_by_fresh_show\":true,\"fresh_interaction_correlated\":true,\"fresh_interaction_advances_semantics\":true,\"physical_scanout\":false,\"human_observation\":false,\"guest_cpu_fault_recovery\":false,\"malformed_guest_reply_recovery\":false}\n");
}
