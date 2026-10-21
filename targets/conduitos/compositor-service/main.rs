//! Independently linked retained compositor. No Root renderer or device imports.
#![no_std]
#![no_main]
#[cfg(not(target_arch = "x86_64"))]
compile_error!("the compositor service image currently has only an x86_64 entry contract");
mod memory;
// Shared freestanding compiler intrinsics, not Root/provider implementation.
#[path = "../domain/memory.rs"]
mod compiler_memory;
use conduitos_compositor_core::wire::{self, FrameSpan, Request, Response};
use conduitos_compositor_core::{Compositor, Error, MAX_SURFACES};
use core::slice;

/// SAFETY: caller installs precisely the admitted service grants and supplies the
/// fixed exchange pointer in rdi. Each entry starts a fresh stack; persistent
/// state exists only in private metadata. A fault requires Root quarantine.
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub unsafe extern "C" fn compositor_entry(exchange: *mut u8) -> ! {
    if exchange as usize != memory::EXCHANGE {
        finish(3);
    }
    // dispatch returns before the terminal gate: no live Rust borrow or suspended
    // stack-local operation is resumed on a subsequent entry.
    if unsafe { dispatch() }.is_err() {
        finish(3);
    }
    finish(0)
}

unsafe fn dispatch() -> Result<(), ()> {
    let exchange =
        unsafe { slice::from_raw_parts_mut(memory::EXCHANGE as *mut u8, memory::EXCHANGE_BYTES) };
    let length = u32::from_le_bytes(exchange[8..12].try_into().map_err(|_| ())?) as usize;
    let (envelope, request) =
        wire::decode_request(exchange.get(..length).ok_or(())?).map_err(|_| ())?;
    let result = match request {
        Request::Initialize { width, height } => {
            unsafe { memory::initialize(envelope.instance, envelope.request, width, height) }
                .map(|()| Response::Unit)
                .map_err(memory_error)
        }
        command => unsafe {
            memory::with_core(envelope.instance, envelope.request, |core| {
                execute(core, command)
            })
        }
        .map_err(memory_error)
        .and_then(|result| result),
    };
    let response = result.unwrap_or_else(Response::Refused);
    wire::encode_response(envelope, response, exchange).map_err(|_| ())?;
    Ok(())
}

fn memory_error(error: memory::MemoryError) -> Error {
    match error {
        memory::MemoryError::WrongInstance | memory::MemoryError::StaleRequest => Error::Stale,
        memory::MemoryError::Dimensions => Error::Bounds,
        memory::MemoryError::NotInitialized | memory::MemoryError::AlreadyInitialized => {
            Error::Order
        }
    }
}

fn execute(core: &mut Compositor<'static>, command: Request) -> Result<Response, Error> {
    match command {
        Request::Admit { rect, z } => core.admit(rect, z).map(Response::Surface),
        Request::Remove(surface) => core.remove(surface).map(|()| Response::Unit),
        Request::Resize { surface, rect, z } => {
            core.resize(surface, rect, z).map(|()| Response::Unit)
        }
        Request::Place { surface, x, y, z } => {
            core.place(surface, x, y, z).map(|()| Response::Unit)
        }
        Request::Visible { surface, visible } => {
            core.visible(surface, visible).map(|()| Response::Unit)
        }
        Request::Focus(surface) => core.focus(surface).map(|()| Response::Unit),
        Request::Cursor { position, hover } => {
            core.cursor(position, hover).map(|()| Response::Unit)
        }
        Request::BeginUpload {
            surface,
            revision,
            rect,
        } => core
            .begin_upload(surface, revision, rect)
            .map(|()| Response::Unit),
        Request::PrepareUpload { pixels } => {
            core.prepare_upload(pixels as usize).map(Response::Prepared)
        }
        Request::UploadWindow {
            offset,
            window_offset,
            pixels,
        } => {
            let (start, count) = window(window_offset, pixels)?;
            // x86_64 native u32 is canonical LE. This temporary grant is disjoint
            // from both retained arenas and is never retained by upload_chunk.
            let input = unsafe { slice::from_raw_parts(start as *const u32, count) };
            core.upload_chunk(offset as usize, input)
                .map(|()| Response::Unit)
        }
        Request::CommitUpload => core.commit_upload().map(|()| Response::Unit),
        Request::AbortUpload => core.abort_upload().map(|()| Response::Unit),
        Request::BeginFrame => match core.begin_frame() {
            Ok(_) => core
                .frame_plan()
                .map(Response::FrameStarted)
                .ok_or(Error::Order),
            Err(Error::NoDamage) => {
                let (cursor_visible, focus_visible) = core.logical_affordances();
                Ok(Response::FrameIdle {
                    cursor_visible,
                    focus_visible,
                })
            }
            Err(error) => Err(error),
        },
        Request::FrameStep { sequence, pixels } => {
            let (start, count) = window(0, pixels)?;
            let output = unsafe { slice::from_raw_parts_mut(start as *mut u32, count) };
            let span = core.compose_next(sequence, output)?;
            let coverage = if span.complete {
                core.completed().ok_or(Error::Incomplete)?.coverage()
            } else {
                [0; MAX_SURFACES]
            };
            Ok(Response::Frame(FrameSpan {
                sequence: span.sequence,
                x: span.x,
                y: span.y,
                window_offset: 0,
                pixels: span.pixels as u32,
                complete: span.complete,
                selected_mask: span.selected_mask,
                coverage,
            }))
        }
        Request::AcknowledgeScanout { sequence } => {
            core.acknowledge_scanout(sequence).map(|()| Response::Unit)
        }
        Request::FailFrame => {
            core.fail_frame();
            Ok(Response::Unit)
        }
        Request::Pointer { x, y } => Ok(Response::Pointer(core.pointer(x, y))),
        Request::Keyboard => Ok(Response::Keyboard(core.keyboard())),
        Request::Focused => Ok(Response::Focused(core.focused_surface())),
        Request::Initialize { .. } => Err(Error::Order),
    }
}

fn window(offset: u32, pixels: u32) -> Result<(usize, usize), Error> {
    let offset = offset as usize;
    let count = pixels as usize;
    if offset % 4 != 0
        || count == 0
        || count > conduitos_compositor_core::MAX_CHUNK
        || count
            .checked_mul(4)
            .and_then(|bytes| offset.checked_add(bytes))
            .is_none_or(|end| end > memory::OUTPUT_BYTES)
    {
        return Err(Error::Bounds);
    }
    Ok((memory::OUTPUT + offset, count))
}

fn finish(status: u32) -> ! {
    // Existing x86 service transition recognizes eax=0 as terminal yield and any
    // unknown nonzero code as InvalidGate, causing quarantine; no effect gate.
    unsafe {
        core::arch::asm!("int 0x80", in("eax") status, options(noreturn));
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    finish(3)
}
