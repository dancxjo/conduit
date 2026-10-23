//! Explicit stopped-owner cancellation and restart with the real embedded guest.
//! No asynchronous cancellation, CPU fault, malformed reply or physical claim.
use super::*;
use crate::protected_compositor::{
    lifecycle::{
        DisplayObservation,
        sign::{AckRef, Cost, Identity},
    },
    session_owner::{OwnerError, SessionId},
};
use conduitos_compositor_core::{
    Handle, Rect,
    wire::{Request, Response},
};
#[path = "cancellation/record.rs"]
mod record;
#[path = "cancellation/zero_record.rs"]
mod zero_record;

fn unfinished_upload(owner: &SharedDisplayOwner, session: SessionId) -> Handle {
    let mut owner = owner.borrow_mut();
    let rect = Rect {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    };
    let handle = match owner
        .sessions
        .request(session, Request::Admit { rect, z: 0 })
    {
        Ok(Response::Surface(handle)) => handle,
        _ => refuse("cancel-upload-admit"),
    };
    if owner.sessions.request(
        session,
        Request::BeginUpload {
            surface: handle,
            revision: 1,
            rect,
        },
    ) != Ok(Response::Unit)
    {
        refuse("cancel-upload-begin");
    }
    let mut prepared = false;
    for _ in
        0..=crate::native_compositor::MAX_COMPOSITOR_PIXELS / conduitos_compositor_core::MAX_CHUNK
    {
        match owner.sessions.request(
            session,
            Request::PrepareUpload {
                pixels: conduitos_compositor_core::MAX_CHUNK as u32,
            },
        ) {
            Ok(Response::Prepared(true)) => {
                prepared = true;
                break;
            }
            Ok(Response::Prepared(false)) => {}
            _ => refuse("cancel-upload-prepare"),
        }
    }
    if !prepared || owner.sessions.upload_chunk(session, 0, &[0x00ab_cdef]) != Ok(Response::Unit) {
        refuse("cancel-upload-data");
    }
    // Intentionally leave the real upload uncommitted at cancellation.
    handle
}

