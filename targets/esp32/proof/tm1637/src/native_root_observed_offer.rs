//! Root-only, single-request offer service. The HTTP callback carries a slot
//! hint; only the actual Root Body/key and actual frozen journal form an offer.
use super::*;
use alloc::{boxed::Box, vec::Vec};
use conduit_environment_http::observed_custody::{self as http, Reply};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
struct Mailbox {
    pending: Option<u8>,
    inflight: Option<u8>,
    completed: Option<(u8, Result<Vec<u8>, ()>)>,
}
#[cfg_attr(feature = "local-appliance", esp_hal::ram(unstable(rtc_slow)))]
static MAILBOX: Mutex<CriticalSectionRawMutex, RefCell<Mailbox>> =
    Mutex::new(RefCell::new(Mailbox {
        pending: None,
        inflight: None,
        completed: None,
    }));
pub(super) struct Job {
    slot: u8,
    inspection: crate::native_observed_archive::offer::Inspection,
}
fn response(slot: u8) -> Reply {
    if slot > 5 || !crate::native_restart::publication_ready() {
        return Reply::Unavailable;
    }
    MAILBOX.lock(|cell| {
        let mut state = cell.borrow_mut();
        if state
            .completed
            .as_ref()
            .is_some_and(|(done, result)| *done == slot && result.is_err())
        {
            // A Cold/empty slot can become closed later. Deliver this refusal
            // once rather than permanently hiding a future epoch in that slot.
            state.completed = None;
            return Reply::Refused;
        }
        if let Some((done, result)) = &state.completed {
            if *done == slot {
                return match result {
                    Err(()) => Reply::Refused,
                    Ok(bytes) => {
                        let mut owned = Vec::new();
                        if owned.try_reserve_exact(bytes.len()).is_err() {
                            return Reply::Unavailable;
                        }
                        owned.extend_from_slice(bytes);
                        Reply::Offer(owned)
                    }
                };
            }
        }
        if let Some(current) = state.pending.or(state.inflight) {
            return if current == slot {
                Reply::Pending
            } else {
                Reply::Unavailable
            };
        }
        state.pending = Some(slot);
        Reply::Pending
    })
}
impl NativeRoot {
    pub(crate) fn advance_observed_offer(&mut self) {
        if !http::available() {
            if self.body.as_ref().is_none() || !crate::native_restart::publication_ready() {
                return;
            }
            let _ = http::install_provider(response);
        }
        if self.observed_offer.is_none() {
            let slot = MAILBOX.lock(|cell| {
                let mut state = cell.borrow_mut();
                let slot = state.pending.take()?;
                state.inflight = Some(slot);
                Some(slot)
            });
            let Some(slot) = slot else {
                return;
            };
            let started = (|| {
                let body = self.body.as_ref().ok_or("observed-offer-no-body")?;
                let key = self.admission.verifying_key();
                crate::native_restart::observed::check_live_owner(body, &key)?;
                let memory = esp_alloc::HEAP.stats();
                if memory.size.saturating_sub(memory.current_usage)
                    < core::mem::size_of::<Job>() + 8192
                {
                    return Err("observed-offer-workspace");
                }
                let inspection = crate::native_observed_archive::offer::Inspection::start(
                    slot,
                    &body.evidence().body_id,
                    &ed25519_dalek::VerifyingKey::from_bytes(&key)
                        .map_err(|_| "observed-offer-key")?,
                )?;
                Ok(Box::new(Job { slot, inspection }))
            })();
            match started {
                Ok(job) => self.observed_offer = Some(job),
                Err(_) => {
                    complete(slot, Err(()));
                    return;
                }
            }
        }
        let job = self.observed_offer.as_mut().expect("owned inspection");
        match job.inspection.step() {
            Ok(false) => return,
            result => {
                let job = *self.observed_offer.take().expect("owned inspection");
                let slot = job.slot;
                let response = (|| {
                    let draft = job.inspection.finish()?;
                    result?;
                    let body = self.body.as_ref().ok_or("observed-offer-no-body")?;
                    let offer = self.admission.sign_closed_observed_offer(draft, body)?;
                    let mut bytes = Vec::new();
                    bytes
                        .try_reserve_exact(1024)
                        .map_err(|_| "observed-offer-response")?;
                    bytes.resize(1024, 0);
                    let length = serde_json_core::to_slice(&offer, &mut bytes)
                        .map_err(|_| "observed-offer-response")?;
                    bytes.truncate(length);
                    Ok::<_, &'static str>(bytes)
                })();
                complete(slot, response.map_err(|_| ()));
            }
        }
    }
}
fn complete(slot: u8, result: Result<Vec<u8>, ()>) {
    MAILBOX.lock(|cell| {
        let mut state = cell.borrow_mut();
        state.inflight = None;
        state.completed = Some((slot, result));
    });
}
