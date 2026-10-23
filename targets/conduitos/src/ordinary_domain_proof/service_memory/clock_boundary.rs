//! Finite cross-privilege envelopes, not pure hardware trap/IRET latency.
use super::*;
use crate::sign_format::FixedText;
use core::{
    fmt::Write,
    sync::atomic::{AtomicU64, Ordering},
};
static SEEN: AtomicU64 = AtomicU64::new(0);
pub(crate) fn observe(raw: arch::ClockBoundaryObservation, value: u64, origin: u64) {
    let ordered = raw
        .stamps
        .windows(2)
        .all(|pair| pair[0] != 0 && pair[1] >= pair[0]);
    let valid = raw.completed && raw.root_cr4_preserved && value == 0 && origin == 0 && ordered;
    if raw.row >= 18 || SEEN.fetch_or(1_u64 << raw.row, Ordering::AcqRel) & (1_u64 << raw.row) != 0
    {
        refuse("boundary-clock-duplicate-row");
    }
    let mut text = FixedText::new();
    let mut hash = FixedText::new();
    for byte in raw.digest {
        write!(hash, "{byte:02x}").unwrap_or_else(|_| refuse("boundary-clock-hash"));
    }
    let digest =
        core::str::from_utf8(hash.as_bytes()).unwrap_or_else(|_| refuse("boundary-clock-hash"));
    write!(text,"CONDUIT_SERVICE_PRIVILEGE_BOUNDARY {{\"schema\":\"conduit.conduitos/service-privilege-boundary@1\",\"row\":{},\"instance\":{},\"guest_sha256\":\"{}\",\"value\":{},\"origin\":{},\"valid\":{},\"root_cr4_preserved\":{},\"stamps\":[{},{},{},{}],\"root_to_user_ticks\":",
        raw.row,raw.instance,digest,value,origin,valid,raw.root_cr4_preserved,raw.stamps[0],raw.stamps[1],raw.stamps[2],raw.stamps[3]).unwrap_or_else(|_|refuse("boundary-clock-line-bound"));
    let mut ticks = |label: &str, value: Option<u64>| {
        text.write_str(label)
            .unwrap_or_else(|_| refuse("boundary-clock-line-bound"));
        match value {
            Some(value) => write!(text, "{value}"),
            None => text.write_str("null"),
        }
        .unwrap_or_else(|_| refuse("boundary-clock-line-bound"));
    };
    ticks("", valid.then(|| raw.stamps[1] - raw.stamps[0]));
    ticks(
        ",\"user_to_root_ticks\":",
        valid.then(|| raw.stamps[3] - raw.stamps[2]),
    );
    write!(
        text,
        ",\"budget_interrupts\":{},\"source_interrupts\":{},\"interrupts_included\":true",
        raw.user_interrupts.0, raw.user_interrupts.1
    )
    .unwrap_or_else(|_| refuse("boundary-clock-line-bound"));
    text.write_str(",\"clock\":\"uncalibrated-raw-tsc\",\"overhead_included\":true,\"pure_hardware\":false,\"guest_clock_permission\":\"exact-image-one-entry\",\"guest_timestamp_storage\":\"reserved-r12-r13\"}\n").unwrap_or_else(|_|refuse("boundary-clock-line-bound"));
    arch::early_write(text.as_bytes());
    if !valid {
        refuse("boundary-clock-invalid");
    }
}
fn sample(domain: &mut arch::ServiceDomain, row: usize) {
    let length = [0, 8, frame::TEXT_CAPACITY][row / 6];
    request(domain, 22, 0, &[b'a'; frame::TEXT_CAPACITY][..length]);
    domain
        .arm_clock_fixture(row)
        .unwrap_or_else(|_| refuse("boundary-clock-permission"));
    if domain.enter(1) != Ok(DomainReturn::Yielded) {
        refuse("boundary-clock-return");
    }
    let frame = exchange(domain);
    if frame.status != 0
        || frame.output_length as usize != length
        || frame.output[..length].iter().any(|v| *v != b'A')
    {
        refuse("boundary-clock-payload");
    }
}
pub(super) fn run() {
    for case in 0..3 {
        for trial in 0..3 {
            let mut domain = arch::ServiceDomain::install_clock_fixture()
                .unwrap_or_else(|_| refuse("boundary-clock-install"));
            sample(&mut domain, case * 6 + trial);
        }
        let mut reused = arch::ServiceDomain::install_clock_fixture()
            .unwrap_or_else(|_| refuse("boundary-clock-install"));
        for trial in 0..3 {
            sample(&mut reused, case * 6 + 3 + trial);
        }
    }
    let mut unarmed = arch::ServiceDomain::install_clock_fixture()
        .unwrap_or_else(|_| refuse("boundary-clock-control-install"));
    let root_cr4 = arch::ServiceDomain::proof_root_cr4();
    request(&mut unarmed, 22, 0, &[]);
    if unarmed.enter(1) != Ok(DomainReturn::Fault(DomainFault::PrivilegedOperation))
        || unarmed.enter(1).is_ok()
        || unarmed.arm_clock_fixture(0).is_ok()
        || arch::ServiceDomain::proof_root_cr4() != root_cr4
        || SEEN.load(Ordering::Acquire) != (1 << 18) - 1
    {
        refuse("boundary-clock-unarmed-control");
    }
    drop(unarmed);
    let image = ServiceImage::compositor(IMAGE).unwrap_or_else(|_| refuse("boundary-clock-image"));
    let mut ordinary = installed(&image);
    request(&mut ordinary, 22, 0, &[]);
    if ordinary.arm_clock_fixture(0).is_ok()
        || ordinary.enter(1) != Ok(DomainReturn::Fault(DomainFault::PrivilegedOperation))
        || arch::ServiceDomain::proof_root_cr4() != root_cr4
    {
        refuse("boundary-clock-generic-install-authority");
    }
    arch::early_write(b"CONDUIT_SERVICE_PRIVILEGE_BOUNDARY_CONTROL {\"schema\":\"conduit.conduitos/service-privilege-boundary-control@1\",\"rows\":18,\"unarmed_clock_traps\":true,\"old_instance_terminal\":true,\"generic_install_ineligible\":true,\"generic_install_clock_traps\":true,\"fault_root_cr4_preserved\":true}\n");
}
