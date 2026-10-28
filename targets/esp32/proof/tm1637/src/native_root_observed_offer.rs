//! Root-only, single-request offer service. The HTTP callback carries a slot
//! hint; only the actual Root Body/key and actual frozen journal form an offer.
use super::*;
use alloc::{boxed::Box, vec::Vec};
use conduit_environment_http::observed_custody::{self as http, Reply};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
#[path = "native_observed_request_queue.rs"]
mod queue;
use queue::{Key, Queue, Request, Response};
#[cfg_attr(feature = "local-appliance", esp_hal::ram(unstable(rtc_slow)))]
static MAILBOX: Mutex<CriticalSectionRawMutex, RefCell<Queue>> =
    Mutex::new(RefCell::new(Queue::new()));
pub(super) struct Job {
    request: Request,
    inspection: crate::native_observed_archive::offer::Inspection,
}
fn response(slot: u8) -> Reply {
    submit(Request::offer(slot))
}
fn acknowledge(slot: u8, bytes: Vec<u8>) -> Reply {
    match Request::acknowledge(slot, bytes) {
        Ok(request) => submit(request),
        Err(()) => Reply::Refused,
    }
}
fn submit(request: Request) -> Reply {
    if request.key.slot > 5 || !crate::native_restart::publication_ready() {
        return Reply::Unavailable;
    }
    MAILBOX.lock(|cell| match cell.borrow_mut().submit(request) {
        Response::Pending => Reply::Pending,
        Response::Busy => Reply::Unavailable,
        Response::Completed(Ok(bytes)) => Reply::Offer(bytes),
        Response::Completed(Err(())) => Reply::Refused,
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
        if !http::ack_available() && http::available() {
            let _ = http::install_ack_provider(acknowledge);
        }
        if self.observed_offer.is_none() {
            let request = MAILBOX.lock(|cell| cell.borrow_mut().take());
            let Some(request) = request else {
                return;
            };
            let key = request.key;
            let started = (|| {
                let body = self.body.as_ref().ok_or("observed-offer-no-body")?;
                let root_key = self.admission.verifying_key();
                crate::native_restart::observed::check_live_owner(body, &root_key)?;
                let memory = esp_alloc::HEAP.stats();
                if memory.size.saturating_sub(memory.current_usage)
                    < core::mem::size_of::<Job>() + 8192
                {
                    return Err("observed-offer-workspace");
                }
                let inspection = crate::native_observed_archive::offer::Inspection::start(
                    key.slot,
                    &body.evidence().body_id,
                    &ed25519_dalek::VerifyingKey::from_bytes(&root_key)
                        .map_err(|_| "observed-offer-key")?,
                )?;
                Ok(Box::new(Job {
                    request,
                    inspection,
                }))
            })();
            match started {
                Ok(job) => self.observed_offer = Some(job),
                Err(_) => {
                    complete(key, Err(()));
                    return;
                }
            }
        }
        let job = self.observed_offer.as_mut().expect("owned inspection");
        match job.inspection.step() {
            Ok(false) => return,
            result => {
                let job = *self.observed_offer.take().expect("owned inspection");
                let key = job.request.key;
                let response = (|| {
                    let draft = job.inspection.finish()?;
                    result?;
                    let body = self.body.as_ref().ok_or("observed-offer-no-body")?;
                    if let Some(bytes) = job.request.ack {
                        let (request, used) = serde_json_core::from_slice::<
                            crate::native_observed_archive::custody_acceptance::wire::Request<'_>,
                        >(&bytes)
                        .map_err(|_| "observed-ack-json")?;
                        if used != bytes.len() {
                            return Err("observed-ack-trailing");
                        }
                        request.validate()?;
                        let accepted = self
                            .admission
                            .acknowledge_closed_observed(&draft, &request, body)?;
                        return accepted_receipt(&accepted);
                    }
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
                complete(key, response.map_err(|_| ()));
            }
        }
    }
}
fn complete(key: Key, result: Result<Vec<u8>, ()>) {
    MAILBOX.lock(|cell| cell.borrow_mut().complete(key, result));
}
/// Signed acceptance confirms custody authorization only. It does not claim that
/// a reclaim marker has committed, data was erased, or storage is reusable.
fn accepted_receipt(accepted: &[u8; 416]) -> Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(960)
        .map_err(|_| "observed-ack-response")?;
    bytes.extend_from_slice(
        b"{\"schema\":\"conduit.environment/accepted-observed-custody@1\",\"accepted\":\"",
    );
    let digits = b"0123456789abcdef";
    for byte in accepted {
        bytes.push(digits[usize::from(byte >> 4)]);
        bytes.push(digits[usize::from(byte & 15)]);
    }
    bytes.extend_from_slice(b"\"}");
    Ok(bytes)
}
