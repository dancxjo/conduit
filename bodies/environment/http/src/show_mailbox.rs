//! Finite custody for already resolved HTTP Show material. This is neither a
//! Face owner nor a source of Body, route, clock or observation authority.
use crate::PreparedHttpShow;
use core::cell::RefCell;
use critical_section::Mutex;

struct Lease {
    show: PreparedHttpShow,
    from_ms: u64,
    until_ms: u64,
}
struct State {
    claimed: bool,
    withdrawn: bool,
    current: Option<Lease>,
}
pub struct HttpShowMailbox {
    state: Mutex<RefCell<State>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpShowRefusal {
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
pub struct HttpShowPublisher<'a> {
    mailbox: &'a HttpShowMailbox,
    remaining: u32,
    maximum_lifetime_ms: u64,
}
/// Separate Root withdrawal custody; closing cannot admit another producer.
pub struct HttpShowControl<'a> {
    mailbox: &'a HttpShowMailbox,
}
impl Default for HttpShowMailbox {
    fn default() -> Self {
        Self::new()
    }
}
impl HttpShowMailbox {
    pub const fn new() -> Self {
        Self {
            state: Mutex::new(RefCell::new(State {
                claimed: false,
                withdrawn: false,
                current: None,
            })),
        }
    }
    pub fn claim(
        &self,
        maximum_publications: u32,
        maximum_lifetime_ms: u64,
    ) -> Result<(HttpShowPublisher<'_>, HttpShowControl<'_>), HttpShowRefusal> {
        if maximum_publications == 0 || maximum_lifetime_ms == 0 {
            return Err(HttpShowRefusal::Budget);
        }
        critical_section::with(|cs| {
            let mut state = self
                .state
                .borrow(cs)
                .try_borrow_mut()
                .map_err(|_| HttpShowRefusal::Busy)?;
            if state.claimed {
                return Err(HttpShowRefusal::AlreadyClaimed);
            }
            state.claimed = true;
            Ok((
                HttpShowPublisher {
                    mailbox: self,
                    remaining: maximum_publications,
                    maximum_lifetime_ms,
                },
                HttpShowControl { mailbox: self },
            ))
        })
    }
    /// Copy an owned bounded response before socket awaits. Missing, withdrawn,
    /// not-yet-valid and expired material all remain unavailable. Expiry does
    /// not rewrite freshness or manufacture a new semantic Face revision.
    pub fn response(
        &self,
        ap_ready: bool,
        now_ms: u64,
        json: bool,
    ) -> Result<Option<heapless::String<2048>>, HttpShowRefusal> {
        critical_section::with(|cs| {
            let state = self
                .state
                .borrow(cs)
                .try_borrow()
                .map_err(|_| HttpShowRefusal::Busy)?;
            if state.withdrawn {
                return Ok(None);
            }
            let Some(lease) = &state.current else {
                return Ok(None);
            };
            if !(lease.from_ms..lease.until_ms).contains(&now_ms) {
                return Ok(None);
            }
            Ok(Some(lease.show.response(ap_ready, now_ms, json)))
        })
    }
    fn withdraw(&self) {
        critical_section::with(|cs| {
            // The private response formatter invokes no external callbacks, so
            // no publication/control owner can reenter this synchronous borrow.
            let mut state = self.state.borrow(cs).borrow_mut();
            state.withdrawn = true;
            state.current = None;
        });
    }
}
impl HttpShowPublisher<'_> {
    /// Lease coordinates come from Root's admitted same-Boot policy. They are
    /// not inferred from formatted temperatures or substituted for sample time.
    /// Refusal preserves the previous material and publication budget.
    pub fn publish(
        &mut self,
        show: PreparedHttpShow,
        from_ms: u64,
        until_ms: u64,
    ) -> Result<(), HttpShowRefusal> {
        let lifetime = until_ms
            .checked_sub(from_ms)
            .ok_or(HttpShowRefusal::Lifetime)?;
        if lifetime == 0 || lifetime > self.maximum_lifetime_ms {
            return Err(HttpShowRefusal::Lifetime);
        }
        critical_section::with(|cs| {
            let mut state = self
                .mailbox
                .state
                .borrow(cs)
                .try_borrow_mut()
                .map_err(|_| HttpShowRefusal::Busy)?;
            if state.withdrawn {
                return Err(HttpShowRefusal::Withdrawn);
            }
            if self.remaining == 0 {
                return Err(HttpShowRefusal::Exhausted);
            }
            if state.current.as_ref().is_some_and(|old| {
                show.face_revision() <= old.show.face_revision() || from_ms < old.from_ms
            }) {
                return Err(HttpShowRefusal::Stale);
            }
            state.current = Some(Lease {
                show,
                from_ms,
                until_ms,
            });
            self.remaining -= 1;
            Ok(())
        })
    }
}
impl Drop for HttpShowPublisher<'_> {
    fn drop(&mut self) {
        self.mailbox.withdraw();
    }
}
impl HttpShowControl<'_> {
    pub fn withdraw(&self) {
        self.mailbox.withdraw();
    }
}

#[cfg(test)]
#[path = "show_mailbox_tests.rs"]
mod tests;
