//! Local consumption of transient display lifecycle outputs. This policy keeps
//! the latest complete kernel page and an ordered digest chain of the fully
//! consumed Closed Shows. It makes no durable archive or reading-history claim.
use conduit_core as c;
use conduit_environment_runtime::display_evidence as wire;
use conduit_kernel as k;
use core::cell::RefCell;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, serde::Serialize)]
pub struct Status {
    pub consumed: u64,
    pub last_closed_bytes: usize,
    pub last_closed_digest: [u8; 32],
    pub evidence_chain: [u8; 32],
    pub retained_page_bytes: usize,
    pub durable: bool,
}
static STATUS: Mutex<CriticalSectionRawMutex, RefCell<Status>> = Mutex::new(RefCell::new(Status {
    consumed: 0,
    last_closed_bytes: 0,
    last_closed_digest: [0; 32],
    evidence_chain: [0; 32],
    retained_page_bytes: 0,
    durable: false,
}));
pub fn status() -> Status {
    STATUS.lock(|state| *state.borrow())
}
pub struct Consumer {
    next: u64,
    last_sequence: Option<u64>,
    page: [u8; wire::packed::MAXIMUM_BYTES],
    packed_length: usize,
    length: usize,
    chain: [u8; 32],
}
impl Consumer {
    pub(crate) fn completion(&self) -> (u64, Option<u64>) {
        (self.next, self.last_sequence)
    }
    /// Recover the complete original canonical page; packing changes storage
    /// only, never the event quota, digest chain, or accepted evidence bytes.
    pub fn canonical_page(&self, output: &mut [u8]) -> Result<usize, &'static str> {
        let length = wire::packed::decode(&self.page[..self.packed_length], output)
            .map_err(|_| "mask-consumer-packed-page")?;
        if length != self.length {
            return Err("mask-consumer-page-length");
        }
        Ok(length)
    }
    pub fn new() -> Self {
        Self {
            next: 0,
            last_sequence: None,
            page: [0; wire::packed::MAXIMUM_BYTES],
            packed_length: 0,
            length: 0,
            chain: [0; 32],
        }
    }
}
impl crate::native_display::EvidenceSink for Consumer {
    fn accept(
        &mut self,
        invocation: u64,
        active: &c::ActivePlayIdentity,
        closed: &[u8],
        signs: &k::FixedSignLog<64, 12>,
    ) -> Result<(), &'static str> {
        if invocation != self.next
            || closed.is_empty()
            || closed.len() > 8192
            || self
                .last_sequence
                .is_some_and(|last| active.play_sequence <= last)
            || c::bind_active_play(
                &active.plan_id,
                &active.host_id,
                &active.boot_id,
                active.play_sequence,
            ) != *active
        {
            return Err("mask-consumer-scope");
        }
        let next = self.next.checked_add(1).ok_or("mask-consumer-exhausted")?;
        // Encode/check the entire exact page before replacing retained custody.
        let mut page = [0; wire::MAXIMUM_BYTES];
        let length = wire::encode(invocation, signs.events(), signs, &mut page)
            .map_err(|_| "mask-consumer-evidence")?;
        let parsed = wire::Page::parse(&page[..length]).map_err(|_| "mask-consumer-page")?;
        if parsed.invocation() != invocation
            || !parsed
                .records()
                .any(|record| record.event.kind == k::KernelEventKind::BackCompleted)
        {
            return Err("mask-consumer-not-terminal");
        }
        let mut packed = [0; wire::packed::MAXIMUM_BYTES];
        let packed_length = wire::packed::encode(&page[..length], &mut packed)
            .map_err(|_| "mask-consumer-packed-page")?;
        let closed_digest: [u8; 32] = Sha256::digest(closed).into();
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/transient-mask-consumption@1");
        hash.update(self.chain);
        hash.update(invocation.to_le_bytes());
        hash.update(active.active_play_id.as_str().as_bytes());
        hash.update((closed.len() as u64).to_le_bytes());
        hash.update(closed_digest);
        hash.update(&page[..length]);
        let chain = hash.finalize().into();
        self.page = packed;
        self.packed_length = packed_length;
        self.length = length;
        self.chain = chain;
        self.next = next;
        self.last_sequence = Some(active.play_sequence);
        STATUS.lock(|state| {
            *state.borrow_mut() = Status {
                consumed: next,
                last_closed_bytes: closed.len(),
                last_closed_digest: closed_digest,
                evidence_chain: chain,
                retained_page_bytes: length,
                durable: false,
            }
        });
        Ok(())
    }
}
