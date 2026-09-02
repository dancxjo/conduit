#![no_std]
#![no_main]

#[cfg(not(target_arch = "x86"))]
compile_error!("conduitos-ia32-product is only the IA-32 product Host");

use core::panic::PanicInfo;

use conduit_core::{BootId, HostId, OfferGeneration};
use conduitos::{
    allocation::BootArena,
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

unsafe extern "C" {
    static __conduitos_image_start: u8;
    static __conduitos_image_end: u8;
}

#[used]
#[unsafe(link_section = ".multiboot")]
static MULTIBOOT1_HEADER: [u32; 6] = [0x1bad_b002, 4, 0xe452_4ffa, 0, 0, 0];

#[repr(align(4096))]
struct AlignedArena([u8; 1024 * 1024]);

static mut RUNTIME_ARENA: AlignedArena = AlignedArena([0; 1024 * 1024]);

#[global_allocator]
static BOOT_ARENA: BootArena = BootArena::new();

core::arch::global_asm!(
    r#"
.section .bss.conduitos_ia32_product_stack,"aw",@nobits
.balign 16
conduitos_ia32_product_stack:
    .skip 1048576
.section .text.conduitos_ia32_product_start,"ax",@progbits
.global conduitos_ia32_product_start
.type conduitos_ia32_product_start,@function
conduitos_ia32_product_start:
    lea esp, [conduitos_ia32_product_stack + 1048576]
    sub esp, 4
    xor ebp, ebp
    mov eax, cr0
    and eax, 0xfffffffb
    or eax, 0x2
    mov cr0, eax
    mov eax, cr4
    or eax, 0x600
    mov cr4, eax
    jmp conduitos_ia32_product_rust_entry
"#
);

#[unsafe(no_mangle)]
extern "C" fn conduitos_ia32_product_rust_entry() -> ! {
    unsafe {
        BOOT_ARENA
            .initialize(
                core::ptr::addr_of_mut!(RUNTIME_ARENA.0) as *mut u8 as usize,
                1024 * 1024,
            )
            .unwrap_or_else(|_| refuse("runtime-arena-initialization-failed"));
    }
    arch::initialize_machine();
    EMBEDDED_FABRICATION
        .validate(1024 * 1024)
        .unwrap_or_else(|error| refuse(error.as_str()));
    if EMBEDDED_FABRICATION.target != "conduitos/ia32/pc"
        || !EMBEDDED_FABRICATION.includes(IMPL_LINEAR_PRESENTER)
    {
        refuse("ia32-product-fabrication-mismatch");
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
        0x0010_0000,
    );
    let offer = ImageBoundHostOffer::new(
        &identities,
        &EMBEDDED_FABRICATION,
        CpuFeatures {
            sse2: true,
            rdrand: false,
            invariant_tsc: false,
        },
        1024 * 1024,
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
    let mut presenter = LinearPresenter::prepare_with_realization(
        host_id,
        boot_id,
        generation,
        EMBEDDED_FABRICATION.profile_id,
        EMBEDDED_FABRICATION.image_binding,
        "presenter/ia32-linear-debugcon@1",
        "conduitos/presenter/ia32-linear-debugcon@1",
        "conduitos/base/ia32-debugcon/0",
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
    let image_start = core::ptr::addr_of!(__conduitos_image_start) as usize;
    let image_end = core::ptr::addr_of!(__conduitos_image_end) as usize;
    let boot_record = BootRecord {
        firmware: Firmware::Uefi32,
        timestamp: counter,
        hhdm_offset: 0,
        image_physical_start: image_start as u64,
        image_length: image_end.saturating_sub(image_start) as u64,
        memory_region_count: 1,
        artifact_count: 0,
        framebuffer_count: 0,
        command_line_bytes: 0,
        runtime_arena: RuntimeArena {
            physical_start: unsafe {
                core::ptr::addr_of_mut!(RUNTIME_ARENA.0) as *mut u8 as usize as u64
            },
            length: 1024 * 1024,
        },
    };
    let observatory_export = observatory::prepare_export(
        &boot_record,
        &identities,
        &offer,
        &prepared,
        EMBEDDED_FABRICATION.build_id,
        EMBEDDED_FABRICATION.image_binding,
        None,
    )
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

    arch::present(b"CONDUIT_IA32_PRODUCT {\"schema\":\"conduit.conduitos/ia32-product@1\",\"status\":\"ready\",\"profile_id\":\"");
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
    arch::present(b"\",\"semantic_result\":\"HELLO, CONDUITOS\",\"interactive_local_control\":false,\"long_lived\":true}\n");
    arch::present(observatory::EXPORT_PREFIX.as_bytes());
    arch::present(observatory_export.as_bytes());
    arch::present(b"\n");
    loop {
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)) }
    }
}

fn refuse(reason: &str) -> ! {
    arch::disable_interrupts();
    arch::present(b"CONDUIT_IA32_PRODUCT_REFUSAL ");
    arch::present(reason.as_bytes());
    arch::present(b"\n");
    loop {
        core::hint::spin_loop();
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memcmp(left: *const u8, right: *const u8, length: usize) -> i32 {
    for index in 0..length {
        let left = unsafe { *left.add(index) };
        let right = unsafe { *right.add(index) };
        if left != right {
            return i32::from(left) - i32::from(right);
        }
    }
    0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn bcmp(left: *const u8, right: *const u8, length: usize) -> i32 {
    unsafe { memcmp(left, right, length) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memmove(destination: *mut u8, source: *const u8, length: usize) -> *mut u8 {
    if (destination as usize) <= (source as usize) {
        for index in 0..length {
            unsafe { destination.add(index).write(source.add(index).read()) };
        }
    } else {
        for index in (0..length).rev() {
            unsafe { destination.add(index).write(source.add(index).read()) };
        }
    }
    destination
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memcpy(destination: *mut u8, source: *const u8, length: usize) -> *mut u8 {
    for index in 0..length {
        unsafe { destination.add(index).write(source.add(index).read()) };
    }
    destination
}

#[unsafe(no_mangle)]
unsafe extern "C" fn memset(destination: *mut u8, value: i32, length: usize) -> *mut u8 {
    for index in 0..length {
        unsafe { destination.add(index).write(value as u8) };
    }
    destination
}

#[unsafe(no_mangle)]
extern "C" fn rust_eh_personality() {}

#[unsafe(no_mangle)]
extern "C" fn _Unwind_Resume() -> ! {
    refuse("unexpected-unwind")
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    refuse("panic")
}
