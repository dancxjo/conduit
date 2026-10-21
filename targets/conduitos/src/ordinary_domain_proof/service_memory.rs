//! Backend fixture evidence, separate from production compositor isolation.
use super::refuse;
use crate::{
    arch,
    protected_region::{DomainBackend, DomainFault, DomainRefusal, DomainReturn},
    service_memory::{MemoryRange, ServiceImage, ServiceMemoryLayout},
};
#[allow(dead_code)]
#[path = "../../domain/frame.rs"]
mod frame;
use frame::TextFrame;

const IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/domain.elf"));
const VALUE: u64 = 0x6f31_9a82_bc47_d205;

fn installed(image: &ServiceImage<'_>) -> arch::ServiceDomain {
    arch::ServiceDomain::install(image).unwrap_or_else(|_| refuse("service-install"))
}
fn exchange(domain: &mut arch::ServiceDomain) -> &mut TextFrame {
    let bytes = domain
        .exchange()
        .unwrap_or_else(|_| refuse("service-exchange"));
    if bytes.len() < core::mem::size_of::<TextFrame>()
        || bytes
            .as_ptr()
            .align_offset(core::mem::align_of::<TextFrame>())
            != 0
    {
        refuse("service-frame-layout");
    }
    unsafe { &mut *bytes.as_mut_ptr().cast::<TextFrame>() }
}
fn request(domain: &mut arch::ServiceDomain, probe: u32, target: u64, input: &[u8]) {
    let frame = exchange(domain);
    // Do not allocate a complete frame on the guest or Root stack.
    frame.input.fill(0);
    frame.output.fill(0);
    frame.input[..input.len()].copy_from_slice(input);
    frame.input_length = input.len() as u32;
    frame.output_length = 0;
    frame.capacity = frame::TEXT_CAPACITY as u32;
    frame.status = 3;
    frame.command = 0;
    frame.probe = probe;
    frame.target = target;
}
fn success(domain: &mut arch::ServiceDomain) {
    if domain.enter(1) != Ok(DomainReturn::Yielded) || exchange(domain).status != 0 {
        refuse("service-positive-entry");
    }
}
fn read(domain: &mut arch::ServiceDomain, address: u64) -> u64 {
    request(domain, 19, address, &[]);
    success(domain);
    let frame = exchange(domain);
    if frame.output_length != 8 {
        refuse("service-read-length");
    }
    let mut value = [0; 8];
    value.copy_from_slice(&frame.output[..8]);
    u64::from_le_bytes(value)
}
fn edges(range: MemoryRange) -> [u64; 2] {
    [range.start(), range.end() - 8]
}
fn private_ranges(layout: ServiceMemoryLayout) -> [MemoryRange; 5] {
    [
        layout.output(),
        layout.stack(),
        layout.metadata(),
        layout.pixels().expect("compositor pixel range"),
        layout.scratch().expect("compositor scratch range"),
    ]
}
fn sentinel(address: u64) -> u32 {
    unsafe { (address as *const u32).read_volatile() }
}

