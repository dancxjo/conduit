#![no_std]
#![no_main]

#[cfg(not(all(target_arch = "aarch64", feature = "aarch64-jetson-nano")))]
compile_error!("the Jetson Nano ConduitOS Host requires AArch64 and its exact board feature");

use core::panic::PanicInfo;

use conduit_core::{BootId, HostId, OfferGeneration};
use conduitos::{
    allocation::BOOT_ARENA,
    arch, dual_region_composition, dual_region_plan,
    front_door::FrontDoor,
    identity, keyboard_text_plan,
    linear_presenter::LinearPresenter,
    make::{EMBEDDED_MAKE, IMPL_LINEAR_PRESENTER},
    offer::CpuFeatures,
    offer_make::ImageBoundHostOffer,
};

const ARENA_BYTES: usize = 8 * 1024 * 1024;

#[repr(C, align(4096))]
struct RuntimeArena([u8; ARENA_BYTES]);

static mut RUNTIME_ARENA: RuntimeArena = RuntimeArena([0; ARENA_BYTES]);

#[unsafe(no_mangle)]
pub extern "C" fn conduitos_aarch64_jetson_nano_start(dtb_address: u64) -> ! {
    validate_handoff(dtb_address);
    boot_diagnostic("CONDUITOS STARTING");
    arch::enable_fp_simd();
    arch::initialize_machine();
    let arena = core::ptr::addr_of_mut!(RUNTIME_ARENA).cast::<u8>() as usize;
    unsafe {
        BOOT_ARENA
            .initialize(arena, ARENA_BYTES)
            .unwrap_or_else(|_| refuse("runtime-arena-initialization-failed"));
    }
    EMBEDDED_MAKE
        .validate(ARENA_BYTES as u64)
        .unwrap_or_else(|error| refuse(error.as_str()));
    if !EMBEDDED_MAKE.includes(IMPL_LINEAR_PRESENTER) {
        refuse("linear-presenter-absent-from-image");
    }

    let counter = arch::read_counter();
    let identities = identity::derive(
        [
            counter,
            counter.rotate_left(13),
            counter.rotate_left(29),
            counter.rotate_left(47),
        ],
        0,
        0xf008_0000,
    );
    let offer = ImageBoundHostOffer::new(
        &identities,
        &EMBEDDED_MAKE,
        CpuFeatures {
            sse2: false,
            rdrand: false,
            invariant_tsc: false,
        },
        ARENA_BYTES as u64,
    )
    .unwrap_or_else(|error| refuse(error.as_str()));
    offer
        .validate()
        .unwrap_or_else(|error| refuse(error.as_str()));

    let host_id = HostId::from(identity::hex(&identities.host));
    let boot_id = BootId::from(identity::hex(&identities.boot));
    let generation = OfferGeneration(offer.generation);
    let plot =
        keyboard_text_plan::checked_plot_identity().unwrap_or_else(|error| refuse(error.as_str()));
    let front_door = FrontDoor::new(
        host_id.clone(),
        boot_id.clone(),
        generation,
        EMBEDDED_MAKE.profile_id,
        EMBEDDED_MAKE.build_id,
        EMBEDDED_MAKE.image_binding,
        plot.source_document_id,
        plot.checked_plot_id,
        5,
        false,
    );
    let face = front_door
        .face()
        .unwrap_or_else(|error| refuse(error.as_str()));
    let mut presenter = LinearPresenter::prepare(
        host_id,
        boot_id,
        generation,
        EMBEDDED_MAKE.profile_id,
        EMBEDDED_MAKE.image_binding,
    )
    .unwrap_or_else(|_| refuse("linear-presenter-plan-refused"));
    let receipt = presenter
        .present(&face)
        .unwrap_or_else(|_| refuse("linear-presenter-manifestation-refused"));
    for line in &receipt.face.lines {
        arch::present(b"CONDUIT_LINEAR_FACE ");
        arch::present(line.as_bytes());
        arch::present(b"\n");
    }

    let mut prepared = dual_region_plan::prepare(&identities, &offer, EMBEDDED_MAKE.build_id)
        .unwrap_or_else(|error| refuse(error.as_str()));
    let mut clock = arch::Clock::new();
    let mut timer = arch::Timer::new();
    let mut serial = arch::Serial::new();
    let mut interrupts = arch::Interrupts::new();
    let mut idle = arch::Idle::new();
    dual_region_composition::run(
        &mut prepared.kernel,
        &mut clock,
        &mut timer,
        &mut serial,
        &mut interrupts,
        &mut idle,
    )
    .unwrap_or_else(|error| refuse(error.as_str()));

    if !arch::jetson_nano::machine::serial_healthy() {
        refuse("serial-output-unavailable");
    }

    arch::present(b"CONDUIT_JETSON_NANO_PRODUCT {\"schema\":\"conduit.conduitos/jetson-nano-product@1\",\"status\":\"ready\",\"target\":\"conduitos/aarch64/jetson-nano-tegra210\",\"architecture\":\"aarch64\",\"machine\":\"tegra210\",\"ordinary_plan_id\":\"");
    arch::present(prepared.plan.plan_id.as_str().as_bytes());
    arch::present(b"\",\"ordinary_play_id\":\"");
    arch::present(prepared.active_play.active_play_id.as_str().as_bytes());
    arch::present(
        b"\",\"semantic_result\":\"HELLO, CONDUITOS\",\"physical_proof_claimed\":false}\n",
    );
    arch::present(b"CONDUIT_BOOT_STAGE jetson-nano-product-ready\n");
    boot_diagnostic("CONDUITOS READY\nHELLO CONDUITOS\nTEXT AND TIMER COMPLETE");
    loop {
        core::hint::spin_loop();
    }
}

