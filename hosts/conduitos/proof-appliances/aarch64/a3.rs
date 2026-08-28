#![no_std]
#![no_main]

#[cfg(not(target_arch = "aarch64"))]
compile_error!("conduitos-aarch64-a3 is only an AArch64 ordinary-Form proof");

use core::panic::PanicInfo;

use conduitos::{allocation::BOOT_ARENA, arch, boot, dual_region_plan, identity, sign_format};

const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
const IMAGE_ID: &str = env!("CONDUITOS_IMAGE_ID");

#[unsafe(no_mangle)]
pub extern "C" fn conduitos_aarch64_a3_start() -> ! {
    arch::enable_fp_simd();
    let (l0, l1, l2) = arch::mmio_table_addresses();
    let l0 = boot::executable_physical_address(l0).unwrap_or_else(|| exit(false));
    let l1 = boot::executable_physical_address(l1).unwrap_or_else(|| exit(false));
    let l2 = boot::executable_physical_address(l2).unwrap_or_else(|| exit(false));
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
    let offer = conduitos::offer::HostOffer::new(
        &identities,
        BUILD_ID,
        conduitos::offer::CpuFeatures {
            sse2: false,
            rdrand: false,
            invariant_tsc: false,
        },
        record.runtime_arena.length,
    );
    offer
        .validate()
        .unwrap_or_else(|error| refuse(error.as_str()));
    let mut prepared = dual_region_plan::prepare(&identities, &offer, BUILD_ID)
        .unwrap_or_else(|error| refuse(error.as_str()));
    let observatory_export = conduitos::observatory::prepare_export(
        &record,
        &identities,
        &offer,
        &prepared,
        BUILD_ID,
        IMAGE_ID,
        None,
    )
    .unwrap_or_else(|error| refuse(error.as_str()));
    let allocation_before_play = BOOT_ARENA.seal();

    let mut clock = arch::Clock::new();
    let mut timer = arch::Timer::new();
    let mut serial = arch::Serial::new();
    let mut interrupts = arch::Interrupts::new();
    let mut idle = arch::Idle::new();
    let report = conduitos::dual_region_composition::run(
        &mut prepared.kernel,
        &mut clock,
        &mut timer,
        &mut serial,
        &mut interrupts,
        &mut idle,
    )
    .unwrap_or_else(|error| refuse(error.as_str()));
    let sign = sign_format::machine_accepted(
        &identities,
        &offer,
        &report,
        &prepared,
        sign_format::AllocationReceipt {
            before_play: allocation_before_play,
            after_play: BOOT_ARENA.used(),
            capacity: BOOT_ARENA.capacity(),
        },
        BUILD_ID,
    )
    .unwrap_or_else(|_| refuse("kernel-sign-storage-full"));
    arch::present(sign.as_bytes());
    arch::present(conduitos::observatory::EXPORT_PREFIX.as_bytes());
    arch::present(observatory_export.as_bytes());
    arch::present(b"\n");
    arch::present(b"CONDUIT_AARCH64_A3_IDENTITY {\"image_id\":\"");
    arch::present(IMAGE_ID.as_bytes());
    arch::present(b"\",\"wake_source\":\"arm-generic-virtual-timer-ppi-27\",\"wake_irq\":27,\"a3_ordinary_form_claimed\":true}\n");
    exit(true)
}

fn refuse(reason: &str) -> ! {
    arch::disable_interrupts();
    arch::present(b"CONDUIT_AARCH64_REFUSAL ");
    arch::present(reason.as_bytes());
    arch::present(b"\n");
    exit(false)
}

fn exit(success: bool) -> ! {
    if success {
        unsafe { core::arch::asm!("hvc #0", in("x0") 0x8400_0008_u64, options(noreturn)) }
    }
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    refuse("panic")
}
