//! Actual x87 #NM frame control, separate from SIMD and broad FPU claims.
use super::refuse;
use crate::{
    arch,
    protected_region::{DomainBackend, DomainFault, DomainReturn},
};
use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};
static VALUE: AtomicU64 = AtomicU64::new(u64::MAX);
static ORIGIN: AtomicU64 = AtomicU64::new(u64::MAX);
/// Root receives these raw words only after the assembly transition returns.
pub(crate) fn observe(value: u64, origin: u64) {
    VALUE.store(value, Ordering::Release);
    ORIGIN.store(origin, Ordering::Release);
}
pub(super) fn run() {
    let root_sentinel = 0x6e6d_726f_6f74_5a5au64;
    let mut sibling = arch::TextDomain::install().unwrap_or_else(|_| refuse("x87-sibling-install"));
    sibling
        .input(b"nm sibling")
        .unwrap_or_else(|_| refuse("x87-sibling-input"));
    let sibling_address = sibling.private_frame_address();
    let sibling_before = unsafe { (sibling_address as *const u32).read_volatile() };
    let mut domain = arch::TextDomain::install().unwrap_or_else(|_| refuse("x87-domain-install"));
    domain.probe(21, 0);
    observe(u64::MAX, u64::MAX);
    arch::early_write(b"CONDUIT_X87_NM_ATTEMPT instruction=fld1 expected_vector=7\n");
    let returned = domain.enter(1);
    let value = VALUE.load(Ordering::Acquire);
    let origin = ORIGIN.load(Ordering::Acquire);
    let mut observation = crate::sign_format::FixedText::new();
    writeln!(
        observation,
        "CONDUIT_X87_NM_RAW origin={origin} value={value}"
    )
    .unwrap_or_else(|_| refuse("x87-observation-capacity"));
    arch::early_write(observation.as_bytes());
    if returned != Ok(DomainReturn::Fault(DomainFault::InvalidInstruction))
        || origin != 1
        || value != 0x107
    {
        refuse("x87-nm-vector-or-return");
    }
    let mut output = [0u8; 256];
    if domain.enter(1).is_ok()
        || domain.input(b"replay").is_ok()
        || domain.output(&mut output).is_ok()
        || domain.cost().scheduler_returns != 1
    {
        refuse("x87-nm-not-terminal");
    }
    if unsafe { (&root_sentinel as *const u64).read_volatile() } != 0x6e6d_726f_6f74_5a5a
        || unsafe { (sibling_address as *const u32).read_volatile() } != sibling_before
    {
        refuse("x87-root-sibling-changed");
    }
    if sibling.enter(1) != Ok(DomainReturn::Yielded) {
        refuse("x87-sibling-return");
    }
    let length = sibling
        .output(&mut output)
        .unwrap_or_else(|_| refuse("x87-sibling-output"));
    if &output[..length] != b"NM SIBLING" {
        refuse("x87-sibling-value");
    }
    arch::early_write(
        b"CONDUIT_X87_NM_CONTROL vector=7 origin=1 terminal=true root-sibling-survived=true\n",
    );
}
