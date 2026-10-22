//! Actual stopped transfer-window copies and full-span CPL3 work; proof only.
use super::{IMAGE, exchange, installed, refuse, request};
use crate::{
    arch,
    protected_region::{DomainBackend, DomainRefusal, DomainReturn},
    service_memory::ServiceImage,
};
use alloc::vec::Vec;
#[path = "../../../domain/service_bulk.rs"]
mod bulk;
#[path = "../../native_observation/raw_counter.rs"]
mod counter;
mod format;
const SENTINEL: u8 = 0xd3;
fn elapsed(start: u64) -> u64 {
    counter::read()
        .checked_sub(start)
        .unwrap_or_else(|| refuse("staged-clock-regressed"))
}
fn stage(
    domain: &mut arch::ServiceDomain,
    descriptor: bulk::Descriptor,
    next: u64,
    input: &[u8],
) -> Result<(), u32> {
    descriptor
        .admit(domain.instance(), next)
        .map_err(|reason| reason as u32)?;
    let start = descriptor.offset as usize;
    let length = descriptor.length as usize;
    let target = domain.transfer_window().map_err(|_| 7u32)?;
    target
        .get_mut(start..start + length)
        .ok_or(1u32)?
        .copy_from_slice(input.get(..length).ok_or(1u32)?);
    Ok(())
}
fn sample(
    domain: &mut arch::ServiceDomain,
    mode: u8,
    case: usize,
    trial: usize,
    next: u64,
    input: &mut [u8],
    output: &mut [u8],
) -> format::Row {
    let length = format::LENGTHS[case];
    let descriptor = bulk::Descriptor::new(domain.instance(), next, length);
    for (index, byte) in input[..length].iter_mut().enumerate() {
        *byte = (index as u8).wrapping_mul(17).wrapping_add(next as u8);
    }
    // Preconditioning warms both finite windows outside all intervals. These
    // component observations are not cold-cache or total transaction timings.
    output.fill(0);
    domain
        .transfer_window()
        .unwrap_or_else(|_| refuse("staged-window"))
        .fill(SENTINEL);
    request(domain, 22, 0, &descriptor.encode());
    let start = counter::read();
    stage(domain, descriptor, next, input).unwrap_or_else(|_| refuse("staged-admission"));
    let stage_ticks = elapsed(start);
    let start = counter::read();
    let result = domain.enter(1);
    let entry = elapsed(start);
    // Preserve the existing hard budget; a span that cannot finish refuses.
    if result != Ok(DomainReturn::Yielded) {
        refuse("staged-entry-or-budget-refusal");
    }
    let start = counter::read();
    let frame = exchange(domain);
    if frame.status != 0
        || frame.output_length as usize != bulk::BYTES
        || frame.capacity != 256
        || frame.probe != 22
        || frame.command != 0
        || bulk::Descriptor::decode(&frame.output[..bulk::BYTES]) != Ok(descriptor.returned())
    {
        refuse("staged-return-correlation");
    }
    output[..length].copy_from_slice(
        &domain
            .output()
            .unwrap_or_else(|_| refuse("staged-return-window"))[..length],
    );
    if output[..length]
        .iter()
        .zip(&input[..length])
        .any(|(actual, original)| *actual != (*original ^ bulk::XOR))
    {
        refuse("staged-full-output-mismatch");
    }
    let returned = elapsed(start);
    if domain
        .output()
        .unwrap_or_else(|_| refuse("staged-tail-window"))[length..]
        .iter()
        .any(|byte| *byte != SENTINEL)
    {
        refuse("staged-tail-changed");
    }
    format::Row {
        mode,
        case,
        trial,
        length,
        instance: domain.instance(),
        request: next,
        stage: stage_ticks,
        entry,
        returned,
    }
}
fn negatives(image: &ServiceImage<'_>, previous: u64, input: &[u8]) {
    let mut domain = installed(image);
    let instance = domain.instance();
    let next = 31;
    if instance <= previous {
        refuse("staged-negative-instance");
    }
    domain
        .transfer_window()
        .unwrap_or_else(|_| refuse("staged-negative-window"))
        .fill(SENTINEL);
    let base = bulk::Descriptor::new(instance, next, 0);
    let changes = [
        bulk::Descriptor {
            length: 65537,
            ..base
        },
        bulk::Descriptor {
            offset: u64::MAX,
            length: 1,
            ..base
        },
        bulk::Descriptor {
            start: bulk::WINDOW + 1,
            ..base
        },
        base.returned(),
        bulk::Descriptor {
            instance: previous,
            ..base
        },
        bulk::Descriptor {
            request: next - 1,
            ..base
        },
    ];
    for (case, descriptor) in changes.into_iter().enumerate() {
        let before = domain.cost().entries;
        if stage(&mut domain, descriptor, next, input) != Err((case + 1) as u32)
            || domain.cost().entries != before
            || domain
                .output()
                .unwrap_or_else(|_| refuse("staged-refusal-window"))
                .iter()
                .any(|byte| *byte != SENTINEL)
        {
            refuse("staged-refusal-mutated");
        }
        let text = format::refusal(
            case,
            instance,
            next,
            descriptor.instance,
            descriptor.request,
            (case + 1) as u32,
        )
        .unwrap_or_else(|_| refuse("staged-serial"));
        arch::early_write(text.as_bytes());
    }
    // Deliberately bypass Root admission for one actual guest syntax refusal.
    request(&mut domain, 22, 0, &base.returned().encode());
    if domain.enter(1) != Ok(DomainReturn::Yielded) {
        refuse("staged-guest-refusal-entry");
    }
    let frame = exchange(&mut domain);
    if frame.status != 0x104
        || frame.output_length != 0
        || domain
            .output()
            .unwrap_or_else(|_| refuse("staged-guest-refusal-window"))
            .iter()
            .any(|byte| *byte != SENTINEL)
    {
        refuse("staged-guest-refusal-output");
    }
    let text = format::refusal(6, instance, next, instance, next, 4)
        .unwrap_or_else(|_| refuse("staged-serial"));
    arch::early_write(text.as_bytes());
    domain.quarantine();
    if stage(&mut domain, base, next, input) != Err(7)
        || domain.transfer_window().is_ok()
        || domain.output().is_ok()
        || domain.enter(1) != Err(DomainRefusal::InvalidLifecycle)
    {
        refuse("staged-quarantine");
    }
    let text = format::refusal(7, instance, next, instance, next, 7)
        .unwrap_or_else(|_| refuse("staged-serial"));
    arch::early_write(text.as_bytes());
}
pub(super) fn run() {
    let image = ServiceImage::compositor(IMAGE).unwrap_or_else(|_| refuse("staged-image"));
    if image.layout().output().start() != bulk::WINDOW
        || image.layout().output().bytes() != bulk::CAPACITY as u64
    {
        refuse("staged-exact-profile");
    }
    // Exactly two finite Root buffers, prepared before all measured intervals;
    // allocator overhead and domain reserved bytes are separate, not total heap.
    let mut input = Vec::with_capacity(bulk::CAPACITY);
    input.resize(bulk::CAPACITY, 0);
    let mut output = Vec::with_capacity(bulk::CAPACITY);
    output.resize(bulk::CAPACITY, 0);
    if input.capacity() + output.capacity() != format::ROOT_WORKSPACE {
        refuse("staged-workspace");
    }
    let mut rows = [format::Row::default(); format::RECORDS];
    let mut index = 0;
    let mut reserved = 0;
    for case in 0..format::LENGTHS.len() {
        for trial in 0..3 {
            let mut domain = installed(&image);
            reserved = domain.cost().reserved_bytes;
            rows[index] = sample(
                &mut domain,
                0,
                case,
                trial,
                index as u64 + 1,
                &mut input,
                &mut output,
            );
            index += 1;
        }
        let mut domain = installed(&image);
        for trial in 0..3 {
            rows[index] = sample(
                &mut domain,
                1,
                case,
                trial,
                index as u64 + 1,
                &mut input,
                &mut output,
            );
            index += 1;
        }
    }
    if index != format::RECORDS {
        refuse("staged-count");
    }
    let header = format::header(reserved).unwrap_or_else(|_| refuse("staged-serial"));
    arch::early_write(header.as_bytes());
    let previous = rows[format::RECORDS - 1].instance;
    for row in rows {
        let text = format::row(row).unwrap_or_else(|_| refuse("staged-serial"));
        arch::early_write(text.as_bytes());
    }
    negatives(&image, previous, &input);
}
