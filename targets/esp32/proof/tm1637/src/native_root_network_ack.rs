//! Root-only scheduling of durable Pending custody. HTTP queues bounded input;
//! sampling executes one qualified flash operation during its actual wait.
use super::*;
use crate::native_restart::network_pending_job::{self, PendingJob, Start};
use alloc::vec::Vec;
use core::cell::Cell;
#[path = "native_network_ack_queue.rs"]
mod queue;
use queue::{Key, Queue, Receipt, Response};
const WRITE_MS: u64 = 300_000;
#[cfg_attr(target_os = "none", esp_hal::ram(unstable(rtc_slow)))]
static ACKS: Mutex<CriticalSectionRawMutex, Cell<Option<Box<Queue>>>> = Mutex::new(Cell::new(None));
fn with_queue<T>(run: impl FnOnce(&mut Queue) -> T) -> Option<T> {
    ACKS.lock(|cell| {
        let mut queue = cell.take()?;
        let result = run(&mut queue);
        cell.set(Some(queue));
        Some(result)
    })
}
pub(super) struct Writing {
    owner: Option<Box<PendingJob>>,
    queued: Option<u64>,
    key: Key,
    deadline: u64,
}
pub(super) fn install() {
    if with_queue(|_| ()).is_none() {
        let reserved = (|| {
            if esp_alloc::HEAP.free() < core::mem::size_of::<Queue>() + FREE_FLOOR {
                return Err("network-ack-queue-capacity");
            }
            let backing = local_renewal::reserve_box::<Queue>()?;
            if esp_alloc::HEAP.free() < FREE_FLOOR {
                return Err("network-ack-queue-capacity");
            }
            Ok(Box::write(backing, Queue::new()))
        })();
        let Ok(queue) = reserved else { return };
        ACKS.lock(|cell| {
            if let Some(existing) = cell.take() {
                cell.set(Some(existing));
            } else {
                cell.set(Some(queue));
            }
        });
    }
    if !http::ack_available() {
        let _ = http::install_ack_provider(response);
    }
}

