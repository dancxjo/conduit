//! Fixture evidence for the actual independent renderer image, not the product
//! compositor or physical scanout. Expected pixels come from simple geometry.
use super::refuse;
use crate::{arch, protected_compositor::CompositorService, sign_format::FixedText};
use conduitos_compositor_core::{
    Error, FrameRect, Handle, Rect,
    wire::{Request, Response},
};
use core::fmt::Write;
use sha2::{Digest, Sha256};

const WIDTH: u32 = 32;
const HEIGHT: u32 = 24;
const VIEW_PIXELS: usize = (WIDTH * HEIGHT) as usize;
const BLUE: u32 = 0x0000_00ff;
const RED: u32 = 0x00ff_0000;
const IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/compositor.elf"));

fn response(service: &mut CompositorService, request: Request) -> Response {
    service
        .request(request)
        .unwrap_or_else(|_| refuse("compositor-fixture-transport"))
}
fn unit(service: &mut CompositorService, request: Request) {
    if response(service, request) != Response::Unit {
        refuse("compositor-fixture-command");
    }
}
fn admit(service: &mut CompositorService, rect: Rect, z: u8) -> Handle {
    match response(service, Request::Admit { rect, z }) {
        Response::Surface(handle) => handle,
        _ => refuse("compositor-fixture-admit"),
    }
}
fn upload(service: &mut CompositorService, surface: Handle, rect: Rect, pixels: &[u32]) {
    unit(
        service,
        Request::BeginUpload {
            surface,
            revision: 1,
            rect,
        },
    );
    let mut prepared = false;
    for _ in 0..VIEW_PIXELS {
        match response(service, Request::PrepareUpload { pixels: 17 }) {
            Response::Prepared(true) => {
                prepared = true;
                break;
            }
            Response::Prepared(false) => {}
            _ => refuse("compositor-fixture-prepare"),
        }
    }
    if !prepared {
        refuse("compositor-fixture-prepare-bound");
    }
    // The transfer window is the only bulk carrier; exchange holds no raster.
    if service
        .upload_chunk(0, pixels)
        .unwrap_or_else(|_| refuse("compositor-fixture-upload"))
        != Response::Unit
    {
        refuse("compositor-fixture-upload-response");
    }
    unit(service, Request::CommitUpload);
    unit(
        service,
        Request::Visible {
            surface,
            visible: true,
        },
    );
}
fn expected(x: u32, y: u32, overlay: bool) -> u32 {
    if overlay && (4..12).contains(&x) && (4..12).contains(&y) {
        RED
    } else if x < 16 && y < 12 {
        BLUE
    } else {
        0
    }
}

/// Validate every span, uniqueness and exact expected damage coverage before the
/// caller can issue the explicit fixture acknowledgement. No framebuffer effect.
fn frame(service: &mut CompositorService, overlay: bool, full: bool) -> (u64, u32, u32) {
    let plan = match response(service, Request::BeginFrame) {
        Response::FrameStarted(plan) => plan,
        _ => refuse("compositor-fixture-begin-frame"),
    };
    let expected_rect = if full {
        FrameRect {
            x: 0,
            y: 0,
            width: WIDTH,
            height: HEIGHT,
        }
    } else {
        FrameRect {
            x: 4,
            y: 4,
            width: 8,
            height: 8,
        }
    };
    if plan.count != 1
        || plan.rects[0] != expected_rect
        || plan.conservative_fallback
        || plan.cursor_visible
        || plan.focus_visible
    {
        refuse("compositor-fixture-frame-plan");
    }
    let sequence = plan.sequence;
    let mut seen = [false; VIEW_PIXELS];
    let mut compared = 0u32;
    let mut steps = 0u32;
    let mut completed = false;
    for _ in 0..VIEW_PIXELS {
        let chunk = service
            .frame_step(sequence, 7)
            .unwrap_or_else(|_| refuse("compositor-fixture-frame-transport"))
            .unwrap_or_else(|_| refuse("compositor-fixture-frame-refusal"));
        steps += 1;
        let span = chunk.span;
        if span.sequence != sequence
            || span.pixels == 0
            || span.pixels > 7
            || span.y >= HEIGHT
            || span
                .x
                .checked_add(span.pixels)
                .is_none_or(|end| end > WIDTH)
        {
            refuse("compositor-fixture-span-bounds");
        }
        let mut count = 0u32;
        for (offset, pixel) in chunk.pixels.pixels().enumerate() {
            let x = span.x + offset as u32;
            let index = (span.y * WIDTH + x) as usize;
            if index >= VIEW_PIXELS || seen[index] || pixel != expected(x, span.y, overlay) {
                refuse("compositor-fixture-pixel-comparison");
            }
            seen[index] = true;
            compared += 1;
            count += 1;
        }
        if count != span.pixels {
            refuse("compositor-fixture-span-length");
        }
        if span.complete {
            completed = true;
            break;
        }
    }
    if !completed {
        refuse("compositor-fixture-frame-bound");
    }
    // Initial forced repaint covers the viewport; removal must damage exactly
    // the removed overlay rectangle, exercising old-rectangle invalidation.
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let should_visit = full || ((4..12).contains(&x) && (4..12).contains(&y));
            if seen[(y * WIDTH + x) as usize] != should_visit {
                refuse("compositor-fixture-damage-coverage");
            }
        }
    }
    (sequence, compared, steps)
}