fn refuse(reason: &str) -> ! {
    arch::disable_interrupts();
    let mut message = [0u8; 192];
    let prefix = b"CONDUITOS BOOT REFUSED\n";
    message[..prefix.len()].copy_from_slice(prefix);
    let bytes = reason.as_bytes();
    let count = bytes.len().min(message.len() - prefix.len());
    message[prefix.len()..prefix.len() + count].copy_from_slice(&bytes[..count]);
    if let Ok(text) = core::str::from_utf8(&message[..prefix.len() + count]) {
        boot_diagnostic(text);
    }
    arch::present(b"CONDUIT_JETSON_NANO_PRODUCT_REFUSAL ");
    arch::present(reason.as_bytes());
    arch::present(b"\n");
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    refuse("panic")
}

core::arch::global_asm!(include_str!("aarch64_jetson_nano_entry.S"));

fn validate_handoff(address: u64) {
    use conduitos::arch::jetson_nano::{
        dtb::Dtb,
        handoff,
        memory::{Region, admit},
    };
    // Firmware must supply a readable DTB in initialized DRAM. Do not form an
    // unbounded slice or assume nominal RAM capacity authorizes a placement.
    if address & 7 != 0 || address < 0x8000_0000 || address > 0x1_8000_0000 - 40 {
        refuse("dtb-address");
    }
    let header = unsafe { core::slice::from_raw_parts(address as *const u8, 40) };
    let total = u32::from_be_bytes(header[4..8].try_into().unwrap()) as usize;
    if !(40..=2 * 1024 * 1024).contains(&total)
        || address
            .checked_add(total as u64)
            .is_none_or(|end| end > 0x1_8000_0000)
    {
        refuse("dtb-extent");
    }
    let bytes = unsafe { core::slice::from_raw_parts(address as *const u8, total) };
    let tree = Dtb::parse(bytes, address).unwrap_or_else(|_| refuse("dtb-invalid"));
    let mut memory = handoff::memory(&tree).unwrap_or_else(|_| refuse("dtb-memory-unsupported"));
    let firmware = conduitos::arch::jetson_nano::firmware_reservations::from_dtb(&tree)
        .unwrap_or_else(|_| refuse("firmware-reservations-invalid"));
    firmware
        .append_to(&mut memory.reserved)
        .unwrap_or_else(|_| refuse("firmware-reservations-bound"));
    unsafe extern "C" {
        static __conduitos_image_start: u8;
        static __conduitos_image_end: u8;
    }
    let start = core::ptr::addr_of!(__conduitos_image_start) as u64;
    let end = core::ptr::addr_of!(__conduitos_image_end) as u64;
    let image = Region::new(start, end - start).unwrap_or_else(|_| refuse("image-extent"));
    admit(&memory.ram, &memory.reserved, image, 4096)
        .unwrap_or_else(|_| refuse("image-memory-conflict"));
    if let Some(framebuffer) = firmware.framebuffer {
        let mut protected = conduitos::arch::jetson_nano::memory::Regions::new();
        let framebuffer_end = framebuffer.start() + framebuffer.bytes();
        if !memory.ram.iter().any(|bank| {
            framebuffer.start() >= bank.start() && framebuffer_end <= bank.start() + bank.bytes()
        }) {
            refuse("framebuffer-outside-ram");
        }
        // Only the exact framebuffer allocation may transfer from retained
        // firmware to diagnostic custody; larger/other live ranges stay fenced.
        for occupied in memory.occupied.iter() {
            if occupied != framebuffer {
                protected
                    .push(occupied, false)
                    .unwrap_or_else(|_| refuse("display-firmware-bound"));
            }
        }
        if let Some(lp0) = firmware.lp0_vector {
            protected
                .push(lp0, false)
                .unwrap_or_else(|_| refuse("display-lp0-bound"));
        }

        protected
            .push(image, false)
            .unwrap_or_else(|_| refuse("display-image-bound"));
        protected
            .push(Region::new(address, total as u64).unwrap(), false)
            .unwrap_or_else(|_| refuse("display-dtb-bound"));
        // CPU data cache and MMU are disabled by the reviewed entry contract;
        // one parked-core boot owns these diagnostics and retained controllers.
        if let Ok(display) =
            unsafe { conduitos::arch::jetson_nano::display_mmio::probe(framebuffer, &protected) }
        {
            unsafe {
                BOOT_DISPLAY = Some(display);
            }
        }
    }
}

// Diagnostic-only custody. Initialized once with IRQs masked, then read-only;
// secondary cores park in the entry stub. This is not product Face state.
static mut BOOT_DISPLAY: Option<conduitos::arch::jetson_nano::display::Descriptor> = None;
fn boot_diagnostic(message: &str) {
    if let Some(display) = unsafe { BOOT_DISPLAY } {
        let _ = unsafe { conduitos::arch::jetson_nano::display_mmio::diagnostic(display, message) };
    }
}
