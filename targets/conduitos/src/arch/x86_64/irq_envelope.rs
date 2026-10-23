//! Proof-only finite software IRQ observations. No guest mapping or clock grant.
use super::ServiceDomain;
use crate::protected_region::DomainRefusal;
use sha2::{Digest, Sha256};
const CAPACITY: usize = 64;
mod policy;
unsafe extern "C" {
    static mut conduitos_irq_clock: [u64; 2];
}
#[derive(Clone, Copy, Default)]
struct Sample {
    start: u64,
    end: u64,
    budget: bool,
    preempted: bool,
    saved_preserved: bool,
    canaries: bool,
}
struct State {
    instance: u64,
    digest: [u8; 32],
    trial: usize,
    active: bool,
    entered: bool,
    overflow: bool,
    count: usize,
    pending: Option<usize>,
    saved: [u64; 15],
    saved_flags: u64,
    rows: [Sample; CAPACITY],
}
static mut STATE: State = State {
    instance: 0,
    digest: [0; 32],
    trial: 0,
    active: false,
    entered: false,
    overflow: false,
    count: 0,
    pending: None,
    saved: [0; 15],
    saved_flags: 0,
    rows: [Sample {
        start: 0,
        end: 0,
        budget: false,
        preempted: false,
        saved_preserved: false,
        canaries: false,
    }; CAPACITY],
};
fn stamp(index: usize) -> u64 {
    // Single proof CPU, IRQ-disabled handler and serialized domain entry.
    unsafe {
        core::ptr::addr_of!(conduitos_irq_clock)
            .cast::<u64>()
            .add(index)
            .read_volatile()
    }
}
fn complete(state: &mut State) {
    if let Some(index) = state.pending.take() {
        state.rows[index].end = stamp(1);
    }
}
impl ServiceDomain {
    /// Only the exact compiled fixture installer is eligible; no TSD permission
    /// is conferred. One immediately following entry owns these finite samples.
    pub(crate) fn arm_irq_fixture(&mut self, trial: usize) -> Result<(), DomainRefusal> {
        if self.quarantined || self.clock_binding.is_none() || trial >= 6 {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        let _interrupts = super::cpu::InterruptMask::new();
        let state = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
        if state.active {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        state.instance = self.instance();
        state.digest =
            Sha256::digest(include_bytes!(concat!(env!("OUT_DIR"), "/domain.elf"))).into();
        state.trial = trial;
        state.count = 0;
        state.pending = None;
        state.overflow = false;
        state.entered = false;
        state.active = true;
        Ok(())
    }
    pub(crate) fn finish_irq_fixture(&self) -> Result<(), DomainRefusal> {
        let _interrupts = super::cpu::InterruptMask::new();
        let state = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
        if !state.active || !state.entered || state.instance != self.instance() {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        state.active = false;
        complete(state);
        let mut valid = !state.overflow && state.count > 0;
        let mut canary_resumes = 0;
        let mut preemptions = 0;
        for (ordinal, sample) in state.rows[..state.count].iter().enumerate() {
            valid &= sample.start != 0 && sample.end >= sample.start && sample.saved_preserved;
            canary_resumes += usize::from(sample.canaries && !sample.preempted);
            preemptions += usize::from(sample.preempted);
            emit(state.instance, &state.digest, state.trial, ordinal, sample);
        }
        valid &= if state.trial < 3 {
            canary_resumes > 0 && preemptions == 0
        } else {
            preemptions == 1 && state.rows[state.count - 1].preempted
        };
        if !valid {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        Ok(())
    }
}
/// `saved` points to the actual15-GPR IRQ save frame before the original Rust
/// handler. The common assembly pushes this complete frame before this call.
// SAFETY: callers are the IRQ-disabled assembly save/handler path; Root arm,
// finish and entry validation mask interrupts before borrowing the private state.
pub(super) unsafe fn before_handler(saved: *const u64, cs: u64, budget: u64) -> bool {
    let state = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
    if !state.active || cs & 3 != 3 {
        return false;
    }
    complete(state);
    if state.count == CAPACITY {
        state.overflow = true;
        return false;
    }
    let words: [u64; 15] = core::array::from_fn(|i| unsafe { saved.add(i).read() });
    // Reverse push order: R15..R8,RDI,RSI,RBP,RBX,RDX,RCX,RAX.
    let canaries = policy::canary_range(&words, unsafe { saved.add(18).read() });
    state.saved = words;
    state.saved_flags = unsafe { saved.add(18).read() };
    state.rows[state.count] = Sample {
        start: stamp(0),
        end: 0,
        budget: budget != 0,
        preempted: false,
        saved_preserved: false,
        canaries,
    };
    state.pending = Some(state.count);
    state.count += 1;
    true
}
pub(super) unsafe fn after_handler(saved: *const u64, disposition: u64) {
    let state = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
    if let Some(index) = state.pending {
        let sample = &mut state.rows[index];
        sample.saved_preserved = state
            .saved
            .iter()
            .enumerate()
            .all(|(i, v)| *v == unsafe { saved.add(i).read() })
            && state.saved_flags == unsafe { saved.add(18).read() };
        sample.preempted = disposition != 0;
    }
}
fn emit(instance: u64, digest: &[u8; 32], trial: usize, ordinal: usize, sample: &Sample) {
    use core::fmt::Write;
    let mut line = crate::sign_format::FixedText::new();
    let mut hash = crate::sign_format::FixedText::new();
    for byte in digest {
        let _ = write!(hash, "{byte:02x}");
    }
    let hash = core::str::from_utf8(hash.as_bytes()).unwrap_or("");
    let written = writeln!(
        line,
        "CONDUIT_SERVICE_SOFTWARE_IRQ {{\"schema\":\"conduit.conduitos/service-software-irq@1\",\"instance\":{instance},\"guest_sha256\":\"{hash}\",\"trial\":{trial},\"ordinal\":{ordinal},\"start\":{},\"end\":{},\"budget\":{},\"preempted\":{},\"saved_gprs_and_rflags_preserved\":{},\"guest_canary_range\":{},\"overhead_included\":true,\"hardware_irq_latency\":false,\"cpu_entry_and_iret_excluded\":true}}",
        sample.start,
        sample.end,
        sample.budget,
        sample.preempted,
        sample.saved_preserved,
        sample.canaries
    );
    if written.is_err() {
        super::early_write(b"CONDUIT_ORDINARY_DOMAIN_REFUSAL software-irq-line-bound\n");
        super::deterministic_exit(false);
    }
    super::early_write(line.as_bytes());
}

pub(super) fn validate_entry(instance: u64) -> Result<(), DomainRefusal> {
    let _interrupts = super::cpu::InterruptMask::new();
    let state = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
    if !state.active {
        return Ok(());
    }
    if state.instance != instance || state.entered {
        return Err(DomainRefusal::InvalidLifecycle);
    }
    state.entered = true;
    Ok(())
}
