//! Independently linked canonical native/text schedulers; no Root device grant.
#![no_std]
#![no_main]
#[path = "../domain/memory.rs"]
mod compiler_memory;
use conduitos_runtime_core::{
    memory,
    runtime::Runtime as State,
    wire::{self, Envelope, Response},
};
use core::{
    mem::{MaybeUninit, align_of, size_of},
    ptr, slice,
};
const MAGIC: u64 = 0x5255_4e54_494d_3031;
#[repr(C)]
struct Retained {
    magic: u64,
    instance: u64,
    request: u64,
    state: MaybeUninit<State>,
}
// The enum retains exactly one canonical scheduler; includes all value/Sign storage.
const _: () = assert!(size_of::<Retained>() <= memory::METADATA_BYTES - 4096);
const _: () = assert!(size_of::<conduitos_runtime_core::text::Scheduler>() <= size_of::<State>());
const _: () = assert!(memory::STATE.is_multiple_of(align_of::<Retained>()));
/// # Safety
/// Root installs the exact finite Runtime profile and passes its fixed exchange
/// grant. Every entry has a fresh stack and exclusive access to retained metadata.
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
pub unsafe extern "C" fn runtime_entry(exchange: *mut u8) -> ! {
    if exchange as usize != memory::EXCHANGE {
        finish(3);
    }
    if unsafe { dispatch() }.is_err() {
        finish(3);
    }
    finish(0)
}
unsafe fn dispatch() -> Result<(), ()> {
    let exchange =
        unsafe { slice::from_raw_parts_mut(memory::EXCHANGE as *mut u8, wire::EXCHANGE_BYTES) };
    let length = u32::from_le_bytes(exchange[8..12].try_into().map_err(|_| ())?) as usize;
    let (envelope, request) =
        wire::decode_request(exchange.get(..length).ok_or(())?).map_err(|_| ())?;
    let state = memory::STATE as *mut Retained;
    if unsafe { ptr::addr_of!((*state).magic).read() } == 0 {
        if envelope.request != 1 {
            return reply(
                exchange,
                envelope,
                Response::Failed(conduit_kernel::scheduler::SchedulerError::InvalidPlan),
                None,
            );
        }
        unsafe {
            ptr::addr_of_mut!((*state).state).write(MaybeUninit::new(State::default()));
            ptr::addr_of_mut!((*state).instance).write(envelope.instance);
            ptr::addr_of_mut!((*state).request).write(0);
            ptr::addr_of_mut!((*state).magic).write(MAGIC);
        }
    }
    let retained = unsafe { &mut *state };
    if retained.magic != MAGIC {
        return Err(());
    }
    if retained.instance != envelope.instance {
        return reply(
            exchange,
            envelope,
            Response::Failed(conduit_kernel::scheduler::SchedulerError::InvalidPlan),
            None,
        );
    }
    if retained.request.checked_add(1) != Some(envelope.request) {
        return reply(
            exchange,
            envelope,
            Response::Failed(conduit_kernel::scheduler::SchedulerError::InvalidPlan),
            None,
        );
    }
    retained.request = envelope.request;
    let transfer =
        unsafe { slice::from_raw_parts_mut(memory::TRANSFER as *mut u8, memory::TRANSFER_BYTES) };
    let state = unsafe { retained.state.assume_init_mut() };
    let response = state
        .execute(request, transfer)
        .unwrap_or_else(Response::Failed);
    reply(exchange, envelope, response, state.gap())
}
fn reply(
    exchange: &mut [u8],
    envelope: Envelope,
    response: Response,
    gap: Option<conduit_kernel::SignRetentionGap>,
) -> Result<(), ()> {
    wire::encode_response(envelope, response, gap, exchange).map_err(|_| ())?;
    Ok(())
}
fn finish(status: u32) -> ! {
    unsafe {
        core::arch::asm!("int 0x80",in("eax")status,options(noreturn));
    }
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    finish(3)
}