pub(super) fn run() {
    let display = crate::boot::framebuffer_display().unwrap_or_else(|_| refuse("cancel-display"));
    let mut provider = crate::product_bases::fixture_surface_provider();
    let grant = Rc::new(
        DisplayGrant::new(
            HostId::from(HOST),
            BootId::from(BOOT),
            OfferGeneration(1),
            provider.entry.base_id.clone(),
            display.format(),
        )
        .unwrap_or_else(|_| refuse("cancel-grant")),
    );
    let old_owner = fresh_owner(&grant);
    let prepared = old_owner.borrow().lifecycle_observation();
    provider.display_owner = Some(old_owner.clone());
    let mut old = mask(&provider);
    let mut draft = draft();
    let basis = old.birth_basis("fixture/stopped-cancel/encounter");
    let face = draft.face(&basis).unwrap_or_else(|_| refuse("cancel-face"));
    let mut target = Target {
        display,
        fail: false,
        writes: 0,
        refused: 0,
    };
    old.present(face.clone(), 11, 11, &mut target)
        .unwrap_or_else(|_| refuse("cancel-present"));
    let held_show = old.show().unwrap_or_else(|| refuse("cancel-show")).clone();
    let held_input = interaction(&old, &face, &held_show, 11);
    let ack = old
        .lifecycle_copy_ack()
        .unwrap_or_else(|| refuse("cancel-ack"));
    let acknowledged = old_owner.borrow().lifecycle_observation();
    let session = old_owner
        .borrow()
        .sessions
        .active_session()
        .unwrap_or_else(|| refuse("cancel-session"));
    if ack.instance != prepared.service.instance
        || Some(ack.namespace) != old_owner.borrow().sessions.active_namespace()
        || target.writes == 0
    {
        refuse("cancel-ack-binding");
    }
    let surface = unfinished_upload(&old_owner, session);
    let before = old_owner.borrow().lifecycle_observation();
    let allocated = crate::allocation::BOOT_ARENA.live_bytes();
    let revision = draft.revision();
    // No guest entry or borrowed raster/chunk is running. Publication is fenced
    // before clearing; this is stopped-owner cancellation, not async preemption.
    let capacity_bytes = old_owner
        .borrow()
        .preparation_report()
        .staging_reserved_bytes;
    let zero_token = arch::arm_service_zero_witness(
        crate::service_memory::ServiceProfile::Compositor,
        before.service.instance,
    )
    .unwrap_or_else(|_| refuse("cancel-zero-arm"));
    if !old_owner
        .borrow_mut()
        .arm_staging_zero_witness(before.service.instance, zero_token.ordinal())
    {
        refuse("cancel-staging-zero-arm");
    }
    old_owner.borrow_mut().quarantine();
    let disposed = old_owner.borrow().lifecycle_observation();
    // Consume before fresh preparation could clear/reuse the old physical store.
    let private_zero =
        arch::take_service_zero_witness(zero_token).unwrap_or_else(|_| refuse("cancel-zero-take"));
    let staging_zero = old_owner
        .borrow_mut()
        .take_staging_zero_witness()
        .unwrap_or_else(|| refuse("cancel-staging-zero-take"));
    let zero = zero_record::validate(
        private_zero,
        staging_zero,
        zero_token,
        before,
        disposed,
        capacity_bytes,
    );
    if old_owner.borrow().sessions.active_session().is_some()
        || disposed.staging_live_bytes != 0
        || disposed.disposal_raw_ticks.is_none()
        || disposed.service.disposal.is_none()
        || allocated.saturating_sub(crate::allocation::BOOT_ARENA.live_bytes())
            < prepared.staging_prepared_bytes
        || disposed.current_cost.teardown_zeroed_bytes != disposed.current_cost.reserved_bytes
    {
        refuse("cancel-disposal");
    }
    // Reserve the new arena before allocating new small semantic state. The
    // grant is retained custody, never reconstructed or renewed after cancel.
    let attempts = crate::allocation::BOOT_ARENA.attempts();
    let retained = grant.clone();
    if !Rc::ptr_eq(&retained, &grant) || crate::allocation::BOOT_ARENA.attempts() != attempts {
        refuse("cancel-grant-custody");
    }
    let fresh_owner = fresh_owner(&retained);
    let restarted = fresh_owner.borrow().lifecycle_observation();
    if restarted.service.instance <= prepared.service.instance
        || restarted.guest_image_sha256 != prepared.guest_image_sha256
        || restarted.current_cost.teardown_zeroed_bytes != 0
    {
        refuse("cancel-restart-identity");
    }
    let fresh_cost = fresh_owner.borrow().service_cost();
    if old.submit(held_input.clone())
        != Err(NativeFaceMaskError::Compositor(
            NativeCompositorError::ServiceLost,
        ))
        || old.route_keyboard().is_ok()
        || draft.revision() != revision
    {
        refuse("cancel-old-semantic-authority");
    }
    {
        let mut owner = old_owner.borrow_mut();
        if owner.sessions.request(session, Request::CommitUpload) != Err(OwnerError::Terminated)
            || owner.sessions.upload_chunk(session, 0, &[0]) != Err(OwnerError::Terminated)
            || owner.sessions.request(session, Request::Remove(surface))
                != Err(OwnerError::Terminated)
        {
            refuse("cancel-old-protocol-authority");
        }
        // Idempotence must not replace the first actual disposal sample.
        owner.quarantine();
        if owner.lifecycle_observation() != disposed {
            refuse("cancel-disposal-idempotence");
        }
    }
    drop(old);
    if fresh_owner.borrow().service_cost() != fresh_cost {
        refuse("cancel-stale-drop");
    }
    provider.display_owner = Some(fresh_owner.clone());
    let mut fresh = mask(&provider);
    if fresh.show().is_some() || fresh.route_keyboard().is_ok() {
        refuse("cancel-no-show-before-present");
    }
    let fresh_basis = fresh.birth_basis("fixture/stopped-cancel/encounter");
    let fresh_face = draft
        .face(&fresh_basis)
        .unwrap_or_else(|_| refuse("cancel-fresh-face"));
    if fresh_face != face {
        refuse("cancel-source-face");
    }
    fresh
        .present(fresh_face.clone(), 12, 12, &mut target)
        .unwrap_or_else(|_| refuse("cancel-fresh-present"));
    let show = fresh
        .show()
        .unwrap_or_else(|| refuse("cancel-fresh-show"))
        .clone();
    let fresh_ack = fresh
        .lifecycle_copy_ack()
        .unwrap_or_else(|| refuse("cancel-fresh-ack"));
    let reacknowledged = fresh_owner.borrow().lifecycle_observation();
    if fresh_ack.instance != restarted.service.instance
        || show.show_id == held_show.show_id
        || show.show.active_play_id == held_show.show.active_play_id
        || held_input.validate_against(&fresh_face, &show).is_ok()
        || fresh.route_keyboard() != Ok(true)
    {
        refuse("cancel-fresh-authority");
    }
    let input = interaction(&fresh, &fresh_face, &show, 12);
    let correlation = fresh
        .submit(input.clone())
        .unwrap_or_else(|_| refuse("cancel-fresh-input"));
    if correlation.show_id != show.show_id || correlation.interaction != input {
        refuse("cancel-correlation");
    }
    draft
        .apply_face_interaction(&fresh_basis, &show, &input)
        .unwrap_or_else(|_| refuse("cancel-semantic-apply"));
    if draft.revision() <= revision {
        refuse("cancel-semantic-progress");
    }
    // Emit all snapshots after the complete experiment, never inside intervals.
    record::emit(
        &grant,
        [
            prepared,
            acknowledged,
            before,
            disposed,
            restarted,
            reacknowledged,
        ],
        ack,
        fresh_ack,
        zero,
    );
    fresh_owner.borrow_mut().quarantine();
}
