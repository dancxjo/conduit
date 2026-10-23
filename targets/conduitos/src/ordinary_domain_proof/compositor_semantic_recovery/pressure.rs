//! Explicit Root fixture pressure; neither diagnostic pixels nor extra surfaces
//! acquire a semantic Show or an eight-surface reservation in the Mask Plan.
use super::*;
use crate::{protected_compositor::session_owner::OwnerError, sign_format::FixedText};
use conduit_face::{GraphicsCommand, GraphicsError, GraphicsPaintRole, GraphicsScene, LayoutRect};
use conduitos_compositor_core::{
    Handle, Rect,
    wire::{Request, Response},
};
use core::fmt::Write;

pub(super) struct SceneReport {
    commands: usize,
    text_bytes: usize,
    raster_commands: u8,
    raster_pixels: u32,
}
pub(super) struct SurfaceReport {
    admitted: usize,
    uploaded: usize,
    removed: usize,
    entries_before: u64,
    entries_after: u64,
    recompose_pixels: u32,
}

/// Runs before the first semantic presentation, in the already reserved staging
/// lease. This is the production Root typography rasterizer, not guest scanout.
pub(super) fn scene(owner: &SharedDisplayOwner) -> SceneReport {
    let bounds = LayoutRect {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
    };
    let bytes = [b'x'; 192];
    let text = core::str::from_utf8(&bytes).unwrap_or_else(|_| refuse("pressure-text"));
    let command = GraphicsCommand::text(bounds, bounds, GraphicsPaintRole::Foreground, text)
        .unwrap_or_else(|_| refuse("pressure-command"));
    let mut scene = GraphicsScene::empty();
    for _ in 0..48 {
        scene
            .push(command)
            .unwrap_or_else(|_| refuse("pressure-scene-admit"));
    }
    let complete = scene;
    if scene.push(command) != Err(GraphicsError::TooManyCommands) || scene != complete {
        refuse("pressure-command49-atomic");
    }
    let overlong = [b'x'; 193];
    if GraphicsCommand::text(
        bounds,
        bounds,
        GraphicsPaintRole::Foreground,
        core::str::from_utf8(&overlong).unwrap_or_else(|_| refuse("pressure-overlong-text")),
    ) != Err(GraphicsError::PayloadTooLong)
        || scene != complete
    {
        refuse("pressure-payload193-atomic");
    }
    let raster = owner
        .borrow_mut()
        .with_raster(640, 480, |_, lease| {
            crate::display::typography::render_scene(lease, &scene, |_, command| {
                command.text_role().into()
            })
        })
        .unwrap_or_else(|_| refuse("pressure-raster-lease"))
        .unwrap_or_else(|_| refuse("pressure-raster"));
    if raster.commands != 48 || raster.pixels_written == 0 || scene.commands().len() != 48 {
        refuse("pressure-raster-completion");
    }
    SceneReport {
        commands: scene.commands().len(),
        text_bytes: bytes.len(),
        raster_commands: raster.commands,
        raster_pixels: raster.pixels_written,
    }
}

