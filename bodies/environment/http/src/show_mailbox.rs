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
    generation: u64,
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
    Scope,
    GenerationExhausted,
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
    generation: u64,
    remaining: u32,
    maximum_lifetime_ms: u64,
}
/// Separate Root withdrawal custody; closing cannot admit another producer.
pub struct HttpShowControl<'a> {
    mailbox: &'a HttpShowMailbox,
    generation: u64,
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
                generation: 0,
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
                    generation: state.generation,
                    remaining: maximum_publications,
                    maximum_lifetime_ms,
                },
                HttpShowControl {
                    mailbox: self,
                    generation: state.generation,
                },
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
    fn withdraw(&self, generation: u64) {
        critical_section::with(|cs| {
            // The private response formatter invokes no external callbacks, so
            // no publication/control owner can reenter this synchronous borrow.
            let mut state = self.state.borrow(cs).borrow_mut();
            if generation != state.generation {
                return;
            }
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
            if self.generation != state.generation {
                return Err(HttpShowRefusal::Scope);
            }
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
        self.mailbox.withdraw(self.generation);
    }
}
impl HttpShowControl<'_> {
    pub fn withdraw(&self) {
        self.mailbox.withdraw(self.generation);
    }
}

#[cfg(test)]
#[path = "show_mailbox_tests.rs"]
mod tests;

/// Affine returned mailbox custody, without a replacement publication budget.
pub struct Returned<'a> {
    mailbox: &'a HttpShowMailbox,
    generation: u64,
}
impl<'a> HttpShowMailbox {
    #[allow(clippy::type_complexity)]
    pub fn reunite(
        &'a self,
        publisher: HttpShowPublisher<'a>,
        control: HttpShowControl<'a>,
    ) -> Result<Returned<'a>, (HttpShowPublisher<'a>, HttpShowControl<'a>, HttpShowRefusal)> {
        let result = critical_section::with(|cs| {
            let mut state = self
                .state
                .borrow(cs)
                .try_borrow_mut()
                .map_err(|_| HttpShowRefusal::Busy)?;
            if !core::ptr::eq(self, publisher.mailbox)
                || !core::ptr::eq(self, control.mailbox)
                || publisher.generation != state.generation
                || control.generation != state.generation
            {
                return Err(HttpShowRefusal::Scope);
            }
            let next = state
                .generation
                .checked_add(1)
                .ok_or(HttpShowRefusal::GenerationExhausted)?;
            state.current = None;
            state.withdrawn = true;
            state.generation = next;
            Ok(next)
        });
        match result {
            Ok(generation) => Ok(Returned {
                mailbox: self,
                generation,
            }),
            Err(error) => Err((publisher, control, error)),
        }
    }
}
impl<'a> Returned<'a> {
    #[allow(clippy::type_complexity)]
    pub fn replace(
        self,
        maximum_publications: u32,
        maximum_lifetime_ms: u64,
    ) -> Result<(HttpShowPublisher<'a>, HttpShowControl<'a>), (Self, HttpShowRefusal)> {
        if maximum_publications == 0 || maximum_lifetime_ms == 0 {
            return Err((self, HttpShowRefusal::Budget));
        }
        let result = critical_section::with(|cs| {
            let mut state = self
                .mailbox
                .state
                .borrow(cs)
                .try_borrow_mut()
                .map_err(|_| HttpShowRefusal::Busy)?;
            if state.generation != self.generation || !state.withdrawn || state.current.is_some() {
                return Err(HttpShowRefusal::Scope);
            }
            state.withdrawn = false;
            Ok(())
        });
        if let Err(error) = result {
            return Err((self, error));
        }
        Ok((
            HttpShowPublisher {
                mailbox: self.mailbox,
                generation: self.generation,
                remaining: maximum_publications,
                maximum_lifetime_ms,
            },
            HttpShowControl {
                mailbox: self.mailbox,
                generation: self.generation,
            },
        ))
    }
}
