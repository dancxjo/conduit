use conduitos::{
    allocation::BOOT_ARENA, arch, boot, identity, make::EMBEDDED_MAKE, offer::CpuFeatures,
    offer_make::ImageBoundHostOffer,
};
use core::{arch::global_asm, cell::UnsafeCell, panic::PanicInfo};

#[path = "../armv6_product/mod.rs"]
mod product;
const ARENA_BYTES: usize = EMBEDDED_MAKE.runtime_arena_ceiling as usize;
#[repr(C, align(64))]
struct RuntimeArena(UnsafeCell<[u8; ARENA_BYTES]>);
unsafe impl Sync for RuntimeArena {}
#[used]
#[unsafe(link_section = ".bss.runtime_arena")]
static RUNTIME_ARENA: RuntimeArena = RuntimeArena(UnsafeCell::new([0; ARENA_BYTES]));

#[cfg(target_arch = "arm")]
global_asm!(
    r#"
    .syntax unified
    .cpu arm1176jzf-s
    .arm
    .section .text.entry, "ax"
    .global conduitos_armv6_rpi_b_plus_product_entry
conduitos_armv6_rpi_b_plus_product_entry:
    cpsid if
    cps #0x12
    ldr sp, =__conduitos_irq_stack_end
    cps #0x13
    ldr sp, =__conduitos_boot_stack_end
    ldr r0, =__bss_start
    ldr r1, =__bss_end
    mov r2, #0
0:
    cmp r0, r1
    strlo r2, [r0], #4
    blo 0b
    bl conduitos_raspberry_pi_product_start
1:
    wfe
    b 1b

    .section .bss.irq_stack, "aw", %nobits
    .balign 16
    .space 4096
__conduitos_irq_stack_end:
    .section .bss.boot_stack, "aw", %nobits
    .balign 16
    .space 4194304
__conduitos_boot_stack_end:
    .section .bss.exception_stack, "aw", %nobits
    .balign 16
    .space 4096
    .global __conduitos_exception_stack_end
__conduitos_exception_stack_end:
"#
);

#[unsafe(no_mangle)]
pub extern "C" fn conduitos_raspberry_pi_product_start() -> ! {
    arch::initialize_machine();
    arch::present(if cfg!(feature = "aarch64-rpi-3b-plus") {
        b"CONDUIT_BOOT_STAGE rpi3-product-entered\n"
    } else {
        b"CONDUIT_BOOT_STAGE armv6-product-entered\n"
    });
    let revision = arch::firmware_board_revision()
        .unwrap_or_else(|| refuse("firmware-board-revision-unavailable"));
    {
        use core::fmt::Write;
        let mut text = conduitos::sign_format::FixedText::new();
        let _ = writeln!(
            text,
            "CONDUIT_RASPBERRY_PI_BOARD revision={revision:#x} architecture={}",
            arch::ARCHITECTURE
        );
        arch::present(text.as_bytes());
    }
    let supported = if cfg!(feature = "aarch64-rpi-3b-plus") {
        revision & 0x00ff_ffff == 0xa020d3
    } else {
        matches!(revision, 0x10 | 0x13 | 0x900032)
    };
    if !supported {
        refuse("raspberry-pi-product-board-unsupported");
    }
    let nonce = arch::read_counter();
    let record = boot_record(nonce).unwrap_or_else(|error| refuse(error.as_str()));
    unsafe {
        BOOT_ARENA
            .initialize(RUNTIME_ARENA.0.get() as usize, ARENA_BYTES)
            .unwrap_or_else(|_| refuse("runtime-arena-invalid"));
    }
    EMBEDDED_MAKE
        .validate(ARENA_BYTES as u64)
        .unwrap_or_else(|error| refuse(error.as_str()));
    let ids = identity::derive(
        [
            nonce,
            nonce.rotate_left(7),
            nonce.rotate_left(19),
            nonce.rotate_left(31),
        ],
        record.timestamp,
        record.image_physical_start,
    );
    let offer = ImageBoundHostOffer::new(
        &ids,
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
    let boot_evidence=serde_json::to_vec(&serde_json::json!({
        "schema":"conduit.conduitos/product-boot@1","target":EMBEDDED_MAKE.target,
        "firmware_board_revision":revision,"host_id":identity::hex(&ids.host),"boot_id":identity::hex(&ids.boot),
        "profile_id":EMBEDDED_MAKE.profile_id,"build_id":EMBEDDED_MAKE.build_id,"image_id":EMBEDDED_MAKE.image_binding,
        "physical_scanout_proved":false,"speaker_heard_claimed":false
    })).unwrap_or_else(|_|refuse("boot-evidence-encoding"));
    arch::present(b"CONDUIT_PRODUCT_BOOT ");
    arch::present(&boot_evidence);
    arch::present(b"\n");
    drop(boot_evidence);
    product::run_product(&ids, &offer)
}

fn boot_record(nonce: u64) -> Result<boot::BootRecord, boot::BootError> {
    unsafe extern "C" {
        static __image_start: u8;
        static __image_file_end: u8;
    }
    let image_start = core::ptr::addr_of!(__image_start) as u64;
    let image_end = core::ptr::addr_of!(__image_file_end) as u64;
    let mut normalizer = boot::BootNormalizer::new(
        boot::Firmware::RaspberryPiVideoCore,
        nonce,
        0,
        image_start,
        image_end - image_start,
    )?;
    normalizer.push_region(boot::MemoryRegion {
        base: image_start,
        length: image_end - image_start,
        kind: boot::MemoryKind::ExecutableAndArtifacts,
    })?;
    normalizer.push_region(boot::MemoryRegion {
        base: RUNTIME_ARENA.0.get() as u64,
        length: ARENA_BYTES as u64,
        kind: boot::MemoryKind::Usable,
    })?;
    normalizer.set_command_line(b"")?;
    normalizer.finish()
}

fn refuse(reason: &str) -> ! {
    arch::disable_interrupts();
    arch::present(if cfg!(feature = "aarch64-rpi-3b-plus") {
        b"CONDUIT_RPI3_PRODUCT_REFUSAL "
    } else {
        b"CONDUIT_ARMV6_PRODUCT_REFUSAL "
    });
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
#[alloc_error_handler]
fn allocation_error(layout: core::alloc::Layout) -> ! {
    use core::fmt::Write;
    arch::disable_interrupts();
    let mut text = conduitos::sign_format::FixedText::new();
    let _ = writeln!(
        text,
        "CONDUIT_ALLOCATION_REFUSAL requested={} alignment={} live={} peak={} capacity={}",
        layout.size(),
        layout.align(),
        BOOT_ARENA.live_bytes(),
        BOOT_ARENA.used(),
        BOOT_ARENA.capacity()
    );
    arch::present(text.as_bytes());
    refuse("allocation-capacity-exhausted")
}

#[cfg(target_arch = "aarch64")]
global_asm!(include_str!("../rpi3_entry.S"));
