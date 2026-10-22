//! Fixed proof-appliance baseline, not calibrated time or a product benchmark.
use super::{IMAGE, exchange, installed, request};
use crate::ordinary_domain_baseline_format::{self as format, RECORDS, Sample, TRIALS};
use crate::{
    arch,
    protected_region::{DomainBackend, DomainFault, DomainReturn},
    service_memory::ServiceImage,
};
#[path = "../../native_observation/raw_counter.rs"]
mod counter;

fn elapsed(start: u64) -> u64 {
    counter::read()
        .checked_sub(start)
        .unwrap_or_else(|| super::refuse("baseline-clock-regressed"))
}
fn install(image: &ServiceImage<'_>) -> (arch::ServiceDomain, u64) {
    let start = counter::read();
    let domain = installed(image);
    (domain, elapsed(start))
}
fn quarantine(domain: &mut arch::ServiceDomain) -> u64 {
    let start = counter::read();
    domain.quarantine();
    elapsed(start)
}
fn sample(domain: &mut arch::ServiceDomain, mode: u8, case: u8, trial: usize) -> Sample {
    let payload = [b'a'; super::frame::TEXT_CAPACITY];
    let length = match case {
        0 | 3 | 4 => 0,
        1 => 8,
        2 => payload.len(),
        _ => unreachable!(),
    };
    let probe = match case {
        3 => 6,
        4 => 5,
        _ => 0,
    };
    let start = counter::read();
    request(domain, probe, 0, &payload[..length]);
    if case == 0 {
        // Existing terminal gate command: no uppercase or capability request.
        let frame = exchange(domain);
        frame.command = 2;
        frame.status = 0;
    }
    let request_ticks = elapsed(start);
    let before = domain.cost();
    let start = counter::read();
    let result = domain.enter(1);
    let entry = elapsed(start);
    let expected = match case {
        3 => DomainReturn::Fault(DomainFault::WorkExhausted),
        4 => DomainReturn::Fault(DomainFault::PrivilegedOperation),
        _ => DomainReturn::Yielded,
    };
    if result != Ok(expected) {
        super::refuse("baseline-return");
    }
    let after = domain.cost();
    let mut response = 0;
    if case < 3 {
        let mut output = [0; super::frame::TEXT_CAPACITY];
        let start = counter::read();
        let frame = exchange(domain);
        if frame.status != 0 || frame.output_length as usize != length {
            super::refuse("baseline-length");
        }
        output[..length].copy_from_slice(&frame.output[..length]);
        core::hint::black_box(&output[..length]);
        response = elapsed(start);
        if output[..length].iter().any(|byte| *byte != b'A') {
            super::refuse("baseline-payload");
        }
    } else if domain.exchange().is_ok() || domain.enter(1).is_ok() {
        super::refuse("baseline-fault-not-terminal");
    }
    Sample {
        mode,
        case,
        trial,
        instance: domain.instance(),
        request: request_ticks,
        entry,
        response,
        interrupts: after.interrupt_entries - before.interrupt_entries,
        preemptions: after.preemptions - before.preemptions,
        ..Sample::default()
    }
}
pub(super) fn run() {
    let image = ServiceImage::compositor(IMAGE).unwrap_or_else(|_| super::refuse("baseline-image"));
    let mut rows = [Sample::default(); RECORDS];
    let mut next = 0;
    let mut overhead = [0u64; TRIALS];
    for value in &mut overhead {
        let start = counter::read();
        *value = elapsed(start);
    }
    for case in 0..3 {
        for trial in 0..TRIALS {
            let (mut domain, setup) = install(&image);
            let mut row = sample(&mut domain, 0, case, trial);
            row.setup = setup;
            row.teardown = quarantine(&mut domain);
            rows[next] = row;
            next += 1;
        }
        let (mut domain, setup) = install(&image);
        for trial in 0..TRIALS {
            let mut row = sample(&mut domain, 1, case, trial);
            if trial == 0 {
                row.setup = setup;
            }
            if trial == TRIALS - 1 {
                row.teardown = quarantine(&mut domain);
            }
            rows[next] = row;
            next += 1;
        }
    }
    for case in 3..5 {
        let (mut domain, setup) = install(&image);
        let mut row = sample(&mut domain, 0, case, 0);
        row.setup = setup;
        // Fault entry already includes automatic quarantine. This is only the
        // subsequent idempotent call; do not subtract it from entry time.
        row.teardown = quarantine(&mut domain);
        rows[next] = row;
        next += 1;
    }
    if next != RECORDS {
        super::refuse("baseline-count");
    }
    // No serial or formatting occurs inside any measured interval.
    let basic = core::arch::x86_64::__cpuid(1);
    let extended = core::arch::x86_64::__cpuid(0x8000_0000).eax;
    let invariant =
        extended >= 0x8000_0007 && core::arch::x86_64::__cpuid(0x8000_0007).edx & (1 << 8) != 0;
    let serial = format::header([basic.eax, basic.ecx, basic.edx], invariant, overhead)
        .unwrap_or_else(|_| super::refuse("baseline-serial"));
    arch::early_write(serial.as_bytes());
    for row in rows {
        let serial = format::sample(row).unwrap_or_else(|_| super::refuse("baseline-serial"));
        arch::early_write(serial.as_bytes());
    }
}
