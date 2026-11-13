//! Bounded VideoCore exchange using a sole cache-line-owned staging buffer.
//! GPU writes never invalidate neighbouring live values on Root's stack.
use super::{read_counter, read32, write32};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering, compiler_fence},
};

const BASE: usize = super::PERIPHERAL_BASE + 0xb880;
const WORDS: usize = 64;
#[repr(C, align(64))]
struct Staging(UnsafeCell<[u32; WORDS]>);
// A single retained request owns every byte of these complete cache lines.
unsafe impl Sync for Staging {}
static STAGING: Staging = Staging(UnsafeCell::new([0; WORDS]));
static BUSY: AtomicBool = AtomicBool::new(false);
const _: () = assert!(core::mem::size_of::<Staging>() == 256);
struct Ownership {
    sent: bool,
    retired: bool,
}
impl Drop for Ownership {
    fn drop(&mut self) {
        if !self.sent || self.retired {
            BUSY.store(false, Ordering::Release);
        }
    }
}

pub(super) fn call(words: &mut [u32]) -> Result<(), &'static str> {
    if words.as_ptr() as usize & 15 != 0
        || words.len() < 3
        || words.len() > WORDS
        || words[0] as usize != words.len() * 4
    {
        return Err("mailbox-buffer-invalid");
    }
    if BUSY.swap(true, Ordering::AcqRel) {
        return Err("mailbox-already-owned");
    }
    let mut ownership = Ownership {
        sent: false,
        retired: false,
    };
    let buffer = STAGING.0.get().cast::<u32>();
    let address = buffer as usize;
    if address >= 0x4000_0000 {
        return Err("mailbox-buffer-invalid");
    }
    // Volatile initialization of the entire object prevents unused padding or
    // stack-slot reuse from sharing a cache line with a firmware response.
    for index in 0..WORDS {
        unsafe {
            buffer
                .add(index)
                .write_volatile(words.get(index).copied().unwrap_or(0));
        }
    }
    let request = address as u32 | 0x4000_0008;
    let start = read_counter() as u32;
    compiler_fence(Ordering::SeqCst);
    super::prepare_mailbox(address, WORDS * 4);
    let mut sent = false;
    for _ in 0..4_000_000 {
        if (read_counter() as u32).wrapping_sub(start) > 2_000_000 {
            break;
        }
        if !sent {
            if unsafe { read32(BASE + 0x18) } & (1 << 31) == 0 {
                // A sent request retains this storage on timeout: the GPU may
                // still write it. Only the matching acknowledgement retires it.
                ownership.sent = true;
                unsafe {
                    write32(BASE + 0x20, request);
                }
                sent = true;
            }
        } else if unsafe { read32(BASE + 0x18) } & (1 << 30) == 0 {
            if unsafe { read32(BASE) } != request {
                continue;
            }
            ownership.retired = true;
            super::drain_writes();
            super::invalidate_mailbox(address, WORDS * 4);
            compiler_fence(Ordering::SeqCst);
            for (index, word) in words.iter_mut().enumerate() {
                *word = unsafe { buffer.add(index).read_volatile() };
            }
            return if words[1] == 0x8000_0000 {
                Ok(())
            } else {
                Err("mailbox-response-refused")
            };
        }
    }
    Err("mailbox-timeout")
}
