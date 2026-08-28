use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::machine::BaseError;

use super::{TIMER_IRQ_VECTOR, pic};

const FACT_CAPACITY: u32 = 4;
static HEAD: AtomicU32 = AtomicU32::new(0);
static TAIL: AtomicU32 = AtomicU32::new(0);
static ENTRIES: [AtomicU32; FACT_CAPACITY as usize] =
    [const { AtomicU32::new(0) }; FACT_CAPACITY as usize];
static OVERFLOWED: AtomicBool = AtomicBool::new(false);

#[unsafe(no_mangle)]
extern "C" fn conduitos_timer_irq_handler() {
    let tail = TAIL.load(Ordering::Relaxed);
    let head = HEAD.load(Ordering::Acquire);
    if tail.wrapping_sub(head) == FACT_CAPACITY {
        OVERFLOWED.store(true, Ordering::Release);
    } else {
        let index = (tail % FACT_CAPACITY) as usize;
        ENTRIES[index].store(u32::from(TIMER_IRQ_VECTOR), Ordering::Relaxed);
        TAIL.store(tail.wrapping_add(1), Ordering::Release);
    }
    pic::end_timer_interrupt();
}

pub(super) fn pop() -> Result<Option<u8>, BaseError> {
    if OVERFLOWED.swap(false, Ordering::AcqRel) {
        return Err(BaseError::RingFull);
    }
    let head = HEAD.load(Ordering::Relaxed);
    let tail = TAIL.load(Ordering::Acquire);
    if head == tail {
        return Ok(None);
    }
    let index = (head % FACT_CAPACITY) as usize;
    let vector = ENTRIES[index].load(Ordering::Relaxed) as u8;
    HEAD.store(head.wrapping_add(1), Ordering::Release);
    Ok(Some(vector))
}