pub(super) fn run() {
    let image = ServiceImage::compositor(IMAGE).unwrap_or_else(|_| refuse("service-image"));
    let layout = image.layout();
    let root_sentinel = VALUE;
    let root_address = &root_sentinel as *const u64 as u64;
    let mut sibling = arch::TextDomain::install().unwrap_or_else(|_| refuse("service-sibling"));
    sibling
        .input(b"sibling-sentinel")
        .unwrap_or_else(|_| refuse("service-sibling-input"));
    let sibling_address = sibling.private_frame_address();
    let sibling_before = sentinel(sibling_address);
    let first_instance;
    {
        let mut service = installed(&image);
        first_instance = service.instance();
        if arch::ServiceDomain::install(&image).is_ok() {
            refuse("service-slot-cap");
        }
        if service.cost().reserved_bytes < layout.mapped_bytes() as u32 {
            refuse("service-memory-charge");
        }
        request(&mut service, 0, 0, b"service uppercase");
        success(&mut service);
        let frame = exchange(&mut service);
        if frame.output_length != 17 || &frame.output[..17] != b"SERVICE UPPERCASE" {
            refuse("service-uppercase");
        }
        request(&mut service, 20, 0, &[]);
        success(&mut service);
        if exchange(&mut service).output[..8] != 3u64.to_le_bytes() {
            refuse("service-cpl3");
        }
        // Root stages a full bounded upload window while the guest is stopped;
        // protected reads observe both ends across separate entries.
        service
            .transfer_window()
            .unwrap_or_else(|_| refuse("service-transfer-window"))
            .fill(0xa5);
        for address in edges(layout.output()) {
            if read(&mut service, address) != 0xa5a5_a5a5_a5a5_a5a5 {
                refuse("service-transfer-not-visible");
            }
        }
        for range in private_ranges(layout) {
            // Stack contents change during entry; verify its mapped edge pages
            // with writes and readback only at the unused bottom boundary.
            let addresses = if range == layout.stack() {
                [range.start(), range.start() + 8]
            } else {
                edges(range)
            };
            for address in addresses {
                request(&mut service, 18, address, &VALUE.to_le_bytes());
                success(&mut service);
                if read(&mut service, address) != VALUE {
                    refuse("service-retained-write");
                }
            }
        }
        // Separate entries retained both full pixel-region boundaries.
        for range in [
            layout.pixels().expect("compositor pixel range"),
            layout.scratch().expect("compositor scratch range"),
        ] {
            for address in edges(range) {
                if read(&mut service, address) != VALUE {
                    refuse("service-retained-state");
                }
            }
        }
    }
    let mut previous = first_instance;
    let mut attempts = 0u32;
    let mut hostile = |probe, address, expected| {
        let mut service = installed(&image);
        if service.instance() <= previous {
            refuse("service-instance-reused");
        }
        previous = service.instance();
        for range in [
            layout.metadata(),
            layout.pixels().expect("compositor pixel range"),
            layout.scratch().expect("compositor scratch range"),
        ] {
            for address in edges(range) {
                if read(&mut service, address) != 0 {
                    refuse("service-fresh-not-zero");
                }
                request(&mut service, 18, address, &VALUE.to_le_bytes());
                success(&mut service);
            }
        }
        request(&mut service, probe, address, &VALUE.to_le_bytes());
        let returned = service.enter(1);
        let flags: u64;
        unsafe { core::arch::asm!("pushfq", "pop {}", out(reg) flags, options(preserves_flags)) };
        if flags & (1 << 10) != 0 {
            refuse("service-root-direction-flag");
        }
        if (probe == 6 || probe == 17) && service.cost().preemptions != 1 {
            refuse("service-loop-not-preempted");
        }
        if returned != Ok(DomainReturn::Fault(expected))
            || service.enter(1) != Err(DomainRefusal::InvalidLifecycle)
            || service.exchange().is_ok()
            || service.output().is_ok()
            || service.transfer_window().is_ok()
            || u64::from(service.cost().teardown_zeroed_bytes) < layout.mapped_bytes()
        {
            refuse("service-hostile-not-contained");
        }
        if unsafe { (root_address as *const u64).read_volatile() } != VALUE
            || sentinel(sibling_address) != sibling_before
        {
            refuse("service-root-or-sibling-changed");
        }
        attempts += 1;
    };
    for address in [root_address, sibling_address, 0xfee0_0000] {
        hostile(1, address, DomainFault::Memory);
        hostile(2, address, DomainFault::Memory);
    }
    // Image text and constants cannot be written; private pixels cannot execute.
    for segment in &image.image().segments {
        hostile(2, segment.address, DomainFault::Memory);
    }
    for range in [
        layout.exchange(),
        layout.output(),
        layout.stack(),
        layout.metadata(),
        layout.pixels().expect("compositor pixel range"),
        layout.scratch().expect("compositor scratch range"),
    ] {
        hostile(3, range.start(), DomainFault::Memory);
    }
    // Both outer guards and all six interior guard gaps remain unmapped.
    hostile(1, layout.image().start() - 4096, DomainFault::Memory);
    for range in [
        layout.image(),
        layout.exchange(),
        layout.output(),
        layout.stack(),
        layout.metadata(),
        layout.pixels().expect("compositor pixel range"),
        layout.scratch().expect("compositor scratch range"),
    ] {
        hostile(2, range.end(), DomainFault::Memory);
    }
    hostile(4, 0, DomainFault::PrivilegedOperation);
    hostile(6, 0, DomainFault::WorkExhausted);
    hostile(17, 0, DomainFault::WorkExhausted);
    hostile(u32::MAX, 0, DomainFault::InvalidGate);
    {
        let mut restarted = installed(&image);
        if restarted.instance() <= previous {
            refuse("service-restart-identity");
        }
        if restarted
            .output()
            .unwrap_or_else(|_| refuse("service-restart-output"))
            .iter()
            .any(|byte| *byte != 0)
        {
            refuse("service-restart-output-not-zero");
        }
        // Boundary samples plus the backend's charged whole-arena zero count.
        for range in [
            layout.metadata(),
            layout.pixels().expect("compositor pixel range"),
            layout.scratch().expect("compositor scratch range"),
        ] {
            for address in edges(range) {
                if read(&mut restarted, address) != 0 {
                    refuse("service-restart-not-zero");
                }
            }
        }
    }
    if sentinel(sibling_address) != sibling_before {
        refuse("service-sibling-corrupt");
    }
    sibling
        .enter(1)
        .unwrap_or_else(|_| refuse("service-sibling-entry"));
    let mut output = [0; 32];
    let count = sibling
        .output(&mut output)
        .unwrap_or_else(|_| refuse("service-sibling-output"));
    if &output[..count] != b"SIBLING-SENTINEL" || attempts != 26 {
        refuse("service-proof-count-or-sibling");
    }
    arch::early_write(b"CONDUIT_SERVICE_MEMORY_PROOF {\"schema\":\"conduit.conduitos/service-memory-fixture@1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"fixture\":true,\"architecture\":\"x86_64\",\"privilege\":\"ring3\",\"hostile_entries\":26,\"retained_bytes\":8388608,\"scratch_bytes\":8388608,\"restart_boundary_samples_zero\":true,\"production_compositor\":false,\"driver_isolation\":false,\"dma_isolation\":false}\n");
}
