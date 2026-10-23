//! Three finite paired render/copy workloads, not a pure-trap or physical benchmark.
use super::*;
use crate::{
    display::DisplayReceipt,
    protected_compositor::lifecycle::{self, DisplayObservation, sign::AckRef},
};
use conduit_face::{FaceContentId, GraphicsScene};
#[path = "retained/record.rs"]
mod record;

struct Arm {
    before: DisplayObservation,
    after: DisplayObservation,
    ack: AckRef,
    elapsed_raw_ticks: Option<u64>,
    writes: u32,
    raster: DisplayReceipt,
}
struct Trial {
    prepared: DisplayObservation,
    face: FaceContentId,
    scene: GraphicsScene,
    cold: Arm,
    retained: Arm,
}
fn present(
    mask: &mut NativeFaceMask,
    owner: &SharedDisplayOwner,
    face: &Face,
    target: &mut Target,
    sequence: u64,
) -> (Arm, FaceContentId) {
    let before = owner.borrow().lifecycle_observation();
    let writes = target.writes;
    let input_face = face.clone();
    let start = lifecycle::start();
    let receipt = mask
        .present(input_face, sequence, sequence, target)
        .unwrap_or_else(|_| refuse("matched-present"));
    let elapsed_raw_ticks = lifecycle::elapsed(start, lifecycle::start());
    let after = owner.borrow().lifecycle_observation();
    let ack = mask
        .lifecycle_copy_ack()
        .unwrap_or_else(|| refuse("matched-ack"));
    if ack.instance != before.service.instance
        || ack.instance != after.service.instance
        || Some(ack.namespace) != owner.borrow().sessions.active_namespace()
        || after.activations.attempts.count != before.activations.attempts.count + 1
        || after.service.installed != before.service.installed
        || after.current_cost.setup_copied_bytes != before.current_cost.setup_copied_bytes
        || after.current_cost.setup_ticks != before.current_cost.setup_ticks
        || after.current_cost.reserved_bytes != before.current_cost.reserved_bytes
        || after.staging_live_bytes != before.staging_live_bytes
        || after.service.disposal.is_some()
        || after.disposal_raw_ticks.is_some()
        || elapsed_raw_ticks.is_none()
    {
        refuse("matched-retained-resource-custody");
    }
    (
        Arm {
            before,
            after,
            ack,
            elapsed_raw_ticks,
            writes: target
                .writes
                .checked_sub(writes)
                .unwrap_or_else(|| refuse("matched-write-counter")),
            raster: receipt.display,
        },
        receipt.face_id,
    )
}
fn trial(
    provider: &mut NativeSurfaceProvider,
    grant: &SharedDisplayGrant,
    target: &mut Target,
    trial: u64,
) -> Trial {
    let owner = fresh_owner(grant);
    let prepared = owner.borrow().lifecycle_observation();
    provider.display_owner = Some(owner.clone());
    let mut mask = mask(provider);
    let draft = draft();
    let basis = mask.birth_basis("fixture/retained-pair/encounter");
    let face = draft
        .face(&basis)
        .unwrap_or_else(|_| refuse("matched-face"));
    let (cold, face_id) = present(&mut mask, &owner, &face, target, 20 + trial * 2);
    let scene = mask
        .scene()
        .unwrap_or_else(|| refuse("matched-cold-scene"))
        .frame()
        .unwrap_or_else(|_| refuse("matched-cold-frame"))
        .scene;
    let held_show = mask
        .show()
        .unwrap_or_else(|| refuse("matched-cold-show"))
        .clone();
    let held_input = interaction(&mask, &face, &held_show, 20 + trial * 2);
    // Withdraw outside both timed presentation intervals, retaining the exact
    // Mask/owner/grant. No service disposal, new admission or retry occurs here.
    mask.suspend()
        .unwrap_or_else(|_| refuse("matched-withdraw"));
    if mask.show().is_some()
        || mask.route_keyboard().is_ok()
        || owner.borrow().sessions.is_terminated()
    {
        refuse("matched-withdraw-authority");
    }
    let (retained, warm_face) = present(&mut mask, &owner, &face, target, 21 + trial * 2);
    let warm_scene = mask
        .scene()
        .unwrap_or_else(|| refuse("matched-retained-scene"))
        .frame()
        .unwrap_or_else(|_| refuse("matched-retained-frame"))
        .scene;
    let fresh_show = mask
        .show()
        .unwrap_or_else(|| refuse("matched-retained-show"));
    if face_id != warm_face
        || scene != warm_scene
        || cold.raster != retained.raster
        || cold.writes == 0
        || cold.writes != retained.writes
        || cold.ack.instance != retained.ack.instance
        || cold.ack.namespace >= retained.ack.namespace
        || fresh_show.show_id == held_show.show_id
        || fresh_show.show.active_play_id == held_show.show.active_play_id
        || held_input.validate_against(&face, fresh_show).is_ok()
        || cold.after.service.installed != retained.after.service.installed
        || cold.after.current_cost.setup_ticks != retained.after.current_cost.setup_ticks
        || cold.after.current_cost.setup_copied_bytes
            != retained.after.current_cost.setup_copied_bytes
        || cold.after.current_cost.reserved_bytes != retained.after.current_cost.reserved_bytes
    {
        use core::fmt::Write;
        let mut line = crate::sign_format::FixedText::new();
        let _ = writeln!(
            line,
            "CONDUIT_COMPOSITOR_RETAINED_PAIR_MISMATCH cold_commands={} retained_commands={} cold_raster_pixels={} retained_raster_pixels={} cold_writes={} retained_writes={} same_scene={} same_face={} cold_instance={} retained_instance={} cold_namespace={} retained_namespace={}",
            cold.raster.commands,
            retained.raster.commands,
            cold.raster.pixels_written,
            retained.raster.pixels_written,
            cold.writes,
            retained.writes,
            scene == warm_scene,
            face_id == warm_face,
            cold.ack.instance,
            retained.ack.instance,
            cold.ack.namespace,
            retained.ack.namespace
        );
        arch::early_write(line.as_bytes());
        refuse("matched-render-copy-workload");
    }
    // Deliberate terminal release at the end of the pair, never hidden reuse
    // across cold trials. Whole-zero observers are not armed in this experiment.
    owner.borrow_mut().quarantine();
    drop(mask);
    provider.display_owner = None;
    drop(owner);
    Trial {
        prepared,
        face: face_id,
        scene,
        cold,
        retained,
    }
}
pub(super) fn run() {
    let display = crate::boot::framebuffer_display().unwrap_or_else(|_| refuse("matched-display"));
    let mut provider = crate::product_bases::fixture_surface_provider();
    let grant = Rc::new(
        DisplayGrant::new(
            HostId::from(HOST),
            BootId::from(BOOT),
            OfferGeneration(1),
            provider.entry.base_id.clone(),
            display.format(),
        )
        .unwrap_or_else(|_| refuse("matched-grant")),
    );
    let mut target = Target {
        display,
        fail: false,
        writes: 0,
        refused: 0,
    };
    let trials = core::array::from_fn::<_, 3, _>(|index| {
        trial(&mut provider, &grant, &mut target, index as u64)
    });
    for pair in trials.windows(2) {
        if pair[0].prepared.service.instance >= pair[1].prepared.service.instance
            || pair[0].face != pair[1].face
            || pair[0].scene != pair[1].scene
            || pair[0].cold.raster != pair[1].cold.raster
            || pair[0].cold.writes != pair[1].cold.writes
            || pair[0].prepared.guest_image_sha256 != pair[1].prepared.guest_image_sha256
        {
            refuse("matched-cross-trial-workload");
        }
    }
    record::emit(&grant, &trials);
}
