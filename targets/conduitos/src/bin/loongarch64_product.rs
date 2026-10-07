#![no_std]
#![no_main]

#[cfg(not(target_arch = "loongarch64"))]
compile_error!("conduitos-loongarch64-product is only the LoongArch64 product Host");

use conduit_core::{BootId, HostId, OfferGeneration};
use conduitos::{
    allocation::BOOT_ARENA,
    arch, boot, dual_region_composition, dual_region_plan,
    front_door::FrontDoor,
    identity, keyboard_text_plan,
    linear_presenter::LinearPresenter,
    make::{EMBEDDED_MAKE, IMPL_LINEAR_PRESENTER},
    observatory,
    offer::CpuFeatures,
    offer_make::ImageBoundHostOffer,
    spore_join,
};
use core::panic::PanicInfo;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.conduitos_loongarch64_product_start")]
pub extern "C" fn conduitos_loongarch64_product_start() -> ! {
    if !arch::initialize_machine() {
        refuse("unavailable-or-stale-trap-controller");
    }
    let boot_record = boot::normalize_boot().unwrap_or_else(|error| refuse(error.as_str()));
    let arena = boot_record
        .hhdm_offset
        .checked_add(boot_record.runtime_arena.physical_start)
        .and_then(|address| usize::try_from(address).ok())
        .unwrap_or_else(|| refuse("runtime-arena-address-invalid"));
    unsafe { BOOT_ARENA.initialize(arena, boot_record.runtime_arena.length as usize) }
        .unwrap_or_else(|_| refuse("runtime-arena-initialization-failed"));
    EMBEDDED_MAKE
        .validate(boot_record.runtime_arena.length)
        .unwrap_or_else(|error| refuse(error.as_str()));
    if EMBEDDED_MAKE.target != "conduitos/loongarch64/virt"
        || !EMBEDDED_MAKE.includes(IMPL_LINEAR_PRESENTER)
    {
        refuse("loongarch64-product-make-mismatch");
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
        boot_record.image_physical_start,
    );
    let offer = ImageBoundHostOffer::new(
        &identities,
        &EMBEDDED_MAKE,
        CpuFeatures {
            sse2: false,
            rdrand: false,
            invariant_tsc: false,
        },
        boot_record.runtime_arena.length,
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
    let presentation = front_door
        .presentation()
        .unwrap_or_else(|error| refuse(error.as_str()));
    let mut presenter = LinearPresenter::prepare_with_realization(
        host_id,
        boot_id,
        generation,
        EMBEDDED_MAKE.profile_id,
        EMBEDDED_MAKE.image_binding,
        "presenter/loongarch64-linear-uart@1",
        "conduitos/presenter/loongarch64-linear-uart@1",
        "conduitos/base/loongarch64-uart/0",
    )
    .unwrap_or_else(|_| refuse("linear-presenter-plan-refused"));
    let receipt = presenter
        .present(&presentation)
        .unwrap_or_else(|_| refuse("linear-presenter-manifestation-refused"));
    let mut prepared = dual_region_plan::prepare(&identities, &offer, EMBEDDED_MAKE.build_id)
        .unwrap_or_else(|error| refuse(error.as_str()));
    let region = boot::spore_module().unwrap_or_else(|| refuse("spore-boot-module-missing"));
    if let Some(join) = spore_join::encode_region(
        region,
        EMBEDDED_MAKE.target,
        EMBEDDED_MAKE.profile_id,
        EMBEDDED_MAKE.build_id,
        &prepared.advertisement,
    )
    .unwrap_or_else(|error| refuse(error))
    {
        arch::present(b"CONDUIT_SPORE_JOIN ");
        arch::present(&join);
        arch::present(b"\n");
    }
    let export = observatory::prepare_export(
        &boot_record,
        &identities,
        &offer,
        &prepared,
        EMBEDDED_MAKE.build_id,
        EMBEDDED_MAKE.image_binding,
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
    arch::present(b"CONDUIT_LOONGARCH64_PRODUCT {\"schema\":\"conduit.conduitos/loongarch64-product@1\",\"status\":\"ready\",\"profile_id\":\"");
    arch::present(EMBEDDED_MAKE.profile_id.as_bytes());
    arch::present(b"\",\"build_id\":\"");
    arch::present(EMBEDDED_MAKE.build_id.as_bytes());
    arch::present(b"\",\"image_id\":\"");
    arch::present(EMBEDDED_MAKE.image_binding.as_bytes());
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
    arch::present(b"\",\"ordinary_source_document_id\":\"");
    arch::present(prepared.source_document_id.as_str().as_bytes());
    arch::present(b"\",\"ordinary_checked_plot_id\":\"");
    arch::present(prepared.checked_plot_id.as_str().as_bytes());
    arch::present(b"\",\"ordinary_expanded_plot_id\":\"");
    arch::present(prepared.expanded_plot_id.as_str().as_bytes());
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
    arch::present(b"CONDUIT_LOONGARCH64_PRODUCT_REFUSAL ");
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
