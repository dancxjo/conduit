#![no_std]
#![no_main]

#[cfg(not(all(target_arch = "aarch64", feature = "aarch64-orange-pi-5")))]
compile_error!("the Orange Pi 5 ConduitOS Host requires AArch64 and its exact board feature");

use core::panic::PanicInfo;

use conduit_core::{BootId, HostId, OfferGeneration};
use conduitos::{
    allocation::BOOT_ARENA,
    arch, dual_region_composition, dual_region_plan,
    fabrication::{EMBEDDED_FABRICATION, IMPL_LINEAR_PRESENTER},
    front_door::FrontDoor,
    identity, keyboard_text_plan,
    linear_presenter::LinearPresenter,
    offer::CpuFeatures,
    offer_fabrication::ImageBoundHostOffer,
};

const ARENA_BYTES: usize = 8 * 1024 * 1024;

#[repr(C, align(4096))]
struct RuntimeArena([u8; ARENA_BYTES]);

static mut RUNTIME_ARENA: RuntimeArena = RuntimeArena([0; ARENA_BYTES]);

#[unsafe(no_mangle)]
pub extern "C" fn conduitos_aarch64_orange_pi_5_start() -> ! {
    arch::enable_fp_simd();
    arch::initialize_machine();
    let arena = core::ptr::addr_of_mut!(RUNTIME_ARENA).cast::<u8>() as usize;
    unsafe {
        BOOT_ARENA
            .initialize(arena, ARENA_BYTES)
            .unwrap_or_else(|_| refuse("runtime-arena-initialization-failed"));
    }
    EMBEDDED_FABRICATION
        .validate(ARENA_BYTES as u64)
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
        0,
        0x0020_0000,
    );
    let offer = ImageBoundHostOffer::new(
        &identities,
        &EMBEDDED_FABRICATION,
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
    dual_region_composition::run(
        &mut prepared.kernel,
        &mut clock,
        &mut timer,
        &mut serial,
        &mut interrupts,
        &mut idle,
    )
    .unwrap_or_else(|error| refuse(error.as_str()));

    arch::present(b"CONDUIT_ORANGE_PI_5_PRODUCT {\"schema\":\"conduit.conduitos/orange-pi-5-product@1\",\"status\":\"ready\",\"target\":\"conduitos/aarch64/orange-pi-5-rk3588s\",\"architecture\":\"aarch64\",\"machine\":\"rk3588s\",\"ordinary_plan_id\":\"");
    arch::present(prepared.plan.plan_id.as_str().as_bytes());
    arch::present(b"\",\"ordinary_play_id\":\"");
    arch::present(prepared.active_play.active_play_id.as_str().as_bytes());
    arch::present(
        b"\",\"semantic_result\":\"HELLO, CONDUITOS\",\"physical_proof_claimed\":false}\n",
    );
    arch::present(b"CONDUIT_BOOT_STAGE orange-pi-5-product-ready\n");
    loop {
        core::hint::spin_loop();
    }
}

fn refuse(reason: &str) -> ! {
    arch::disable_interrupts();
    arch::present(b"CONDUIT_ORANGE_PI_5_PRODUCT_REFUSAL ");
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

core::arch::global_asm!(
    r#"
    .section .text.orange_pi_5_header,"ax"
    .global conduitos_orange_pi_5_image_header
conduitos_orange_pi_5_image_header:
    b conduitos_orange_pi_5_boot
    nop
    .quad 0x00200000
    .quad 0
    .quad 0
    .quad 0
    .quad 0
    .quad 0
    .byte 0x41, 0x52, 0x4d, 0x64
    .word 0
conduitos_orange_pi_5_boot:
    msr daifset, #0xf
    adrp x1, __conduitos_boot_stack_top
    add x1, x1, :lo12:__conduitos_boot_stack_top
    mov sp, x1
    adrp x1, __conduitos_bss_start
    add x1, x1, :lo12:__conduitos_bss_start
    adrp x2, __conduitos_bss_end
    add x2, x2, :lo12:__conduitos_bss_end
1:
    cmp x1, x2
    b.hs 2f
    stp xzr, xzr, [x1], #16
    b 1b
2:
    bl conduitos_aarch64_orange_pi_5_start
3:
    wfe
    b 3b
"#
);
