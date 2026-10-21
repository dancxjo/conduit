use super::*;
use crate::Route;
/// Numeric metadata for pixels in the separately admitted transfer window.
/// Coverage describes composition, never an acknowledged semantic Show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameSpan {
    pub sequence: u64,
    pub x: u32,
    pub y: u32,
    pub window_offset: u32,
    pub pixels: u32,
    pub complete: bool,
    pub selected_mask: u8,
    pub coverage: [u32; MAX_SURFACES],
}
impl FrameSpan {
    fn validate(self) -> Result<(), CodecError> {
        nonzero(self.sequence)?;
        window_range(self.window_offset, self.pixels)?;
        if self
            .x
            .checked_add(self.pixels)
            .is_none_or(|end| end > u32::from(u16::MAX))
            || self.y >= u32::from(u16::MAX)
        {
            return Err(CodecError::Value);
        }
        let mut sum = 0u32;
        for (i, count) in self.coverage.into_iter().enumerate() {
            if count > 0 && self.selected_mask & (1 << i) == 0 {
                return Err(CodecError::Value);
            }
            sum = sum.checked_add(count).ok_or(CodecError::Value)?;
        }
        if sum > MAX_PIXELS as u32 || (!self.complete && (sum != 0 || self.selected_mask != 0)) {
            return Err(CodecError::Value);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Unit,
    Surface(Handle),
    Prepared(bool),
    FrameStarted(FramePlan),
    FrameIdle {
        cursor_visible: bool,
        focus_visible: bool,
    },
    Frame(FrameSpan),
    Pointer(Option<Route>),
    Keyboard(Option<(Handle, u64)>),
    Focused(Option<Handle>),
    Refused(Error),
}
pub fn decode_response(bytes: &[u8]) -> Result<(Envelope, Response), CodecError> {
    let (e, op, mut r) = header(bytes)?;
    let response = match op {
        0x8000 => Response::Unit,
        0x8001 => Response::Surface(r.handle()?),
        0x8002 => Response::Prepared(r.bool()?),
        0x8003 => {
            let sequence = r.u64()?;
            let count = r.u8()?;
            let mut rects = [FrameRect::default(); 16];
            for rect in &mut rects {
                *rect = FrameRect {
                    x: r.u32()?,
                    y: r.u32()?,
                    width: r.u32()?,
                    height: r.u32()?,
                };
            }
            let plan = FramePlan {
                sequence,
                count,
                rects,
                conservative_fallback: r.bool()?,
                cursor_visible: r.bool()?,
                focus_visible: r.bool()?,
            };
            validate_plan(plan)?;
            Response::FrameStarted(plan)
        }
        0x8004 => {
            let sequence = r.u64()?;
            let x = r.u32()?;
            let y = r.u32()?;
            let window_offset = r.u32()?;
            let pixels = r.u32()?;
            let complete = r.bool()?;
            let selected_mask = r.u8()?;
            let mut coverage = [0; MAX_SURFACES];
            for count in &mut coverage {
                *count = r.u32()?;
            }
            let frame = FrameSpan {
                sequence,
                x,
                y,
                window_offset,
                pixels,
                complete,
                selected_mask,
                coverage,
            };
            frame.validate()?;
            Response::Frame(frame)
        }
        0x8005 => Response::Pointer(if r.bool()? {
            let surface = r.handle()?;
            let revision = r.u64()?;
            nonzero(revision)?;
            Some(Route {
                surface,
                revision,
                x: r.u16()?,
                y: r.u16()?,
            })
        } else {
            None
        }),
        0x8006 => Response::Keyboard(if r.bool()? {
            let handle = r.handle()?;
            let revision = r.u64()?;
            nonzero(revision)?;
            Some((handle, revision))
        } else {
            None
        }),
        0x8009 => Response::Focused(if r.bool()? { Some(r.handle()?) } else { None }),
        0x8007 => Response::Refused(error(r.u16()?)?),
        0x8008 => Response::FrameIdle {
            cursor_visible: r.bool()?,
            focus_visible: r.bool()?,
        },
        _ => return Err(CodecError::Operation),
    };
    r.done()?;
    Ok((e, response))
}
pub fn encode_response(
    e: Envelope,
    response: Response,
    bytes: &mut [u8],
) -> Result<usize, CodecError> {
    let (tag, size) = match response {
        Response::Unit => (0, 0),
        Response::Surface(_) => (1, 9),
        Response::Prepared(_) => (2, 1),
        Response::FrameStarted(_) => (3, 268),
        Response::Frame(_) => (4, 58),
        Response::Pointer(value) => (5, if value.is_some() { 22 } else { 1 }),
        Response::Keyboard(value) => (6, if value.is_some() { 18 } else { 1 }),
        Response::Refused(_) => (7, 2),
        Response::Focused(value) => (9, if value.is_some() { 10 } else { 1 }),
        Response::FrameIdle { .. } => (8, 2),
    };
    let length = HEADER_BYTES + size;
    let mut w = begin(bytes, e, 0x8000 | tag, length)?;
    match response {
        Response::Unit => {}
        Response::Surface(h) => w.handle(h)?,
        Response::Prepared(value) => w.bool(value)?,
        Response::FrameStarted(plan) => {
            validate_plan(plan)?;
            w.u64(plan.sequence)?;
            w.u8(plan.count)?;
            for rect in plan.rects {
                w.u32(rect.x)?;
                w.u32(rect.y)?;
                w.u32(rect.width)?;
                w.u32(rect.height)?;
            }
            w.bool(plan.conservative_fallback)?;
            w.bool(plan.cursor_visible)?;
            w.bool(plan.focus_visible)?;
        }
        Response::Frame(f) => {
            f.validate()?;
            w.u64(f.sequence)?;
            w.u32(f.x)?;
            w.u32(f.y)?;
            w.u32(f.window_offset)?;
            w.u32(f.pixels)?;
            w.bool(f.complete)?;
            w.u8(f.selected_mask)?;
            for count in f.coverage {
                w.u32(count)?;
            }
        }
        Response::Pointer(value) => {
            w.bool(value.is_some())?;
            if let Some(route) = value {
                w.handle(route.surface)?;
                nonzero(route.revision)?;
                w.u64(route.revision)?;
                w.u16(route.x)?;
                w.u16(route.y)?;
            }
        }
        Response::Keyboard(value) => {
            w.bool(value.is_some())?;
            if let Some((h, revision)) = value {
                w.handle(h)?;
                nonzero(revision)?;
                w.u64(revision)?;
            }
        }
        Response::Focused(value) => {
            w.bool(value.is_some())?;
            if let Some(handle) = value {
                w.handle(handle)?;
            }
        }
        Response::Refused(value) => w.u16(error_code(value))?,
        Response::FrameIdle {
            cursor_visible,
            focus_visible,
        } => {
            w.bool(cursor_visible)?;
            w.bool(focus_visible)?;
        }
    }
    Ok(length)
}
fn error(code: u16) -> Result<Error, CodecError> {
    match code {
        1 => Ok(Error::Bounds),
        2 => Ok(Error::Capacity),
        3 => Ok(Error::Stale),
        4 => Ok(Error::Busy),
        5 => Ok(Error::Incomplete),
        6 => Ok(Error::Order),
        7 => Ok(Error::NoDamage),
        8 => Ok(Error::Exhausted),
        _ => Err(CodecError::Value),
    }
}
fn error_code(value: Error) -> u16 {
    match value {
        Error::Bounds => 1,
        Error::Capacity => 2,
        Error::Stale => 3,
        Error::Busy => 4,
        Error::Incomplete => 5,
        Error::Order => 6,
        Error::NoDamage => 7,
        Error::Exhausted => 8,
    }
}

fn validate_plan(plan: FramePlan) -> Result<(), CodecError> {
    nonzero(plan.sequence)?;
    let count = usize::from(plan.count);
    if count == 0 || count > plan.rects.len() {
        return Err(CodecError::Value);
    }
    for (i, rect) in plan.rects.into_iter().enumerate() {
        if i >= count {
            if rect != FrameRect::default() {
                return Err(CodecError::Value);
            }
        } else if rect.width == 0
            || rect.height == 0
            || rect
                .x
                .checked_add(rect.width)
                .is_none_or(|end| end > u32::from(u16::MAX))
            || rect
                .y
                .checked_add(rect.height)
                .is_none_or(|end| end > u32::from(u16::MAX))
        {
            return Err(CodecError::Value);
        }
    }
    Ok(())
}
