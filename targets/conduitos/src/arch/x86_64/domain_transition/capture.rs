//! Scoped one-entry Root sink. Raw capture is not a resume permit: the actual
//! owner must validate/capture it into its exact continuation custody first.
use super::continuation::{UserContext, UserSegments};
use core::sync::atomic::{AtomicPtr, Ordering};
static TARGET: AtomicPtr<Option<UserContext>> = AtomicPtr::new(core::ptr::null_mut());
// Keep the slot occupied after its one capture until its owning Sink drops.
// This tag is never dereferenced; a target has UserContext's 16-byte alignment.
const CONSUMED: *mut Option<UserContext> = core::ptr::without_provenance_mut(1);
pub(super) struct Sink<'a> {
    _borrow: core::marker::PhantomData<(&'a mut Option<UserContext>, *mut ())>,
}
impl<'a> Sink<'a> {
    pub(super) fn install(target: &'a mut Option<UserContext>) -> Option<Self> {
        TARGET
            .compare_exchange(
                core::ptr::null_mut(),
                target,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .ok()?;
        *target = None;
        Some(Self {
            _borrow: core::marker::PhantomData,
        })
    }
}
impl Drop for Sink<'_> {
    fn drop(&mut self) {
        TARGET.store(core::ptr::null_mut(), Ordering::Release);
    }
}
/// Called only by the real IRQ entry with complete pre-normalization snapshots.
/// The single active entry and disabled interrupts fence both pointed ranges.
pub(super) unsafe fn save(words: *const u64, segments: *const UserSegments) -> bool {
    let target = TARGET.load(Ordering::Acquire);
    if target.is_null() || target == CONSUMED {
        return false;
    }
    if TARGET
        .compare_exchange(target, CONSUMED, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return false;
    }
    if words.is_null() || segments.is_null() {
        return false;
    }
    // No guest-selected pointers: entry_return.S supplies its own stack spills.
    let words = unsafe { &*words.cast::<[u64; 21]>() };
    let segments = unsafe { segments.read() };
    match UserContext::from_source_irq(words, segments) {
        Ok(context) => {
            unsafe {
                target.write(Some(context));
            }
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sink_is_single_entry_and_cleared_on_drop_and_capture() {
        let mut first = None;
        let mut second = None;
        let sink = Sink::install(&mut first).unwrap();
        assert!(Sink::install(&mut second).is_none());
        let mut words = [0; 21];
        words[16] = 0x4000;
        words[17] = 0x2b;
        words[18] = 0x202;
        words[19] = 0x9000;
        words[20] = 0x33;
        let segments = UserSegments {
            ds: 0x33,
            es: 0x33,
            fs: 0,
            gs: 0,
            fs_base: 0,
            gs_base: 0,
        };
        assert!(unsafe { save(words.as_ptr(), &segments) });
        assert!(!unsafe { save(words.as_ptr(), &segments) });
        assert!(Sink::install(&mut second).is_none());
        drop(sink);
        let observed = first.unwrap();
        assert_eq!(observed.returned.rip, 0x4000);
        assert_eq!(observed.segments, segments);
        let sink = Sink::install(&mut second).unwrap();
        drop(sink);
        assert!(!unsafe { save(words.as_ptr(), &segments) });
        assert!(second.is_none());
    }
}
