#![no_std]
#![no_main]

#[cfg(not(target_arch = "riscv64"))]
compile_error!("conduitos-riscv64-product is only the RISC-V64 product Host");

use conduit_core::{BootId, HostId, OfferGeneration};
use conduitos::{
    allocation::BOOT_ARENA,
    arch,
    boot::{BootRecord, Firmware, RuntimeArena},
    dual_region_composition, dual_region_plan,
    fabrication::{EMBEDDED_FABRICATION, IMPL_LINEAR_PRESENTER},
    front_door::FrontDoor,
    identity, keyboard_text_plan,
    linear_presenter::LinearPresenter,
    observatory,
    offer::CpuFeatures,
    offer_fabrication::ImageBoundHostOffer,
};
use core::panic::PanicInfo;

unsafe extern "C" {
    static __conduitos_image_start: u8;
    static __conduitos_image_end: u8;
}
static mut MEMORY_ARENA: [u8; 1024 * 1024] = [0; 1024 * 1024];

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.conduitos_riscv64_product_start")]
pub extern "C" fn conduitos_riscv64_product_start() -> ! {
    unsafe {
        BOOT_ARENA.initialize(
            core::ptr::addr_of_mut!(MEMORY_ARENA) as *mut u8 as usize,
            1024 * 1024,
        )
    }
    .unwrap_or_else(|_| refuse("runtime-arena-initialization-failed"));
    if !arch::initialize_machine() {
        refuse("unavailable-or-stale-trap-controller");
    }
    EMBEDDED_FABRICATION
        .validate(1024 * 1024)
        .unwrap_or_else(|error| refuse(error.as_str()));
    if EMBEDDED_FABRICATION.target != "conduitos/riscv64/virt"
        || !EMBEDDED_FABRICATION.includes(IMPL_LINEAR_PRESENTER)
    {
        refuse("riscv64-product-fabrication-mismatch");
    }
    let counter = arch::read_counter();
    let identities = identity::derive(
        [
            counter,
            counter.rotate_left(13),
            counter.rotate_left(29),
            counter.rotate_left(47),
        ],
        counter,
        0x8020_0000,
    );
    let offer = ImageBoundHostOffer::new(
        &identities,
        &EMBEDDED_FABRICATION,
        CpuFeatures {
            sse2: false,
            rdrand: false,
            invariant_tsc: false,
        },
        1024 * 1024,
    )
    .unwrap_or_else(|error| refuse(error.as_str()));
    offer
        .validate()
        .unwrap_or_else(|error| refuse(error.as_str()));
    let host_identity = identity::hex(&identities.host);
    let boot_identity = identity::hex(&identities.boot);
    let host_id = HostId::from(host_identity.clone());
    let boot_id = BootId::from(boot_identity.clone());
    let generation = OfferGeneration(offer.generation);
    let form =
        keyboard_text_plan::checked_form_identity().unwrap_or_else(|error| refuse(error.as_str()));
    let front_door = FrontDoor::new(
        host_id.clone(),
        boot_id.clone(),
        generation,
        EMBEDDED_FABRICATION.profile_id,
        EMBEDDED_FABRICATION.build_id,
        EMBEDDED_FABRICATION.image_binding,
        form.source_document_id,
        form.checked_form_id,
        5,
        false,
    );
    let presentation = front_door
        .presentation()
        .unwrap_or_else(|error| refuse(error.as_str()));
    let mut presenter = LinearPresenter::prepare_with_realization(
        host_id,
        boot_id,
        generation,
        EMBEDDED_FABRICATION.profile_id,
        EMBEDDED_FABRICATION.image_binding,
        "presenter/riscv64-linear-sbi-console@1",
        "conduitos/presenter/riscv64-linear-sbi-console@1",
        "conduitos/base/riscv64-sbi-console/0",
    )
    .unwrap_or_else(|_| refuse("linear-presenter-plan-refused"));
    let receipt = presenter
        .present(&presentation)
        .unwrap_or_else(|_| refuse("linear-presenter-manifestation-refused"));
    let mut prepared =
        dual_region_plan::prepare(&identities, &offer, EMBEDDED_FABRICATION.build_id)
            .unwrap_or_else(|error| refuse(error.as_str()));
    let image_start = core::ptr::addr_of!(__conduitos_image_start) as usize;
    let image_end = core::ptr::addr_of!(__conduitos_image_end) as usize;
    let boot_record = BootRecord {
        firmware: Firmware::Sbi,
        timestamp: counter,
        hhdm_offset: 0,
        image_physical_start: image_start as u64,
        image_length: image_end.saturating_sub(image_start) as u64,
        memory_region_count: 1,
        artifact_count: 0,
        framebuffer_count: 0,
        command_line_bytes: 0,
        runtime_arena: RuntimeArena {
            physical_start: core::ptr::addr_of!(MEMORY_ARENA) as u64,
            length: 1024 * 1024,
        },
    };
    let export = observatory::prepare_export(
        &boot_record,
        &identities,
        &offer,
        &prepared,
        EMBEDDED_FABRICATION.build_id,
        EMBEDDED_FABRICATION.image_binding,
        None,
    )
    .unwrap_or_else(|error| refuse(error.as_str()));
    let before = BOOT_ARENA.seal();
    let (mut clock, mut timer, mut serial, mut interrupts, mut idle) = (
        arch::Clock::new(),
        arch::Timer::new(),
        arch::Serial::new(),
        arch::Interrupts::new(),
        arch::Idle::new(),
    );
    let report = dual_region_composition::run(
        &mut prepared.kernel,
        &mut clock,
        &mut timer,
        &mut serial,
        &mut interrupts,
        &mut idle,
    )
    .unwrap_or_else(|error| refuse(error.as_str()));
    if BOOT_ARENA.used() != before {
        refuse("allocation-during-play");
    }
    arch::present(b"CONDUIT_RISCV64_PRODUCT {\"schema\":\"conduit.conduitos/riscv64-product@1\",\"status\":\"ready\",\"profile_id\":\"");
    arch::present(EMBEDDED_FABRICATION.profile_id.as_bytes());
    arch::present(b"\",\"build_id\":\"");
    arch::present(EMBEDDED_FABRICATION.build_id.as_bytes());
    arch::present(b"\",\"image_id\":\"");
    arch::present(EMBEDDED_FABRICATION.image_binding.as_bytes());
    arch::present(b"\",\"host_id\":\"");
    arch::present(host_identity.as_bytes());
    arch::present(b"\",\"boot_id\":\"");
    arch::present(boot_identity.as_bytes());
    arch::present(b"\",\"offer_generation\":1,\"presentation_id\":\"");
    arch::present(receipt.presentation.presentation_id.as_str().as_bytes());
    arch::present(b"\",\"manifestation_id\":\"");
    arch::present(receipt.manifestation_id.as_str().as_bytes());
    arch::present(b"\",\"presenter_implementation_id\":\"");
    arch::present(receipt.presenter_implementation_id.as_str().as_bytes());
    arch::present(b"\",\"ordinary_plan_id\":\"");
    arch::present(prepared.plan.plan_id.as_str().as_bytes());
    arch::present(b"\",\"ordinary_play_id\":\"");
    arch::present(prepared.active_play.active_play_id.as_str().as_bytes());
    arch::present(b"\",\"semantic_result\":\"");
    if report.timer_irq_wakes == 0 {
        refuse("timer-wake-absent");
    }
    arch::present(b"HELLO, CONDUITOS");
    arch::present(b"\",\"timer_irq_wakes\":1,\"long_lived\":true}\n");
    arch::present(observatory::EXPORT_PREFIX.as_bytes());
    arch::present(export.as_bytes());
    arch::present(b"\n");
    for line in &receipt.presentation.lines {
        arch::present(b"CONDUIT_LINEAR_PRESENTATION ");
        arch::present(line.as_bytes());
        arch::present(b"\n");
    }
    loop {
        core::hint::spin_loop();
    }
}

fn refuse(reason: &str) -> ! {
    arch::disable_interrupts();
    arch::present(b"CONDUIT_RISCV64_PRODUCT_REFUSAL ");
    arch::present(reason.as_bytes());
    arch::present(b"\n");
    loop {
        core::hint::spin_loop();
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn memcpy(d: *mut u8, s: *const u8, n: usize) -> *mut u8 {
    for i in 0..n {
        unsafe { d.add(i).write(s.add(i).read()) };
    }
    d
}
#[unsafe(no_mangle)]
unsafe extern "C" fn memset(d: *mut u8, v: i32, n: usize) -> *mut u8 {
    for i in 0..n {
        unsafe { d.add(i).write(v as u8) };
    }
    d
}
#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    refuse("panic")
}
