#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

use conduitos::{
    allocation::BOOT_ARENA, arch, boot, identity, make::EMBEDDED_MAKE, offer::CpuFeatures,
    offer_make::ImageBoundHostOffer,
};
use core::{arch::global_asm, cell::UnsafeCell, panic::PanicInfo};

#[path = "armv6_product/mod.rs"]
mod product;
const ARENA_BYTES: usize = EMBEDDED_MAKE.runtime_arena_ceiling as usize;
#[repr(C, align(64))]
struct RuntimeArena(UnsafeCell<[u8; ARENA_BYTES]>);
unsafe impl Sync for RuntimeArena {}
#[used]
#[unsafe(link_section = ".bss.runtime_arena")]
static RUNTIME_ARENA: RuntimeArena = RuntimeArena(UnsafeCell::new([0; ARENA_BYTES]));

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
    bl conduitos_armv6_product_start
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
pub extern "C" fn conduitos_armv6_product_start() -> ! {
    arch::initialize_machine();
    arch::present(b"CONDUIT_BOOT_STAGE armv6-product-entered\n");
    let revision = arch::firmware_board_revision().unwrap_or(0);
    if !matches!(revision, 0x10 | 0x13 | 0x900032) {
        refuse("armv6-product-board-unsupported");
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
    #[cfg(feature = "native-startup-chime")]
    let mut audio = arch::PwmAudio::prepare().ok();
    #[cfg(feature = "native-startup-chime")]
    let mut startup = {
        use conduitos::pcm_output::PcmOutput;
        arch::present(b"CONDUIT_BOOT_STAGE startup-plot-prepare\n");
        conduitos::startup_plot::StartupPlot::prepare(
            &ids,
            &offer,
            EMBEDDED_MAKE.build_id,
            audio.as_ref().map(PcmOutput::provider),
        )
        .unwrap_or_else(|reason| refuse(reason))
    };
    #[cfg(feature = "native-startup-chime")]
    {
        use conduitos::pcm_output::PcmOutput;
        let before = BOOT_ARENA.attempts();
        let receipt = startup
            .run(audio.as_mut().map(|device| device as &mut dyn PcmOutput))
            .unwrap_or_else(|reason| refuse(reason));
        let allocation_stable = BOOT_ARENA.attempts() == before;
        let evidence = serde_json::to_vec(&serde_json::json!({
            "source":"plots/startup-chime/main.conduit",
            "body_id":receipt.body_id.as_str(),"plan_id":receipt.plan_id.as_str(),
            "active_play_id":receipt.active_play_id.as_str(),"outcome":receipt.outcome,
            "frames":receipt.frames,"allocation_stable_during_play":allocation_stable,
            "speaker_heard_claimed":false
        }))
        .unwrap_or_else(|_| refuse("startup-evidence-encoding"));
        if evidence.len() > 2048 {
            refuse("startup-evidence-bound");
        }
        arch::present(b"CONDUIT_STARTUP_PLOT ");
        arch::present(&evidence);
        arch::present(b"\n");
    }
    #[cfg(feature = "native-startup-chime")]
    if let Some(observation) = audio.as_ref().and_then(arch::PwmAudio::last_playback) {
        let evidence = serde_json::to_vec(&serde_json::json!({
            "frames_queued":observation.frames_queued,"elapsed_us":observation.elapsed_us,
            "pwm_status":observation.pwm_status,"completed":observation.completed,
            "failure":observation.failure.map(|error|error.as_str())
        }))
        .unwrap_or_else(|_| refuse("pcm-evidence-encoding"));
        arch::present(b"CONDUIT_PCM_PLAYBACK ");
        arch::present(&evidence);
        arch::present(b"\n");
    }
    #[cfg(feature = "native-startup-chime")]
    drop(startup);
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
    arch::present(b"CONDUIT_ARMV6_PRODUCT_REFUSAL ");
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
fn allocation_error(_layout: core::alloc::Layout) -> ! {
    refuse("allocation-capacity-exhausted")
}
