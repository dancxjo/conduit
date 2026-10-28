//! One finite read-only snapshot lease. All work belongs to the existing Root
//! grant; a request cannot enlarge reserves, install custody or erase evidence.
use super::*;
use crate::native_play::journal::snapshot::{self, export::Export};
use alloc::boxed::Box;
use conduit_environment_http::network_custody::{self as http, Reply, Request};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
#[path = "native_root_network_ack.rs"]
mod ack;
const LEASE_MS: u64 = 120_000;
const COOLDOWN_MS: u64 = 30_000;
// Preserve the existing 8KiB Root and 8KiB child preparation allowances, plus
// the one bounded response allocation while the dynamic export owner exists.
const FREE_FLOOR: usize = 2 * 8192 + snapshot::export::REPLY_BYTES;
enum Phase {
    Verifying {
        verifier: Option<snapshot::Verifier>,
        work: [u8; 512],
    },
    Ready(Export),
    Writing(ack::Writing),
    Failed,
}
struct Session {
    phase: Phase,
    deadline: u64,
}
struct Slot {
    requested: bool,
    session: Option<Box<Session>>,
    not_before: u64,
}
// CPU-only volatile mailbox/pointers, initialized each Boot; never DMA backing.
#[cfg_attr(target_os = "none", esp_hal::ram(unstable(rtc_slow)))]
static SLOT: Mutex<CriticalSectionRawMutex, RefCell<Slot>> = Mutex::new(RefCell::new(Slot {
    requested: false,
    session: None,
    not_before: 0,
}));
fn expire(slot: &mut Slot, now: u64) {
    if slot
        .session
        .as_ref()
        .is_some_and(|s| now >= s.deadline && !matches!(s.phase, Phase::Writing(_)))
    {
        slot.session = None;
        slot.requested = false;
        slot.not_before = now.saturating_add(COOLDOWN_MS);
    }
}
fn response(request: Request) -> Reply {
    let now = embassy_time::Instant::now().as_millis();
    SLOT.lock(|cell| {
        let mut slot = cell.borrow_mut();
        expire(&mut slot, now);
        if let Some(session) = slot.session.as_mut() {
            return match &mut session.phase {
                Phase::Ready(export) => {
                    if esp_alloc::HEAP.free() < FREE_FLOOR {
                        return Reply::Unavailable;
                    }
                    let reply = export.reply(request, now);
                    // Check actual allocator charge, including alignment/header
                    // overhead, before exposing any response allocation.
                    if esp_alloc::HEAP.free() < 2 * 8192 {
                        drop(reply);
                        return Reply::Unavailable;
                    }
                    reply
                }
                Phase::Verifying { .. } | Phase::Writing(_) => Reply::Pending,
                Phase::Failed => Reply::Refused,
            };
        }
        if !matches!(request, Request::Offer) {
            return Reply::Refused;
        }
        if now < slot.not_before || !crate::native_restart::publication_ready() {
            return Reply::Unavailable;
        }
        slot.requested = true;
        Reply::Pending
    })
}
impl NativeRoot {
    /// Run even while other preparation is deferred: expiration must always
    /// restore the actual frozen owner. Each productive turn reads at most512B.
    pub(crate) fn advance_network_export(&mut self) {
        let now = embassy_time::Instant::now().as_millis();
        SLOT.lock(|cell| expire(&mut cell.borrow_mut(), now));
        if !http::available() {
            if self.body.is_none() || !crate::native_restart::publication_ready() {
                return;
            }
            let _ = http::install_provider(response);
        }
        // Temporarily take the affine owner out of the mailbox for this bounded
        // Root turn. HTTP can only queue an Offer while it is checked here.
        ack::install();
        let (mut owned, requested) = SLOT.lock(|cell| {
            let mut slot = cell.borrow_mut();
            (slot.session.take(), core::mem::take(&mut slot.requested))
        });
        if self.advance_network_ack(&mut owned, now) {
            SLOT.lock(|cell| {
                cell.borrow_mut().session = owned;
            });
            return;
        }
        let mut owned = match owned {
            Some(owned) => owned,
            None if requested && !self.local_renewal_pending() => {
                let prepared = (|| {
                    if esp_alloc::HEAP.free() < core::mem::size_of::<Session>() + FREE_FLOOR {
                        return Err("network-export-workspace");
                    }
                    let backing = local_renewal::reserve_box::<Session>()?;
                    if esp_alloc::HEAP.free() < FREE_FLOOR {
                        return Err("network-export-workspace");
                    }
                    let verifier = self.begin_network_snapshot()?;
                    Ok(Box::write(
                        backing,
                        Session {
                            phase: Phase::Verifying {
                                verifier: Some(verifier),
                                work: [0; 512],
                            },
                            deadline: now.saturating_add(LEASE_MS),
                        },
                    ))
                })();
                match prepared {
                    Ok(owned) => owned,
                    Err(reason) => {
                        SLOT.lock(|cell| {
                            cell.borrow_mut().not_before = now.saturating_add(COOLDOWN_MS)
                        });
                        esp_println::println!(
                            "{{\"event\":\"network-export-refused\",\"reason\":\"{}\"}}",
                            reason
                        );
                        return;
                    }
                }
            }
            None => {
                if requested {
                    SLOT.lock(|cell| cell.borrow_mut().requested = true);
                }
                return;
            }
        };
        if let Phase::Verifying { verifier, work } = &mut owned.phase {
            let result = verifier
                .as_mut()
                .expect("owned snapshot verifier")
                .step(work);
            if result != Ok(false) {
                let verified = verifier.take().expect("owned snapshot verifier").finish();
                let result = result
                    .and_then(|_| verified)
                    .and_then(|verified| self.endorse_network_snapshot(verified))
                    .and_then(|(draft, signed)| Export::new(draft, signed, owned.deadline));
                owned.phase = match result {
                    Ok(export) => Phase::Ready(export),
                    Err(_) => Phase::Failed,
                };
            }
        }
        SLOT.lock(|cell| {
            cell.borrow_mut().session = Some(owned);
        });
    }
}
