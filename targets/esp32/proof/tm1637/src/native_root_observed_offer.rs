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
static MAILBOX: Mutex<CriticalSectionRawMutex, RefCell<Queue<Completed>>> =
    Mutex::new(RefCell::new(Queue::new()));
// Cache the exact compact immutable proof once. JSON is response presentation,
// not retained Root metadata or another copy of the inspected epoch.
#[derive(Clone, Copy)]
enum Completed {
    Offer(crate::native_observed_archive::custody_acceptance::wire::Offer<'static>),
    Accepted([u8; 416]),
}
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
    let result = MAILBOX.lock(|cell| cell.borrow_mut().submit(request));
    match result {
        Response::Pending => Reply::Pending,
        Response::Busy => Reply::Unavailable,
        Response::Completed(Ok(proof)) => match render_proof(proof) {
            Ok(bytes) => Reply::Offer(bytes),
            Err(_) => Reply::Unavailable,
        },
        Response::Completed(Err(())) => Reply::Refused,
    }
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
                    let closed = job.inspection.finish_owned()?;
                    result?;
                    let draft = closed.draft();
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
                            .acknowledge_closed_observed(draft, &request, body)?;
                        let root_key = ed25519_dalek::VerifyingKey::from_bytes(
                            &self.admission.verifying_key(),
                        )
                        .map_err(|_| "observed-ack-root")?;
                        if !draft.matches_accepted(&accepted, &body.evidence().body_id, &root_key) {
                            return Err("observed-ack-closed-owner");
                        }
                        if self.observed_reclaim.is_some()
                            || !self
                                .reclaim_submitter
                                .as_ref()
                                .is_some_and(|submitter| submitter.available())
                        {
                            return Err("observed-reclaim-busy-or-unreviewed");
                        }
                        let receipt = crate::native_restart::observed::read_live_layout_receipt(
                            body,
                            &closed.identity(),
                        )?;
                        let start = super::local_renewal::reserve_box::<
                            crate::native_observed_reclaim_job::PreparedStart<
                                crate::native_evidence_region::ObservationEpoch,
                            >,
                        >()?;
                        let state = super::local_renewal::reserve_box::<Reclamation>()?;
                        let (destination, prepared) = closed.prepare_reclaim(
                            &accepted,
                            &receipt,
                            &body.evidence().body_id,
                            &root_key,
                        )?;
                        let owner = crate::native_observed_reclaim_job::ReclaimJob::from_prepared(
                            Box::write(
                                start,
                                crate::native_observed_reclaim_job::PreparedStart::new(prepared),
                            ),
                        );
                        self.observed_reclaim = Some(Box::write(
                            state,
                            Reclamation {
                                destination: Some(destination),
                                owner: Some(owner),
                                queued: None,
                                deadline: embassy_time::Instant::now()
                                    .as_millis()
                                    .saturating_add(300_000),
                                failed: false,
                            },
                        ));
                        return Ok(Completed::Accepted(accepted));
                    }
                    let offer = closed.sign_offer(|draft| {
                        self.admission.sign_closed_observed_offer(draft, body)
                    })?;
                    Ok::<_, &'static str>(Completed::Offer(offer))
                })();
                if let Err(reason) = &response {
                    esp_println::println!(
                        "{{\"event\":\"observed-custody-refused\",\"reason\":\"{}\"}}",
                        reason
                    );
                }
                complete(key, response.map_err(|_| ()));
            }
        }
    }
}
type ReclaimOwner =
    crate::native_observed_reclaim_job::ReclaimJob<crate::native_evidence_region::ObservationEpoch>;
