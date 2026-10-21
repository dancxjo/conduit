use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Admit {
        rect: Rect,
        z: u8,
    },
    Remove(Handle),
    Resize {
        surface: Handle,
        rect: Rect,
        z: u8,
    },
    Place {
        surface: Handle,
        x: i16,
        y: i16,
        z: u8,
    },
    Visible {
        surface: Handle,
        visible: bool,
    },
    Focus(Handle),
    Cursor {
        position: Option<(u32, u32)>,
        hover: bool,
    },
    /// Renderer-local upload sequence, never a Face revision.
    BeginUpload {
        surface: Handle,
        revision: u64,
        rect: Rect,
    },
    PrepareUpload {
        pixels: u32,
    },
    UploadWindow {
        offset: u32,
        window_offset: u32,
        pixels: u32,
    },
    CommitUpload,
    AbortUpload,
    BeginFrame,
    FrameStep {
        sequence: u64,
        pixels: u32,
    },
    AcknowledgeScanout {
        sequence: u64,
    },
    FailFrame,
    Pointer {
        x: u32,
        y: u32,
    },
    Keyboard,
    Initialize {
        width: u16,
        height: u16,
    },
}
pub fn decode_request(bytes: &[u8]) -> Result<(Envelope, Request), CodecError> {
    let (e, op, mut r) = header(bytes)?;
    let request = match op {
        1 => Request::Admit {
            rect: r.rect()?,
            z: r.u8()?,
        },
        2 => Request::Remove(r.handle()?),
        3 => Request::Place {
            surface: r.handle()?,
            x: r.u16()? as i16,
            y: r.u16()? as i16,
            z: r.u8()?,
        },
        4 => Request::Visible {
            surface: r.handle()?,
            visible: r.bool()?,
        },
        5 => Request::Focus(r.handle()?),
        6 => {
            let present = r.bool()?;
            let hover = r.bool()?;
            let x = r.u32()?;
            let y = r.u32()?;
            if (!present && (x != 0 || y != 0))
                || x > i32::MAX as u32 - 10
                || y > i32::MAX as u32 - 16
            {
                return Err(CodecError::Value);
            }
            Request::Cursor {
                position: present.then_some((x, y)),
                hover,
            }
        }
        7 => {
            let surface = r.handle()?;
            let revision = r.u64()?;
            nonzero(revision)?;
            Request::BeginUpload {
                surface,
                revision,
                rect: r.rect()?,
            }
        }
        8 => {
            let pixels = r.u32()?;
            limit(pixels)?;
            Request::PrepareUpload { pixels }
        }
        9 => {
            let offset = r.u32()?;
            let window_offset = r.u32()?;
            let pixels = r.u32()?;
            window_range(window_offset, pixels)?;
            if offset
                .checked_add(pixels)
                .is_none_or(|end| end > MAX_PIXELS as u32)
            {
                return Err(CodecError::Value);
            }
            Request::UploadWindow {
                offset,
                window_offset,
                pixels,
            }
        }
        10 => Request::CommitUpload,
        11 => Request::AbortUpload,
        12 => Request::BeginFrame,
        13 => {
            let sequence = r.u64()?;
            nonzero(sequence)?;
            let pixels = r.u32()?;
            limit(pixels)?;
            Request::FrameStep { sequence, pixels }
        }
        14 => {
            let sequence = r.u64()?;
            nonzero(sequence)?;
            Request::AcknowledgeScanout { sequence }
        }
        15 => Request::FailFrame,
        16 => Request::Pointer {
            x: r.u32()?,
            y: r.u32()?,
        },
        17 => Request::Keyboard,
        18 => {
            let width = r.u16()?;
            let height = r.u16()?;
            if width == 0 || height == 0 {
                return Err(CodecError::Value);
            }
            Request::Initialize { width, height }
        }
        19 => Request::Resize {
            surface: r.handle()?,
            rect: r.rect()?,
            z: r.u8()?,
        },
        _ => return Err(CodecError::Operation),
    };
    r.done()?;
    Ok((e, request))
}
pub fn encode_request(
    e: Envelope,
    request: Request,
    bytes: &mut [u8],
) -> Result<usize, CodecError> {
    let (op, size) = match request {
        Request::Admit { .. } => (1, 9),
        Request::Remove(_) => (2, 9),
        Request::Place { .. } => (3, 14),
        Request::Visible { .. } => (4, 10),
        Request::Focus(_) => (5, 9),
        Request::Cursor { .. } => (6, 10),
        Request::BeginUpload { .. } => (7, 25),
        Request::PrepareUpload { .. } => (8, 4),
        Request::UploadWindow { .. } => (9, 12),
        Request::CommitUpload => (10, 0),
        Request::AbortUpload => (11, 0),
        Request::BeginFrame => (12, 0),
        Request::FrameStep { .. } => (13, 12),
        Request::AcknowledgeScanout { .. } => (14, 8),
        Request::FailFrame => (15, 0),
        Request::Pointer { .. } => (16, 8),
        Request::Keyboard => (17, 0),
        Request::Initialize { .. } => (18, 4),
        Request::Resize { .. } => (19, 18),
    };
    let length = HEADER_BYTES + size;
    let mut w = begin(bytes, e, op, length)?;
    match request {
        Request::Admit { rect, z } => {
            w.rect(rect)?;
            w.u8(z)?;
        }
        Request::Remove(h) | Request::Focus(h) => w.handle(h)?,
        Request::Resize { surface, rect, z } => {
            w.handle(surface)?;
            w.rect(rect)?;
            w.u8(z)?;
        }
        Request::Place { surface, x, y, z } => {
            w.handle(surface)?;
            w.u16(x as u16)?;
            w.u16(y as u16)?;
            w.u8(z)?;
        }
        Request::Visible { surface, visible } => {
            w.handle(surface)?;
            w.bool(visible)?;
        }
        Request::Cursor { position, hover } => {
            w.bool(position.is_some())?;
            w.bool(hover)?;
            let (x, y) = position.unwrap_or((0, 0));
            w.u32(x)?;
            w.u32(y)?;
        }
        Request::BeginUpload {
            surface,
            revision,
            rect,
        } => {
            w.handle(surface)?;
            w.u64(revision)?;
            w.rect(rect)?;
        }
        Request::PrepareUpload { pixels } => w.u32(pixels)?,
        Request::UploadWindow {
            offset,
            window_offset,
            pixels,
        } => {
            w.u32(offset)?;
            w.u32(window_offset)?;
            w.u32(pixels)?;
        }
        Request::FrameStep { sequence, pixels } => {
            w.u64(sequence)?;
            w.u32(pixels)?;
        }
        Request::AcknowledgeScanout { sequence } => w.u64(sequence)?,
        Request::Pointer { x, y } => {
            w.u32(x)?;
            w.u32(y)?;
        }
        Request::Initialize { width, height } => {
            w.u16(width)?;
            w.u16(height)?;
        }
        _ => {}
    }
    decode_request(&bytes[..length])?;
    Ok(length)
}
