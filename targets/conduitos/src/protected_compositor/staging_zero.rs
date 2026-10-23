//! Proof-only one-shot observation of initialized staging, before actual release.
//! The production volatile wipe is independent of this observer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Witness {
    pub instance: u64,
    pub release_ordinal: u64,
    pub initialized_words: usize,
    pub checked_words: usize,
    pub capacity_words: usize,
    pub nonzero_words: usize,
}
#[derive(Clone, Copy)]
struct Arm {
    instance: u64,
    release_ordinal: u64,
}
pub(crate) struct Observer {
    armed: Option<Arm>,
    completed: Option<Witness>,
}
impl Observer {
    pub const fn new() -> Self {
        Self {
            armed: None,
            completed: None,
        }
    }
    pub fn arm(&mut self, instance: u64, release_ordinal: u64) -> bool {
        if instance == 0 || release_ordinal == 0 || self.armed.is_some() || self.completed.is_some()
        {
            return false;
        }
        self.armed = Some(Arm {
            instance,
            release_ordinal,
        });
        true
    }
    pub fn observe_before_free(&mut self, initialized: &[u32], capacity_words: usize) {
        let Some(arm) = self.armed.take() else {
            return;
        };
        let mut checked_words = 0;
        let mut nonzero_words = 0;
        // Scan every initialized element, including suffix outside a raster.
        // Capacity metadata is recorded; spare capacity is never dereferenced.
        for word in initialized {
            // SAFETY: aligned initialized words remain borrowed before free.
            let value = unsafe { core::ptr::read_volatile(word) };
            checked_words += 1;
            nonzero_words += usize::from(value != 0);
        }
        self.completed = Some(Witness {
            instance: arm.instance,
            release_ordinal: arm.release_ordinal,
            initialized_words: initialized.len(),
            checked_words,
            capacity_words,
            nonzero_words,
        });
    }
    pub fn take(&mut self) -> Option<Witness> {
        self.completed.take()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checks_full_initialized_extent_and_consumes_one_release() {
        let mut observer = Observer::new();
        assert!(!observer.arm(0, 1));
        assert!(!observer.arm(7, 0));
        assert!(observer.arm(7, 1));
        assert!(!observer.arm(7, 2));
        observer.observe_before_free(&[0, 5, 0, 9], 16);
        assert!(!observer.arm(7, 2));
        assert_eq!(
            observer.take(),
            Some(Witness {
                instance: 7,
                release_ordinal: 1,
                initialized_words: 4,
                checked_words: 4,
                capacity_words: 16,
                nonzero_words: 2,
            })
        );
        observer.observe_before_free(&[99], 1);
        assert_eq!(observer.take(), None);
        assert!(observer.arm(8, 2));
        observer.observe_before_free(&[0; 9], 9);
        assert_eq!(observer.take().unwrap().nonzero_words, 0);
        assert_eq!(observer.take(), None);
    }
}