/// Fill the actual Mask's active service namespace; do not activate another
/// namespace or invent planned surface authority for the seven Root fixtures.
pub(super) fn surfaces(
    owner: &SharedDisplayOwner,
    mask: &mut NativeFaceMask,
    face: &Face,
    show: &MaskShow,
    draft: &BirthDraft,
    target: &mut Target,
) -> SurfaceReport {
    let revision = draft.revision();
    let mut handles: [Option<Handle>; 7] = [None; 7];
    let mut admitted = 0;
    let mut uploaded = 0;
    let mut removed = 0;
    let (entries_before, entries_after) = {
        let mut owner = owner.borrow_mut();
        let session = owner
            .sessions
            .active_session()
            .unwrap_or_else(|| refuse("pressure-session"));
        let instance = owner.sessions.service_instance();
        let rect = Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
        for index in 0..handles.len() {
            let handle = match owner
                .sessions
                .request(session, Request::Admit { rect, z: 0 })
            {
                Ok(Response::Surface(handle)) => handle,
                _ => refuse("pressure-surface-admit"),
            };
            if handles.iter().flatten().any(|previous| *previous == handle) {
                refuse("pressure-duplicate-handle");
            }
            handles[index] = Some(handle);
            admitted += 1;
            let unit = |response| {
                if response != Ok(Response::Unit) {
                    refuse("pressure-upload-unit");
                }
            };
            unit(owner.sessions.request(
                session,
                Request::Visible {
                    surface: handle,
                    visible: false,
                },
            ));
            unit(owner.sessions.request(
                session,
                Request::BeginUpload {
                    surface: handle,
                    revision: 1,
                    rect,
                },
            ));
            let mut prepared = false;
            for _ in 0..=crate::native_compositor::MAX_COMPOSITOR_PIXELS
                / conduitos_compositor_core::MAX_CHUNK
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
                    _ => refuse("pressure-upload-prepare"),
                }
            }
            if !prepared {
                refuse("pressure-upload-work-bound");
            }
            unit(owner.sessions.upload_chunk(session, 0, &[0]));
            unit(owner.sessions.request(session, Request::CommitUpload));
            uploaded += 1;
        }
        let before = owner.service_cost();
        if owner
            .sessions
            .request(session, Request::Admit { rect, z: 0 })
            != Err(OwnerError::Capacity)
        {
            refuse("pressure-surface-nine");
        }
        let after = owner.service_cost();
        if before != after
            || owner.sessions.active_session() != Some(session)
            || owner.sessions.service_instance() != instance
            || !owner.sessions.is_active(session)
        {
            refuse("pressure-ninth-effects");
        }
        for handle in handles.into_iter().flatten() {
            if owner.sessions.request(session, Request::Remove(handle)) != Ok(Response::Unit) {
                refuse("pressure-surface-cleanup");
            }
            removed += 1;
        }
        if owner.sessions.active_session() != Some(session)
            || owner.sessions.service_instance() != instance
        {
            refuse("pressure-cleanup-session");
        }
        (before.entries, after.entries)
    };
    let before = target.writes;
    mask.show_local_notice("Surface capacity refused", target)
        .unwrap_or_else(|_| refuse("pressure-mask-recompose"));
    let recompose_pixels = target
        .writes
        .checked_sub(before)
        .unwrap_or_else(|| refuse("pressure-write-count"));
    if admitted != 7
        || uploaded != 7
        || removed != 7
        || recompose_pixels == 0
        || mask.show() != Some(show)
        || mask.scene().map(|scene| scene.face()) != Some(face)
        || draft.revision() != revision
        || mask.route_keyboard() != Ok(true)
    {
        refuse("pressure-semantic-custody");
    }
    SurfaceReport {
        admitted,
        uploaded,
        removed,
        entries_before,
        entries_after,
        recompose_pixels,
    }
}

pub(super) fn emit(scene: SceneReport, surfaces: SurfaceReport) {
    let mut out = FixedText::new();
    writeln!(out,
        "CONDUIT_COMPOSITOR_PRESSURE_PROOF {{\"schema\":\"conduit.conduitos/compositor-pressure-fixture@1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"fixture\":true,\"scene\":{{\"commands\":{},\"text_bytes_per_command\":{},\"command49_refusal\":\"TooManyCommands\",\"payload193_refusal\":\"PayloadTooLong\",\"scene_unchanged_after_refusals\":true,\"raster_commands\":{},\"raster_pixels_written\":{},\"scope\":\"root-production-typography-raster\",\"diagnostic_show\":false,\"protected_scene_interpretation\":false}},\"surfaces\":{{\"mask_surfaces\":1,\"extra_admitted\":{},\"extra_uploaded\":{},\"retained_total\":{},\"ninth_refusal\":\"OwnerError::Capacity\",\"guest_entries_before_ninth\":{},\"guest_entries_after_ninth\":{},\"same_session\":true,\"same_service_instance\":true,\"extra_removed\":{},\"source_face_unchanged\":true,\"show_unchanged\":true,\"draft_revision_unchanged\":true,\"keyboard_after_recompose\":true,\"recompose_pixels_written\":{},\"scope\":\"root-fixture-session-capacity\",\"mask_plan_reserves_eight\":false}},\"physical_scanout\":false,\"ordinary_product\":false}}",
        scene.commands, scene.text_bytes, scene.raster_commands, scene.raster_pixels,
        surfaces.admitted, surfaces.uploaded, 1 + surfaces.admitted, surfaces.entries_before,
        surfaces.entries_after, surfaces.removed, surfaces.recompose_pixels,
    ).unwrap_or_else(|_| refuse("pressure-receipt-capacity"));
    arch::early_write(out.as_bytes());
}