pub(super) fn run() {
    let mut service = CompositorService::install_embedded(WIDTH as u16, HEIGHT as u16)
        .unwrap_or_else(|_| refuse("compositor-fixture-install"));
    let old_instance = service.instance();
    let blue_rect = Rect {
        x: 0,
        y: 0,
        width: 16,
        height: 12,
    };
    let red_rect = Rect {
        x: 4,
        y: 4,
        width: 8,
        height: 8,
    };
    let blue = admit(&mut service, blue_rect, 0);
    upload(&mut service, blue, blue_rect, &[BLUE; 16 * 12]);
    let red = admit(&mut service, red_rect, 1);
    // Committing red repacks and preserves the already committed blue raster.
    upload(&mut service, red, red_rect, &[RED; 8 * 8]);
    unit(&mut service, Request::FailFrame); // explicit full viewport damage
    if response(&mut service, Request::Pointer { x: 6, y: 6 }) != Response::Pointer(None) {
        refuse("compositor-fixture-input-before-frame");
    }
    let (sequence, first_pixels, first_steps) = frame(&mut service, true, true);
    if response(&mut service, Request::Pointer { x: 6, y: 6 }) != Response::Pointer(None) {
        refuse("compositor-fixture-input-before-ack");
    }
    // Fixture-only acknowledgement after comparison, not a physical presentation
    // receipt and never a semantic Show acknowledgement.
    unit(&mut service, Request::AcknowledgeScanout { sequence });
    match response(&mut service, Request::Pointer { x: 6, y: 6 }) {
        Response::Pointer(Some(route))
            if route.surface == red && route.revision == 1 && route.x == 2 && route.y == 2 => {}
        _ => refuse("compositor-fixture-top-route"),
    }
    unit(&mut service, Request::Remove(red));
    let (sequence, removed_pixels, removed_steps) = frame(&mut service, false, false);
    unit(&mut service, Request::AcknowledgeScanout { sequence });
    match response(&mut service, Request::Pointer { x: 6, y: 6 }) {
        Response::Pointer(Some(route))
            if route.surface == blue && route.revision == 1 && route.x == 6 && route.y == 6 => {}
        _ => refuse("compositor-fixture-underlying-route"),
    }
    service.quarantine();
    if service.request(Request::Pointer { x: 6, y: 6 }).is_ok() {
        refuse("compositor-fixture-quarantine");
    }
    let old_cost = service.cost();
    let mut fresh = CompositorService::install_embedded(WIDTH as u16, HEIGHT as u16)
        .unwrap_or_else(|_| refuse("compositor-fixture-reinstall"));
    if fresh.instance() == old_instance
        || fresh.instance() == 0
        || response(&mut fresh, Request::Pointer { x: 6, y: 6 }) != Response::Pointer(None)
        || response(&mut fresh, Request::Remove(blue)) != Response::Refused(Error::Stale)
        || response(&mut fresh, Request::BeginFrame)
            != (Response::FrameIdle {
                cursor_visible: false,
                focus_visible: false,
            })
    {
        refuse("compositor-fixture-restart-state");
    }
    // Old clients remain alive through independent replacement. Their requests
    // and transfer writes stay terminal and cannot alter the fresh service.
    if service.instance() != old_instance
        || service.cost() != old_cost
        || service.request(Request::DamagePending).is_ok()
        || service.upload_chunk(0, &[0]).is_ok()
        || service.frame_step(1, 1).is_ok()
    {
        refuse("compositor-fixture-retired-client");
    }
    service.quarantine();
    drop(service);
    // Inert stale Drop must not free the replacement's backend slot.
    if response(&mut fresh, Request::DamagePending) != Response::DamagePending(false) {
        refuse("compositor-fixture-stale-drop");
    }
    drop(fresh);
    if first_pixels != VIEW_PIXELS as u32 || removed_pixels != 64 {
        refuse("compositor-fixture-pixel-total");
    }
    let digest = Sha256::digest(IMAGE);
    let mut sign = FixedText::new();
    write!(sign, "CONDUIT_COMPOSITOR_GUEST_PROOF {{\"schema\":\"conduit.conduitos/compositor-guest-fixture@1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"fixture\":true,\"architecture\":\"x86_64\",\"privilege\":\"ring3\",\"production_compositor\":false,\"physical_scanout\":false,\"semantic_show_acknowledged\":false,\"driver_isolation\":false,\"dma_isolation\":false,\"pixel_comparisons\":{},\"initial_pixels\":{},\"removed_rectangle_pixels\":{},\"frame_steps\":{},\"fixture_acknowledgements\":2,\"old_instance_quarantined\":true,\"replacement_while_old_alive\":true,\"stale_drop_preserves_replacement\":true,\"fresh_instance_empty\":true,\"old_handle_rejected\":true,\"image_sha256\":\"", first_pixels + removed_pixels, first_pixels, removed_pixels, first_steps + removed_steps)
        .unwrap_or_else(|_| refuse("compositor-fixture-sign"));
    for byte in digest {
        write!(sign, "{byte:02x}").unwrap_or_else(|_| refuse("compositor-fixture-hash"));
    }
    writeln!(sign, "\"}}").unwrap_or_else(|_| refuse("compositor-fixture-sign-end"));
    arch::early_write(sign.as_bytes());
}
