//! Finite custody for already resolved Show material. This is neither a
//! Face owner nor a source of Body, route, clock or observation authority.
use core::cell::{Cell, RefCell};
use critical_section::Mutex;

struct Lease<T> {
    show: T,
    revision: u64,
    from_ms: u64,
    until_ms: u64,
}
struct State<T> {
    claimed: bool,
    current: Option<Lease<T>>,
}
/// Owns inert bounded Show material, never source evidence or effect authority.
/// The caller separately admits material storage and synchronous callback work.
pub struct ShowMailbox<T> {
    state: Mutex<RefCell<State<T>>>,
    withdrawn: Mutex<Cell<bool>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowCustodyRefusal {
    Budget,
    AlreadyClaimed,
    Busy,
    Withdrawn,
    Exhausted,
    Lifetime,
    Stale,
}
/// Root retains the sole producer and independently admits its exact Face basis,
/// publication work and lease policy. No ID can recreate this private custody.
pub struct ShowPublisher<'a, T> {
    mailbox: &'a ShowMailbox<T>,
    remaining: u32,
    maximum_lifetime_ms: u64,
}
/// Separate Root withdrawal custody; closing cannot admit another producer.
pub struct ShowControl<'a, T> {
    mailbox: &'a ShowMailbox<T>,
}
impl<T> Default for ShowMailbox<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> ShowMailbox<T> {
    pub const fn new() -> Self {
        Self {
            state: Mutex::new(RefCell::new(State {
                claimed: false,
                current: None,
            })),
            withdrawn: Mutex::new(Cell::new(false)),
        }
    }
    pub fn claim(
        &self,
        maximum_publications: u32,
        maximum_lifetime_ms: u64,
    ) -> Result<(ShowPublisher<'_, T>, ShowControl<'_, T>), ShowCustodyRefusal> {
        if maximum_publications == 0 || maximum_lifetime_ms == 0 {
            return Err(ShowCustodyRefusal::Budget);
        }
        critical_section::with(|cs| {
            let mut state = self
                .state
                .borrow(cs)
                .try_borrow_mut()
                .map_err(|_| ShowCustodyRefusal::Busy)?;
            if state.claimed {
                return Err(ShowCustodyRefusal::AlreadyClaimed);
            }
            state.claimed = true;
            Ok((
                ShowPublisher {
                    mailbox: self,
                    remaining: maximum_publications,
                    maximum_lifetime_ms,
                },
                ShowControl { mailbox: self },
            ))
        })
    }
    /// Consume one exact leased value synchronously. The caller admits finite
    /// callback work and owned output before publication; no borrow may escape.
    /// Reentrant withdrawal suppresses the result. The outermost read retires
    /// retained material after its borrow ends.
    /// ```compile_fail
    /// use conduit_face::ShowMailbox;
    /// fn leak<'a>(mailbox: &'a ShowMailbox<u8>) -> &'a u8 {
    ///     mailbox.with_current(0, |value| value).unwrap().unwrap()
    /// }
    /// ```
    pub fn with_current<R>(
        &self,
        now_ms: u64,
        consume: impl FnOnce(&T) -> R,
    ) -> Result<Option<R>, ShowCustodyRefusal> {
        critical_section::with(|cs| {
            let state = self
                .state
                .borrow(cs)
                .try_borrow()
                .map_err(|_| ShowCustodyRefusal::Busy)?;
            if self.withdrawn.borrow(cs).get() {
                return Ok(None);
            }
            let Some(lease) = &state.current else {
                return Ok(None);
            };
            if !(lease.from_ms..lease.until_ms).contains(&now_ms) {
                return Ok(None);
            }
            let result = consume(&lease.show);
            drop(state);
            if self.withdrawn.borrow(cs).get() {
                if let Ok(mut state) = self.state.borrow(cs).try_borrow_mut() {
                    state.current = None;
                }
                Ok(None)
            } else {
                Ok(Some(result))
            }
        })
    }
    fn withdraw(&self) {
        critical_section::with(|cs| {
            // The independent flag fences reentrant withdrawal even while a
            // reader holds the material. That read retires it before returning.
            self.withdrawn.borrow(cs).set(true);
            if let Ok(mut state) = self.state.borrow(cs).try_borrow_mut() {
                state.current = None;
            }
        });
    }
}
impl<T> ShowPublisher<'_, T> {
    /// Lease coordinates come from Root's admitted same-Boot policy. They are
    /// not inferred from formatted temperatures or substituted for sample time.
    /// Refusal preserves the previous material and publication budget.
    pub fn publish(
        &mut self,
        show: T,
        revision: u64,
        from_ms: u64,
        until_ms: u64,
    ) -> Result<(), ShowCustodyRefusal> {
        let lifetime = until_ms
            .checked_sub(from_ms)
            .ok_or(ShowCustodyRefusal::Lifetime)?;
        if lifetime == 0 || lifetime > self.maximum_lifetime_ms {
            return Err(ShowCustodyRefusal::Lifetime);
        }
        critical_section::with(|cs| {
            let mut state = self
                .mailbox
                .state
                .borrow(cs)
                .try_borrow_mut()
                .map_err(|_| ShowCustodyRefusal::Busy)?;
            if self.mailbox.withdrawn.borrow(cs).get() {
                return Err(ShowCustodyRefusal::Withdrawn);
            }
            if self.remaining == 0 {
                return Err(ShowCustodyRefusal::Exhausted);
            }
            if state
                .current
                .as_ref()
                .is_some_and(|old| revision <= old.revision || from_ms < old.from_ms)
            {
                return Err(ShowCustodyRefusal::Stale);
            }
            state.current = Some(Lease {
                show,
                revision,
                from_ms,
                until_ms,
            });
            self.remaining -= 1;
            Ok(())
        })
    }
}
impl<T> Drop for ShowPublisher<'_, T> {
    fn drop(&mut self) {
        self.mailbox.withdraw();
    }
}
impl<T> ShowControl<'_, T> {
    pub fn withdraw(&self) {
        self.mailbox.withdraw();
    }
}
