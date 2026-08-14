#![no_std]
#![no_main]

#[cfg(not(target_arch = "aarch64"))]
compile_error!("conduitos-aarch64-product is only the AArch64 product Host");

use core::panic::PanicInfo;

use conduit_core::{BootId, HostId, OfferGeneration};
use conduitos::{
    allocation::BOOT_ARENA,
    arch, boot, dual_region_composition, dual_region_plan,
    fabrication::{EMBEDDED_FABRICATION, IMPL_LINEAR_PRESENTER},
    front_door::FrontDoor,
    identity, keyboard_text_plan,
    linear_presenter::LinearPresenter,
    offer::CpuFeatures,
    offer_fabrication::ImageBoundHostOffer,
};

#[unsafe(no_mangle)]
pub extern "C" fn conduitos_aarch64_product_start() -> ! {
    arch::enable_fp_simd();
    let (l0, l1, l2) = arch::mmio_table_addresses();
    let l0 = boot::executable_physical_address(l0).unwrap_or_else(|| refuse("mmio-l0-invalid"));
    let l1 = boot::executable_physical_address(l1).unwrap_or_else(|| refuse("mmio-l1-invalid"));
    let l2 = boot::executable_physical_address(l2).unwrap_or_else(|| refuse("mmio-l2-invalid"));
    arch::install_low_mmio_map(l0, l1, l2);
    arch::initialize_machine();
    let record = boot::normalize_boot().unwrap_or_else(|error| refuse(error.as_str()));
    let arena = record
        .hhdm_offset
        .checked_add(record.runtime_arena.physical_start)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_else(|| refuse("runtime-arena-address-invalid"));
    unsafe {
        BOOT_ARENA
            .initialize(
                arena,
                usize::try_from(record.runtime_arena.length).unwrap_or(0),
            )
            .unwrap_or_else(|_| refuse("runtime-arena-initialization-failed"));
    }
    EMBEDDED_FABRICATION
        .validate(record.runtime_arena.length)
        .unwrap_or_else(|error| refuse(error.as_str()));
    if !EMBEDDED_FABRICATION.includes(IMPL_LINEAR_PRESENTER) {
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
        record.timestamp,
        record.image_physical_start,
    );
    let offer = ImageBoundHostOffer::new(
        &identities,
        &EMBEDDED_FABRICATION,
        CpuFeatures {
            sse2: false,
            rdrand: false,
            invariant_tsc: false,
        },
        record.runtime_arena.length,
    )
    .unwrap_or_else(|error| refuse(error.as_str()));
    offer
        .validate()
        .unwrap_or_else(|error| refuse(error.as_str()));

    let host_id = HostId::from(identity::hex(&identities.host));
    let boot_id = BootId::from(identity::hex(&identities.boot));
    let generation = OfferGeneration(offer.generation);
    let seed =
        keyboard_text_plan::checked_seed_identity().unwrap_or_else(|error| refuse(error.as_str()));
    let front_door = FrontDoor::new(
        host_id.clone(),
        boot_id.clone(),
        generation,
        EMBEDDED_FABRICATION.profile_id,
        EMBEDDED_FABRICATION.build_id,
        EMBEDDED_FABRICATION.image_binding,
        seed.source_document_id,
        seed.checked_form_id,
        5,
        false,
    );
    let presentation = front_door
        .presentation()
        .unwrap_or_else(|error| refuse(error.as_str()));
    let mut presenter = LinearPresenter::prepare(
        host_id,
        boot_id,
        generation,
        EMBEDDED_FABRICATION.profile_id,
        EMBEDDED_FABRICATION.image_binding,
    )
    .unwrap_or_else(|_| refuse("linear-presenter-plan-refused"));
    let receipt = presenter
        .present(&presentation)
        .unwrap_or_else(|_| refuse("linear-presenter-manifestation-refused"));
    for line in &receipt.presentation.lines {
        arch::present(b"CONDUIT_LINEAR_PRESENTATION ");
        arch::present(line.as_bytes());
        arch::present(b"\n");
    }

    let mut prepared =
        dual_region_plan::prepare(&identities, &offer, EMBEDDED_FABRICATION.build_id)
            .unwrap_or_else(|error| refuse(error.as_str()));
    let mut clock = arch::Clock::new();
    let mut timer = arch::Timer::new();
    let mut serial = arch::Serial::new();
    let mut interrupts = arch::Interrupts::new();
    let mut idle = arch::Idle::new();
    let report = dual_region_composition::run(
        &mut prepared.kernel,
        &mut clock,
        &mut timer,
        &mut serial,
        &mut interrupts,
        &mut idle,
    )
    .unwrap_or_else(|error| refuse(error.as_str()));

    arch::present(b"CONDUIT_AARCH64_PRODUCT {\"schema\":\"conduit.conduitos/aarch64-product@1\",\"status\":\"ready\",\"profile_id\":\"");
    arch::present(EMBEDDED_FABRICATION.profile_id.as_bytes());
    arch::present(b"\",\"build_id\":\"");
    arch::present(EMBEDDED_FABRICATION.build_id.as_bytes());
    arch::present(b"\",\"image_id\":\"");
    arch::present(EMBEDDED_FABRICATION.image_binding.as_bytes());
    arch::present(b"\",\"host_id\":\"");
    arch::present(identity::hex(&identities.host).as_bytes());
    arch::present(b"\",\"boot_id\":\"");
    arch::present(identity::hex(&identities.boot).as_bytes());
    arch::present(b"\",\"offer_generation\":1,\"body_id\":null,\"presentation_id\":\"");
    arch::present(receipt.presentation.presentation_id.as_str().as_bytes());
    arch::present(b"\",\"manifestation_id\":\"");
    arch::present(receipt.manifestation_id.as_str().as_bytes());
    arch::present(b"\",\"presenter_implementation_id\":\"");
    arch::present(receipt.presenter_implementation_id.as_str().as_bytes());
    arch::present(b"\",\"presenter_plan_id\":\"");
    arch::present(receipt.plan_id.as_str().as_bytes());
    arch::present(b"\",\"ordinary_plan_id\":\"");
    arch::present(prepared.plan.plan_id.as_str().as_bytes());
    arch::present(b"\",\"ordinary_play_id\":\"");
    arch::present(prepared.active_play.active_play_id.as_str().as_bytes());
    arch::present(b"\",\"semantic_result\":\"");
    let _ = report;
    arch::present(b"HELLO, CONDUITOS");
    arch::present(b"\",\"interactive_local_control\":false,\"long_lived\":true}\n");
    arch::present(b"CONDUIT_BOOT_STAGE aarch64-product-ready\n");
    loop {
        unsafe { core::arch::asm!("wfi", options(nomem, nostack)) };
    }
}

fn refuse(reason: &str) -> ! {
    arch::disable_interrupts();
    arch::present(b"CONDUIT_AARCH64_PRODUCT_REFUSAL ");
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