pub(super) struct Reclamation {
    destination: Option<crate::native_observed_archive::offer::ReclaimSlot>,
    owner: Option<ReclaimOwner>,
    queued: Option<u64>,
    deadline: u64,
    failed: bool,
}
impl NativeRoot {
    /// Root polls completions even during local renewal. The sampler's actual
    /// wait provider performs every physical step under its own scoped permit.
    pub(crate) fn advance_observed_reclaim(&mut self) {
        let Some(mut state) = self.observed_reclaim.take() else {
            return;
        };
        if state.failed {
            self.observed_reclaim = Some(state);
            return;
        }
        let Some(submitter) = self.reclaim_submitter.as_mut() else {
            state.failed = true;
            self.observed_reclaim = Some(state);
            return;
        };
        if let Some(id) = state.queued {
            if submitter.phase() != crate::native_flash_jobs::Phase::Completed(id) {
                if embassy_time::Instant::now().as_millis() >= state.deadline
                    && submitter.phase() == crate::native_flash_jobs::Phase::Queued(id)
                {
                    if let Ok(owner) = submitter.cancel(id) {
                        state.queued = None;
                        state.owner = Some(owner);
                        retire_reclaim(&mut state);
                        esp_println::println!("{{\"event\":\"observed-live-reclaim-refused\",\"reason\":\"queued-deadline\"}}");
                        if state.owner.is_some() {
                            self.observed_reclaim = Some(state);
                        }
                        return;
                    }
                }
                self.observed_reclaim = Some(state);
                return;
            }
            let completion = match submitter.take(id) {
                Ok(completion) => completion,
                Err(_) => {
                    state.failed = true;
                    self.observed_reclaim = Some(state);
                    return;
                }
            };
            state.queued = None;
            state.owner = Some(completion.owner);
            if completion.result.is_err()
                || completion.progress != crate::native_flash_jobs::Progress::AdvancedOne
            {
                retire_reclaim(&mut state);
                if state.owner.is_some() {
                    self.observed_reclaim = Some(state);
                }
                esp_println::println!(
                    "{{\"event\":\"observed-live-reclaim-refused\",\"reason\":\"physical-step\"}}"
                );
                return;
            }
        }
        let Some(owner) = state.owner.take() else {
            state.failed = true;
            self.observed_reclaim = Some(state);
            return;
        };
        let owner = match owner.into_erased() {
            Ok(proof) => {
                state
                    .destination
                    .take()
                    .expect("actual reclaim destination")
                    .erased(proof);
                esp_println::println!(
                    "{{\"event\":\"observed-live-reclaimed\",\"verified_reusable\":true}}"
                );
                return;
            }
            Err(owner) => owner,
        };
        if embassy_time::Instant::now().as_millis() >= state.deadline {
            state.owner = Some(owner);
            retire_reclaim(&mut state);
            if state.owner.is_some() {
                self.observed_reclaim = Some(state);
            }
            esp_println::println!(
                "{{\"event\":\"observed-live-reclaim-refused\",\"reason\":\"deadline\"}}"
            );
            return;
        }
        match submitter.submit(owner) {
            Ok(id) => state.queued = Some(id),
            Err((_, owner)) => {
                state.owner = Some(owner);
                retire_reclaim(&mut state);
                esp_println::println!(
                    "{{\"event\":\"observed-live-reclaim-refused\",\"reason\":\"submission\"}}"
                );
            }
        }
        if state.queued.is_some() || state.owner.is_some() {
            self.observed_reclaim = Some(state);
        }
    }
}
fn retire_reclaim(state: &mut Reclamation) {
    let Some(owner) = state.owner.take() else {
        state.failed = true;
        return;
    };
    match owner.into_custody() {
        Ok(custody) => state
            .destination
            .take()
            .expect("actual reclaim destination")
            .retained(custody),
        Err(owner) => {
            state.owner = Some(owner);
            state.failed = true;
        }
    }
}
fn complete(key: Key, result: Result<Completed, ()>) {
    MAILBOX.lock(|cell| cell.borrow_mut().complete(key, result));
}
fn render_proof(proof: Completed) -> Result<Vec<u8>, &'static str> {
    match proof {
        Completed::Accepted(accepted) => accepted_receipt(&accepted),
        Completed::Offer(offer) => {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(1024)
                .map_err(|_| "observed-offer-response")?;
            bytes.resize(1024, 0);
            let length = serde_json_core::to_slice(&offer, &mut bytes)
                .map_err(|_| "observed-offer-response")?;
            bytes.truncate(length);
            Ok(bytes)
        }
    }
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