fn response(bytes: Vec<u8>) -> Reply {
    if esp_alloc::HEAP.free() < FREE_FLOOR {
        return Reply::Unavailable;
    }
    let result = with_queue(|queue| queue.submit(bytes));
    let Some(result) = result else {
        return Reply::Unavailable;
    };
    match result {
        Response::Pending => Reply::Pending,
        Response::Busy => Reply::Unavailable,
        Response::Refused => Reply::Refused,
        Response::Completed(receipt) => render(receipt),
    }
}
fn render(receipt: Receipt) -> Reply {
    if esp_alloc::HEAP.free() < FREE_FLOOR {
        return Reply::Unavailable;
    }
    let mut out = Vec::new();
    if out
        .try_reserve_exact(snapshot::export::REPLY_BYTES)
        .is_err()
    {
        return Reply::Unavailable;
    }
    let result = crate::native_restart::network_pending::with_committed_cursor(
        receipt.generation,
        &receipt.offer,
        |generation, cursor| {
            out.extend_from_slice(b"{\"profile\":\"network-retirement-committed@1\",\"reclaimed\":false,\"generation\":");
            let mut digits = [0u8; 20];
            let mut n = generation;
            let mut at = 20;
            loop {
                at -= 1;
                digits[at] = b'0' + (n % 10) as u8;
                n /= 10;
                if n == 0 {
                    break;
                }
            }
            out.extend_from_slice(&digits[at..]);
            out.extend_from_slice(b",\"receipt\":\"");
            for b in cursor {
                out.push(b"0123456789abcdef"[(b >> 4) as usize]);
                out.push(b"0123456789abcdef"[(b & 15) as usize]);
            }
            out.extend_from_slice(b"\"}");
        },
    );
    if result.is_err()
        || esp_alloc::HEAP.free() < 2 * 8192
        || out.len() > snapshot::export::REPLY_BYTES
    {
        Reply::Unavailable
    } else {
        Reply::Offer(out)
    }
}
fn complete(
    key: Key,
    result: Result<crate::native_restart::network_pending::CommittedCustody, &'static str>,
) {
    let result = result
        .map(|r| Receipt {
            generation: r.generation(),
            offer: *r.offer(),
        })
        .map_err(|_| ());
    with_queue(|queue| queue.complete(key, result)).expect("Root-owned ACK queue");
}
impl NativeRoot {
    pub(crate) fn network_write_inflight(&self) -> bool {
        SLOT.lock(|cell| {
            cell.borrow()
                .session
                .as_ref()
                .is_some_and(|s| matches!(s.phase, Phase::Writing(_)))
        })
    }
    /// Returns true while this turn belongs to ACK handling. Busy input remains
    /// in its finite queue, including while another Root mutation owns storage.
    pub(super) fn advance_network_ack(
        &mut self,
        session: &mut Option<Box<Session>>,
        now: u64,
    ) -> bool {
        if let Some(owned) = session.as_mut() {
            if let Phase::Writing(writing) = &mut owned.phase {
                if self.poll_network_write(writing, now) {
                    owned.phase = Phase::Failed;
                    owned.deadline = now.saturating_add(COOLDOWN_MS);
                }
                return true;
            }
        }
        if self.local_renewal_pending()
            || self.pending_network_workspace.is_some()
            || self.observed_offer.is_some()
            || self.observed_reclaim.is_some()
            || !crate::native_restart::publication_ready()
            || !self
                .reclaim_submitter
                .as_ref()
                .is_some_and(|s| s.available())
            || session
                .as_ref()
                .is_some_and(|s| matches!(s.phase, Phase::Verifying { .. }))
        {
            return false;
        }
        let request = with_queue(|queue| queue.take()).flatten();
        let Some(request) = request else { return false };
        let result = (|| {
            let decoded = snapshot::ack::Request::decode(&request.bytes)?;
            if let Some(receipt) = self.replay_network_snapshot_custody(&decoded)? {
                return Ok(Some(receipt));
            }
            let owned = session.as_mut().ok_or("network-ack-export-missing")?;
            if !matches!(owned.phase, Phase::Ready(_)) {
                return Err("network-ack-export-not-ready");
            }
            if esp_alloc::HEAP.free() < core::mem::size_of::<PendingJob>() + 480 + FREE_FLOOR {
                return Err("network-ack-workspace");
            }
            let Phase::Ready(export) = core::mem::replace(&mut owned.phase, Phase::Failed) else {
                unreachable!()
            };
            let (draft, signed, deadline) = export.into_parts();
            let started = match network_pending_job::start(
                self.body.as_ref().ok_or("network-snapshot-body")?,
                &self.boot,
                &self.admission.verifying_key(),
                draft,
                &decoded,
            ) {
                Ok(started) => started,
                Err((draft, reason)) => {
                    owned.phase = Phase::Ready(Export::new(draft, signed, deadline)?);
                    return Err(reason);
                }
            };
            match started {
                Start::Committed(receipt) => Ok(Some(receipt)),
                Start::Writing(job) => {
                    if esp_alloc::HEAP.free() < FREE_FLOOR {
                        drop(job);
                        return Err("network-ack-workspace");
                    }
                    owned.phase = Phase::Writing(Writing {
                        owner: Some(job),
                        queued: None,
                        key: request.key,
                        deadline: now.saturating_add(WRITE_MS),
                    });
                    Ok(None)
                }
            }
        })();
        match result {
            Ok(None) => {}
            Ok(Some(receipt)) => complete(request.key, Ok(receipt)),
            Err(reason) => {
                complete(request.key, Err(reason));
                esp_println::println!(
                    "{{\"event\":\"network-custody-refused\",\"reason\":\"{}\"}}",
                    reason
                );
            }
        }
        true
    }
    fn poll_network_write(&mut self, writing: &mut Writing, now: u64) -> bool {
        let Some(submitter) = self.reclaim_submitter.as_mut() else {
            return false;
        };
        if let Some(id) = writing.queued {
            if submitter.phase() != crate::native_flash_jobs::Phase::Completed(id) {
                if now >= writing.deadline
                    && submitter.phase() == crate::native_flash_jobs::Phase::Queued(id)
                {
                    if let Ok(owner) = submitter.cancel_pending(id) {
                        writing.queued = None;
                        drop(owner);
                        complete(writing.key, Err("network-ack-deadline"));
                        return true;
                    }
                }
                return false;
            }
            let completion = match submitter.take_pending(id) {
                Ok(c) => c,
                Err(_) => return false,
            };
            writing.queued = None;
            writing.owner = Some(completion.owner);
            if completion.result.is_err()
                || completion.progress != crate::native_flash_jobs::Progress::AdvancedOne
            {
                drop(writing.owner.take());
                complete(writing.key, Err("network-ack-physical-step"));
                return true;
            }
        }
        if now >= writing.deadline {
            drop(writing.owner.take());
            complete(writing.key, Err("network-ack-deadline"));
            return true;
        }
        let Some(mut job) = writing.owner.take() else {
            return false;
        };
        let Some(owner) = self.body.as_ref() else {
            drop(job);
            complete(writing.key, Err("network-snapshot-body"));
            return true;
        };
        if job.is_complete() {
            complete(writing.key, job.finish(owner));
            return true;
        }
        if job.needs_chunk() {
            if let Err(reason) = job.prepare_chunk(owner) {
                drop(job);
                complete(writing.key, Err(reason));
                return true;
            }
        }
        if !submitter.available() {
            writing.owner = Some(job);
            return false;
        }
        match submitter.submit_pending(job) {
            Ok(id) => writing.queued = Some(id),
            Err((_, job)) => writing.owner = Some(job),
        }
        false
    }
}
