//! Twenty finite architecture-fixture rows, not a product latency benchmark.
use super::{IMAGE, exchange, installed, request};
use crate::{
    arch,
    protected_region::{DomainBackend, DomainFault, DomainReturn},
    service_memory::ServiceImage,
};
use core::sync::atomic::{AtomicU64, Ordering};
mod wire;

// Single CPU, one ACTIVE domain entry. Zero disarms; only the bounded fixture
// arms a row. Unrelated existing ordinary-proof entries remain unreported.
static ARMED: AtomicU64 = AtomicU64::new(0);
static INSTANCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn observe(value: u64, origin: u64, raw: [u64; 14]) {
    let armed = ARMED.swap(0, Ordering::AcqRel);
    if armed == 0 {
        return;
    }
    let row = usize::try_from(armed - 1).unwrap_or(usize::MAX);
    let record = wire::sample(row, INSTANCE.load(Ordering::Acquire), value, origin, raw)
        .unwrap_or_else(|_| super::refuse("trap-envelope-format"));
    arch::early_write(record.as_bytes());
}

fn sample(domain: &mut arch::ServiceDomain, row: usize) {
    let (_, case, _) = wire::position(row).unwrap_or_else(|| super::refuse("trap-envelope-row"));
    let payload = [b'a'; super::frame::TEXT_CAPACITY];
    let length = match case {
        1 => 8,
        2 => payload.len(),
        _ => 0,
    };
    request(
        domain,
        match case {
            3 => 6,
            4 => 5,
            _ => 0,
        },
        0,
        &payload[..length],
    );
    if case == 0 {
        let frame = exchange(domain);
        frame.command = 2;
        frame.status = 0;
    }
    INSTANCE.store(domain.instance(), Ordering::Release);
    ARMED.store(row as u64 + 1, Ordering::Release);
    let returned = domain.enter(1);
    if ARMED.swap(0, Ordering::AcqRel) != 0 {
        super::refuse("trap-envelope-missing-return");
    }
    let expected = match case {
        3 => DomainReturn::Fault(DomainFault::WorkExhausted),
        4 => DomainReturn::Fault(DomainFault::PrivilegedOperation),
        _ => DomainReturn::Yielded,
    };
    if returned != Ok(expected) {
        super::refuse("trap-envelope-return");
    }
    if case < 3 {
        let frame = exchange(domain);
        if frame.status != 0
            || frame.output_length as usize != length
            || frame.output[..length].iter().any(|byte| *byte != b'A')
        {
            super::refuse("trap-envelope-payload");
        }
    } else if domain.exchange().is_ok() || domain.enter(1).is_ok() {
        super::refuse("trap-envelope-fault-not-terminal");
    }
}

pub(super) fn run() {
    let image =
        ServiceImage::compositor(IMAGE).unwrap_or_else(|_| super::refuse("trap-envelope-image"));
    for case in 0..3 {
        for trial in 0..3 {
            let mut domain = installed(&image);
            sample(&mut domain, case * 6 + trial);
        }
        let mut domain = installed(&image);
        for trial in 0..3 {
            sample(&mut domain, case * 6 + 3 + trial);
        }
    }
    for row in 18..20 {
        let mut domain = installed(&image);
        sample(&mut domain, row);
    }
}
