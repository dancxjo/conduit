//! Dedicated finite native execution domain; no Conduit scheduler or Gear adapter.
mod linux;
mod ort_api;
use super::{
    protocol::{self, Bootstrap, Prepared},
    BackingRole, BackingSpan, NativeBackingRefusal,
};
pub(super) use linux::Snapshot;
pub(super) use ort_api::file_digest;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
pub fn worker_main() -> Result<(), NativeBackingRefusal> {
    unsafe {
        libc::mallopt(libc::M_ARENA_MAX, 1);
        libc::mallopt(libc::M_MMAP_MAX, 0);
        libc::mallopt(libc::M_TRIM_THRESHOLD, -1);
    }
    let bootstrap: Bootstrap = protocol::receive(&mut std::io::stdin())?;
    bootstrap.profile.validate()?;
    bootstrap.grant.validate()?;
    if bootstrap.graph.len() != 77
        || Sha256::digest(&bootstrap.graph).as_slice() != super::profile::MODEL_SHA256
    {
        return Err(NativeBackingRefusal::IdentityMismatch);
    }
    let original = 0u8;
    let snapshot = Snapshot::capture(std::process::id())?;
    let (start, end) = snapshot.stack_at((&original as *const u8) as u64)?;
    drop(snapshot);
    let limit = libc::rlimit {
        rlim_cur: end - start,
        rlim_max: end - start,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_STACK, &limit) } != 0 {
        return Err(NativeBackingRefusal::BackingPreparation);
    }
    let stack_bytes = super::profile::STACK_BYTES;
    let guard = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            stack_bytes + 8192,
            libc::PROT_NONE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if guard == libc::MAP_FAILED {
        return Err(NativeBackingRefusal::BackingPreparation);
    }
    let stack = unsafe { guard.cast::<u8>().add(4096) };
    if unsafe {
        libc::mprotect(
            stack.cast(),
            stack_bytes,
            libc::PROT_READ | libc::PROT_WRITE,
        )
    } != 0
    {
        return Err(NativeBackingRefusal::BackingPreparation);
    }
    unsafe { std::ptr::write_bytes(stack, 0, stack_bytes) };
    let context = Box::new(WorkerContext {
        bootstrap,
        stack: BackingSpan {
            start: stack as u64,
            end: stack as u64 + stack_bytes as u64,
            role: BackingRole::ExecutionStack,
        },
    });
    let context = Box::into_raw(context);
    // One process, one thread. The context is moved to a fixed guarded stack and
    // never returns onto the original startup stack.
    unsafe {
        CONTEXT = context;
    }
    let mut machine = unsafe { std::mem::zeroed::<libc::ucontext_t>() };
    if unsafe { libc::getcontext(&mut machine) } != 0 {
        return Err(NativeBackingRefusal::BackingPreparation);
    }
    machine.uc_stack.ss_sp = stack.cast();
    machine.uc_stack.ss_size = stack_bytes;
    machine.uc_link = std::ptr::null_mut();
    unsafe {
        libc::makecontext(&mut machine, entry, 0);
        libc::setcontext(&machine);
    }
    Err(NativeBackingRefusal::BackingPreparation)
}
struct WorkerContext {
    bootstrap: Bootstrap,
    stack: BackingSpan,
}
static mut FROZEN: bool = false;
static mut CONTEXT: *mut WorkerContext = std::ptr::null_mut();
extern "C" fn entry() {
    let context = unsafe { &mut *CONTEXT };
    match prepare_and_run(context) {
        Ok(()) => unsafe { libc::_exit(0) },
        Err(error) => {
            if unsafe { FROZEN } {
                unsafe { libc::_exit(6) }
            };
            let _ = protocol::send(&mut std::io::stdout(), &Prepared::Refused(error));
            unsafe { libc::_exit(1) }
        }
    }
}
fn prepare_and_run(context: &WorkerContext) -> Result<(), NativeBackingRefusal> {
    if std::fs::read_dir("/proc/self/task")
        .map_err(|_| NativeBackingRefusal::Io)?
        .count()
        != 1
    {
        return Err(NativeBackingRefusal::UnsupportedProfile);
    }
    let io = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            4096,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if io == libc::MAP_FAILED {
        return Err(NativeBackingRefusal::BackingPreparation);
    }
    let io_span = BackingSpan {
        start: io as u64,
        end: io as u64 + 4096,
        role: BackingRole::FixedIo,
    };
    let mut native = unsafe {
        ort_api::NativeSession::prepare(
            &context.bootstrap.profile,
            &context.bootstrap.graph,
            io.cast(),
        )
    }?;
    if native.infer(1.25)?.to_bits() != 1.25f32.to_bits() {
        return Err(NativeBackingRefusal::InvalidOutput);
    }
    let mut seed = Vec::new();
    seed.try_reserve_exact(super::profile::HEAP_SEED_BYTES)
        .map_err(|_| NativeBackingRefusal::BackingPreparation)?;
    seed.resize(super::profile::HEAP_SEED_BYTES, 0u8);
    drop(seed);
    let snapshot = Snapshot::capture(std::process::id())?;
    let ledger = snapshot.ledger(io_span, context.stack)?;
    ledger.validate(context.bootstrap.grant)?;
    unsafe { snapshot.prefault() };
    // Prepare serialization before freeze; fixed execution buffers are stack-owned.
    let ready =
        serde_json::to_vec(&Prepared::Ready(ledger)).map_err(|_| NativeBackingRefusal::Protocol)?;
    if ready.len() > protocol::MAX_FRAME {
        return Err(NativeBackingRefusal::Protocol);
    }
    linux::freeze()?;
    unsafe {
        FROZEN = true;
    }
    let mut output = std::io::stdout();
    output
        .write_all(&(ready.len() as u32).to_le_bytes())
        .and_then(|()| output.write_all(&ready))
        .and_then(|()| output.flush())
        .map_err(|_| NativeBackingRefusal::Io)?;
    let mut input = std::io::stdin();
    let mut ack = [0u8; 1];
    input
        .read_exact(&mut ack)
        .map_err(|_| NativeBackingRefusal::Io)?;
    if ack != [b'G'] {
        unsafe { libc::_exit(2) }
    }
    for sequence in 1..=context.bootstrap.profile.maximum_calls {
        let mut request = [0u8; 9];
        input
            .read_exact(&mut request)
            .map_err(|_| NativeBackingRefusal::Io)?;
        if request[0] == 0 {
            unsafe { libc::_exit(0) }
        }
        if request[0] == 2 {
            let bytes = (ledger_private_capacity(&snapshot)? as usize)
                .checked_mul(2)
                .ok_or(NativeBackingRefusal::Capacity)?;
            let p = unsafe { libc::malloc(bytes) };
            if p.is_null() {
                unsafe { libc::_exit(7) }
            };
            unsafe {
                std::ptr::write_volatile(p.cast::<u8>(), 1);
                libc::_exit(8)
            }
        }
        if request[0] != 1
            || u32::from_le_bytes(request[1..5].try_into().unwrap()) != u32::from(sequence)
        {
            unsafe { libc::_exit(3) }
        }
        let value = f32::from_bits(u32::from_le_bytes(request[5..].try_into().unwrap()));
        if !value.is_finite() {
            unsafe { libc::_exit(4) }
        }
        let result = native.infer(value)?;
        let mut reply = [0u8; 8];
        reply[..4].copy_from_slice(&u32::from(sequence).to_le_bytes());
        reply[4..].copy_from_slice(&result.to_bits().to_le_bytes());
        output
            .write_all(&reply)
            .and_then(|()| output.flush())
            .map_err(|_| NativeBackingRefusal::Io)?;
        input
            .read_exact(&mut ack)
            .map_err(|_| NativeBackingRefusal::Io)?;
        if ack != [b'C'] {
            unsafe { libc::_exit(5) }
        }
    }
    unsafe { libc::_exit(0) }
}

fn ledger_private_capacity(snapshot: &Snapshot) -> Result<u64, NativeBackingRefusal> {
    snapshot.private_capacity()
}
