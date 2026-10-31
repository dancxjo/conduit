//! Untrusted proof mailbox; only the owning Root pump can admit or sign. No
//! private invitation/continuity secret enters this HTTP state or its responses.
#[path = "native_body_join_cache.rs"]
mod cache;
use alloc::vec::Vec;
use core::cell::RefCell;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
use sha2::{Digest, Sha256};
struct Pending {
    ticket: u32,
    digest: [u8; 32],
    bytes: Vec<u8>,
}
struct Completed {
    ticket: u32,
    digest: [u8; 32],
    bytes: Vec<u8>,
    // Only a typed Root-issued challenge supplies a deadline. Durable admission
    // and return receipts have no transport-cache expiry.
    deadline: Option<u64>,
}
struct State {
    enabled: bool,
    next: u32,
    pending: Option<Pending>,
    inflight: Option<(u32, [u8; 32])>,
    completed: Option<Completed>,
    // One fixed static code reference; no diagnostic payload allocation.
    refused: Option<(u32, &'static str)>,
}
// Awake CPU-only mailbox metadata; all transport payloads remain heap-owned.
#[allow(unexpected_cfgs)] // Native profile is absent from the CPU proof crate.
#[cfg_attr(feature = "local-appliance", esp_hal::ram(unstable(rtc_slow)))]
static STATE: Mutex<CriticalSectionRawMutex, RefCell<State>> = Mutex::new(RefCell::new(State {
    enabled: false,
    next: 0,
    pending: None,
    inflight: None,
    completed: None,
    refused: None,
}));
/// Scalar retained receipt allocation telemetry; never receipt contents.
pub(crate) fn completed_storage_usage() -> Option<[usize; 2]> {
    STATE.lock(|state| {
        state
            .borrow()
            .completed
            .as_ref()
            .map(|done| [done.bytes.len(), done.bytes.capacity()])
    })
}
/// Public signed material only. Existing responses remain available while the
/// next request is checked; an invalid proof cannot evict a successful receipt.
pub(crate) enum ResultReply {
    Pending(u32),
    Complete(Vec<u8>),
    Refused(u32, &'static str),
    Unknown,
    Unavailable,
}
pub(crate) fn submit(bytes: &[u8]) -> Result<u32, &'static str> {
    // Strict scalar/empty-array decoder only; no capability metadata visitors.
    crate::native_body_join::decode_command(bytes)?;
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(bytes.len())
        .map_err(|_| "body-join-memory")?;
    owned.extend_from_slice(bytes);
    STATE.lock(|state| {
        let mut state = state.borrow_mut();
        prune_completed(&mut state, embassy_time::Instant::now().as_millis());
        if let Some(done) = &state.completed {
            if done.digest == digest {
                return Ok(done.ticket);
            }
        }
        if let Some(pending) = &state.pending {
            return if pending.digest == digest {
                Ok(pending.ticket)
            } else {
                Err("body-join-busy")
            };
        }
        if let Some((ticket, current)) = state.inflight {
            return if current == digest {
                Ok(ticket)
            } else {
                Err("body-join-busy")
            };
        }
        // Retrying retained custody does not start admission or need Store access.
        // Only a new proof is gated on the current checkpoint owner being ready.
        if !state.enabled || !crate::native_restart::publication_ready() {
            return Err("body-join-unavailable");
        }
        let ticket = state.next.checked_add(1).ok_or("body-join-exhausted")?;
        state.next = ticket;
        state.pending = Some(Pending {
            ticket,
            digest,
            bytes: owned,
        });
        Ok(ticket)
    })
}
pub(crate) fn result(ticket: u32) -> ResultReply {
    STATE.lock(|state| {
        let mut state = state.borrow_mut();
        prune_completed(&mut state, embassy_time::Instant::now().as_millis());
        if let Some(done) = &state.completed {
            if done.ticket == ticket {
                let mut bytes = Vec::new();
                if bytes.try_reserve_exact(done.bytes.len()).is_err() {
                    return ResultReply::Unavailable;
                }
                bytes.extend_from_slice(&done.bytes);
                return ResultReply::Complete(bytes);
            }
        }
        if state
            .pending
            .as_ref()
            .is_some_and(|pending| pending.ticket == ticket)
            || state.inflight.is_some_and(|(active, _)| active == ticket)
        {
            return ResultReply::Pending(ticket);
        }
        if let Some((refused, reason)) = state.refused.filter(|(refused, _)| *refused == ticket) {
            ResultReply::Refused(refused, reason)
        } else if !state.enabled {
            ResultReply::Unavailable
        } else {
            ResultReply::Unknown
        }
    })
}
fn prune_completed(state: &mut State, now_ms: u64) {
    cache::prune(&mut state.completed, now_ms, |done| done.deadline);
}
impl crate::native_root::NativeRoot {
    /// Call from Root's ordinary50ms service tick, independently of both local
    /// workers. Canonical admission and checkpoint complete before receipt release.
    pub(crate) fn advance_join(&mut self, now_ms: u64) {
        if !conduit_environment_http::body_join::available() {
            if self.body_join_scope().is_err() {
                return;
            }
            let _ = conduit_environment_http::body_join::install_provider(response);
        }
        let pending = STATE.lock(|state| {
            let mut state = state.borrow_mut();
            prune_completed(&mut state, now_ms);
            state.enabled = crate::native_restart::publication_ready();
            if !state.enabled {
                return None;
            }
            let pending = state.pending.take()?;
            state.inflight = Some((pending.ticket, pending.digest));
            Some(pending)
        });
        let Some(pending) = pending else {
            return;
        };
        let ticket = pending.ticket;
        let digest = pending.digest;
        let result = (|| {
            let request = crate::native_body_join::decode_command(&pending.bytes)?;
            drop(pending);
            // Reserve response custody before mutating the authoritative Body.
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(2048)
                .map_err(|_| "body-join-memory")?;
            bytes.resize(2048, 0);
            let mut deadline = None;
            let length = match request {
                crate::native_body_join::Command::ReturnChallenge(request) => {
                    let mut nonce = [0; 32];
                    crate::native_body_join_uart::entropy(&mut nonce)?;
                    let challenge = self.begin_body_observer_return(&request, nonce, now_ms)?;
                    deadline = Some(challenge.challenge.expires_at_millis);
                    serde_json_core::to_slice(&challenge, &mut bytes)
                }
                crate::native_body_join::Command::Return(request) => {
                    let receipt = self.complete_body_observer_return(&request, now_ms)?;
                    serde_json_core::to_slice(&receipt, &mut bytes)
                }
                crate::native_body_join::Command::Admission(request) => {
                    let receipt = self.admit_body_observer_request(&request, now_ms)?;
                    serde_json_core::to_slice(&receipt, &mut bytes)
                }
                crate::native_body_join::Command::Membership(request) => {
                    let receipt = self.admit_body_membership_request(&request, now_ms)?;
                    serde_json_core::to_slice(&receipt, &mut bytes)
                }
                crate::native_body_join::Command::Recovery(request) => {
                    let receipt = self.recover_body_observer(&request)?;
                    serde_json_core::to_slice(&receipt, &mut bytes)
                }
            }
            .map_err(|_| "body-join-response-capacity")?;
            bytes.truncate(length);
            // The authoritative mutation already succeeded. Compaction may
            // retire construction slack, but pressure retains the exact receipt.
            let bytes = cache::compact(bytes);
            Ok::<_, &'static str>((bytes, deadline))
        })();
        STATE.lock(|state| {
            let mut state = state.borrow_mut();
            state.inflight = None;
            match result {
                Ok((bytes, deadline)) => {
                    state.completed = Some(Completed {
                        ticket,
                        digest,
                        bytes,
                        deadline,
                    })
                }
                Err(reason) => state.refused = Some((ticket, reason)),
            }
        });
    }
}

fn response(
    action: conduit_environment_http::body_join::Action,
) -> conduit_environment_http::body_join::Reply {
    use conduit_environment_http::body_join::{Action, Reply};
    match action {
        Action::Submit(bytes) => submit(&bytes).map_or(Reply::Unavailable, Reply::Queued),
        Action::Result(ticket) => match result(ticket) {
            ResultReply::Pending(ticket) => Reply::Queued(ticket),
            ResultReply::Complete(bytes) => Reply::Receipt(bytes),
            ResultReply::Refused(ticket, reason) => Reply::Refused(ticket, reason),
            ResultReply::Unknown => Reply::Unknown,
            ResultReply::Unavailable => Reply::Unavailable,
        },
    }
}

#[cfg(test)]
#[path = "native_body_join_http_tests.rs"]
mod tests;
