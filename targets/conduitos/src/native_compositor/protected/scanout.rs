use super::*;
use crate::display::{DisplayError, DisplayFormat, PixelTarget};
use crate::native_compositor::{DamageRect, FrameReceipt, MAX_DAMAGE_RECTS};
use conduitos_compositor_core::FramePlan;
use conduitos_compositor_core::wire::{FrameSpan, PixelBytes};
impl NativeCompositor {
    pub fn compose_frame(
        &mut self,
        target: &mut impl PixelTarget,
    ) -> Result<FrameReceipt, NativeCompositorError> {
        self.invalidate();
        let ordinal = self
            .frame_sequence
            .checked_add(1)
            .ok_or(NativeCompositorError::ServiceLost)?;
        let result = self.scanout_frame(target, ordinal);
        match result {
            Ok((receipt, coverage)) => {
                self.frame_sequence = ordinal;
                if let Some(coverage) = coverage {
                    self.scanout = Some(Scanout {
                        sequence: ordinal,
                        coverage,
                        session: self.scope()?,
                    });
                }
                Ok(receipt)
            }
            Err(error) => {
                self.fail();
                Err(error)
            }
        }
    }
    fn scanout_frame(
        &self,
        target: &mut impl PixelTarget,
        ordinal: u64,
    ) -> Result<(FrameReceipt, Option<[u32; 8]>), NativeCompositorError> {
        let format = target.format().validate()?;
        let session = self.scope()?;
        let mut owner = self
            .owner()?
            .try_borrow_mut()
            .map_err(|_| NativeCompositorError::ServiceBusy)?;
        let returned = owner
            .sessions
            .request(session, Request::BeginFrame)
            .map_err(owner_error)?;
        let plan = match returned {
            Response::FrameStarted(plan) => plan,
            Response::FrameIdle {
                cursor_visible,
                focus_visible,
            } => {
                record_frame(&owner, ordinal, 0, 0, 0);
                return Ok((
                    FrameReceipt {
                        frame_sequence: ordinal,
                        surfaces_composed: 0,
                        pixels_written: 0,
                        damage_count: 0,
                        damage_rects: [DamageRect::default(); MAX_DAMAGE_RECTS],
                        conservative_fallback: false,
                        cursor_visible,
                        focus_visible,
                    },
                    None,
                ));
            }
            Response::Refused(error) => return Err(core_error(error)),
            _ => return Err(NativeCompositorError::ServiceProtocol),
        };
        let (maximum_steps, expected_pixels) = validate_plan(plan, format)?;
        let mut written = 0u32;
        let mut completed = None;
        for _ in 0..maximum_steps {
            let response = owner
                .sessions
                .with_frame_step(
                    session,
                    plan.sequence,
                    conduitos_compositor_core::MAX_CHUNK as u32,
                    |span, pixels| {
                        copy_span(target, format, span, pixels)?;
                        Ok::<_, DisplayError>(span)
                    },
                )
                .map_err(owner_error)?;
            let span = response.map_err(core_error)??;
            written = written
                .checked_add(span.pixels)
                .ok_or(NativeCompositorError::InvalidBounds)?;
            if written > expected_pixels {
                return Err(NativeCompositorError::ServiceProtocol);
            }
            if span.complete {
                completed = Some(span);
                break;
            }
        }
        let final_span = completed.ok_or(NativeCompositorError::ServiceProtocol)?;
        if written != expected_pixels {
            return Err(NativeCompositorError::ServiceProtocol);
        }
        // Successful validated copies, not guest completion, establish scanout.
        match response(
            owner
                .sessions
                .request(
                    session,
                    Request::AcknowledgeScanout {
                        sequence: plan.sequence,
                    },
                )
                .map_err(owner_error)?,
        )? {
            Response::Unit => {}
            _ => return Err(NativeCompositorError::ServiceProtocol),
        }
        record_frame(&owner, ordinal, plan.sequence, written, plan.count);
        let mut damage_rects = [DamageRect::default(); MAX_DAMAGE_RECTS];
        for (target, source) in damage_rects.iter_mut().zip(plan.rects) {
            *target = DamageRect {
                x: source.x,
                y: source.y,
                width: source.width,
                height: source.height,
            };
        }
        Ok((
            FrameReceipt {
                frame_sequence: ordinal,
                surfaces_composed: final_span.selected_mask.count_ones() as u8,
                pixels_written: written,
                damage_count: plan.count,
                damage_rects,
                conservative_fallback: plan.conservative_fallback,
                cursor_visible: plan.cursor_visible,
                focus_visible: plan.focus_visible,
            },
            Some(final_span.coverage),
        ))
    }
    pub fn scanout_acknowledgement(
        &self,
        expected: &CompositionReceipt,
        face_revision: u64,
    ) -> Option<ScanoutAcknowledgement<'_>> {
        let scanout = self.scanout.as_ref()?;
        let session = self.session?;
        if scanout.session != session {
            return None;
        }
        let surface = self.surfaces.iter().find(|s| s.id == expected.surface_id)?;
        if surface.receipt.as_ref() != Some(expected)
            || surface.binding.as_ref()?.face_revision != face_revision
        {
            return None;
        }
        let pixels = *scanout.coverage.get(usize::from(surface.handle.slot))?;
        if pixels == 0 {
            return None;
        }
        let owner = self.owner.as_ref()?.try_borrow().ok()?;
        if !owner.sessions.is_active(session) {
            return None;
        }
        Some(ScanoutAcknowledgement::protected(
            surface.receipt.as_ref()?,
            face_revision,
            scanout.sequence,
            pixels,
            owner,
        ))
    }
}
fn validate_plan(
    plan: FramePlan,
    format: DisplayFormat,
) -> Result<(u32, u32), NativeCompositorError> {
    if plan.count == 0 || usize::from(plan.count) > MAX_DAMAGE_RECTS {
        return Err(NativeCompositorError::ServiceProtocol);
    }
    let mut steps = 0u32;
    let mut pixels = 0u32;
    for rect in &plan.rects[..usize::from(plan.count)] {
        if rect.width == 0
            || rect.height == 0
            || rect
                .x
                .checked_add(rect.width)
                .is_none_or(|end| end > format.width)
            || rect
                .y
                .checked_add(rect.height)
                .is_none_or(|end| end > format.height)
        {
            return Err(NativeCompositorError::InvalidBounds);
        }
        pixels = pixels
            .checked_add(
                rect.width
                    .checked_mul(rect.height)
                    .ok_or(NativeCompositorError::InvalidBounds)?,
            )
            .ok_or(NativeCompositorError::InvalidBounds)?;
        let row_steps = rect
            .width
            .div_ceil(conduitos_compositor_core::MAX_CHUNK as u32);
        steps = steps
            .checked_add(
                row_steps
                    .checked_mul(rect.height)
                    .ok_or(NativeCompositorError::InvalidBounds)?,
            )
            .ok_or(NativeCompositorError::InvalidBounds)?;
    }
    if u64::from(pixels) > u64::from(format.width) * u64::from(format.height) {
        return Err(NativeCompositorError::ServiceProtocol);
    }
    Ok((steps, pixels))
}
fn copy_span(
    target: &mut impl PixelTarget,
    format: DisplayFormat,
    span: FrameSpan,
    pixels: PixelBytes<'_>,
) -> Result<(), DisplayError> {
    if span.y >= format.height
        || span
            .x
            .checked_add(span.pixels)
            .is_none_or(|end| end > format.width)
    {
        return Err(DisplayError::InvalidExtent);
    }
    let mut buffer = [0u32; 1024];
    let mut at = 0u32;
    let mut input = pixels.pixels();
    while at < span.pixels {
        let count = usize::try_from((span.pixels - at).min(buffer.len() as u32))
            .map_err(|_| DisplayError::InvalidExtent)?;
        for pixel in &mut buffer[..count] {
            let source = input.next().ok_or(DisplayError::BufferTooSmall)?;
            *pixel = format.pixel((source >> 16) as u8, (source >> 8) as u8, source as u8);
        }
        target.write_span(span.x + at, span.y, &buffer[..count])?;
        at += count as u32;
    }
    if input.next().is_some() {
        return Err(DisplayError::InvalidExtent);
    }
    Ok(())
}

/// Execution telemetry follows successful copies and guest acknowledgement.
/// It is not a physical display, vblank, or human observation receipt.
fn record_frame(
    owner: &crate::display_owner::DisplayOwner,
    frame_sequence: u64,
    guest_sequence: u64,
    pixels_written: u32,
    damage_count: u8,
) {
    use core::fmt::Write;
    let cost = owner.service_cost();
    let mut line = crate::sign_format::FixedText::new();
    if writeln!(
        line,
        "CONDUIT_COMPOSITOR_SERVICE_FRAME service_instance={} frame_sequence={} guest_sequence={} pixels_written={} damage_count={} entries={} privilege_transitions={} address_space_switches={} scheduler_returns={}",
        owner.sessions.service_instance(), frame_sequence, guest_sequence,
        pixels_written, damage_count, cost.entries, cost.privilege_transitions,
        cost.address_space_switches, cost.scheduler_returns,
    ).is_ok() {
        crate::arch::early_write(line.as_bytes());
    }
}
