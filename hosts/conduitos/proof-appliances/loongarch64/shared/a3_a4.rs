#[cfg(not(target_arch = "loongarch64"))]
compile_error!("the shared ConduitOS LoongArch64 A3/A4 implementation must compile as LoongArch64");

use core::panic::PanicInfo;

use conduitos::{allocation::BOOT_ARENA, arch, dual_region_plan, identity, sign_format};

const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
const ARTIFACT_COMMIT: &str = env!("CONDUITOS_ARTIFACT_COMMIT");
const IMAGE_ID: &str = env!("CONDUITOS_IMAGE_ID");

static mut MEMORY_ARENA: [u8; 1024 * 1024] = [0; 1024 * 1024];

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.conduitos_loongarch64_a3_start")]
pub extern "C" fn conduitos_loongarch64_a3_start() -> ! {
    unsafe {
        BOOT_ARENA
            .initialize(
                core::ptr::addr_of_mut!(MEMORY_ARENA) as *mut u8 as usize,
                1024 * 1024,
            )
            .unwrap_or_else(|_| refuse("memory-base-unavailable"));
    }
    if !arch::initialize_machine() {
        refuse("unavailable-or-stale-trap-controller");
    }
    let counter = arch::read_counter();
    entry_sign(counter);
    let identities = identity::derive(
        [
            counter,
            counter.rotate_left(13),
            counter.rotate_left(29),
            counter.rotate_left(47),
        ],
        counter,
        0xffff_ffff_8000_0000,
    );
    let offer = conduitos::offer::HostOffer::new(
        &identities,
        BUILD_ID,
        conduitos::offer::CpuFeatures {
            sse2: false,
            rdrand: false,
            invariant_tsc: false,
        },
        1024 * 1024,
    );
    offer
        .validate()
        .unwrap_or_else(|error| refuse(error.as_str()));
    let mut prepared = dual_region_plan::prepare(&identities, &offer, BUILD_ID)
        .unwrap_or_else(|error| refuse(error.as_str()));
    #[cfg(feature = "loongarch64-a4")]
    let observatory_export = {
        let record = conduitos::boot::BootRecord {
            firmware: conduitos::boot::Firmware::Uefi64,
            timestamp: counter,
            hhdm_offset: 0,
            image_physical_start: 0xffff_ffff_8000_0000,
            image_length: 0,
            memory_region_count: 1,
            artifact_count: 0,
            framebuffer_count: 0,
            command_line_bytes: 0,
            runtime_arena: conduitos::boot::RuntimeArena {
                physical_start: core::ptr::addr_of!(MEMORY_ARENA) as u64,
                length: 1024 * 1024,
            },
        };
        conduitos::observatory::prepare_export(
            &record,
            &identities,
            &offer,
            &prepared,
            BUILD_ID,
            IMAGE_ID,
            None,
        )
        .unwrap_or_else(|error| refuse(error.as_str()))
    };
    let before = BOOT_ARENA.seal();
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
            before_play: before,
            after_play: BOOT_ARENA.used(),
            capacity: BOOT_ARENA.capacity(),
        },
        BUILD_ID,
    )
    .unwrap_or_else(|_| refuse("kernel-sign-storage-full"));
    arch::present(sign.as_bytes());
    #[cfg(feature = "loongarch64-a4")]
    {
        arch::present(conduitos::observatory::EXPORT_PREFIX.as_bytes());
        arch::present(observatory_export.as_bytes());
        arch::present(b"\n");
    }
    arch::present(b"CONDUIT_LOONGARCH64_A3_IDENTITY {\"image_id\":\"");
    arch::present(IMAGE_ID.as_bytes());
    #[cfg(feature = "loongarch64-a4")]
    arch::present(b"\",\"wake_source\":\"loongarch-local-timer-interrupt\",\"wake_cause\":11,\"timer_mechanism\":\"TCFG/TICLR\",\"a3_ordinary_form_claimed\":true,\"a4_observatory_patchbay_claimed\":true}\n");
    #[cfg(not(feature = "loongarch64-a4"))]
    arch::present(b"\",\"wake_source\":\"loongarch-local-timer-interrupt\",\"wake_cause\":11,\"timer_mechanism\":\"TCFG/TICLR\",\"a3_ordinary_form_claimed\":true,\"a4_observatory_patchbay_claimed\":false}\n");
    loop {
        core::hint::spin_loop();
    }
}

fn entry_sign(nonce: u64) {
    let mut output = Output::new();
    output.push(b"CONDUIT_LOONGARCH64_ENTRY_SIGN {\"schema\":\"conduit.conduitos.loongarch64-entry-sign/v1\",\"status\":\"entered\",\"architecture\":\"loongarch64\",\"build_id\":\"");
    output.push(ARTIFACT_COMMIT.as_bytes());
    output.push(b"\",\"image_id\":\"");
    output.push(IMAGE_ID.as_bytes());
    output.push(b"\",\"bootloader\":\"Limine 12.5.2/BOOTLOONGARCH64.EFI\",\"emulator_profile\":\"qemu-loongarch64-virt-single-cpu-2g-edk2\",\"firmware\":\"EDK2 QEMU_EFI.fd (mechanism only)\",\"host_id\":\"host-loongarch64-");
    output.hex(nonce.rotate_left(17) ^ 0x434f_4e44_5549_5401);
    output.push(b"\",\"boot_id\":\"boot-loongarch64-");
    output.hex(nonce ^ 0x4c41_3634_0000_0001);
    output.push(b"\",\"runtime_bases_available\":false,\"a2_machine_wake_claimed\":false}\n");
    arch::present(output.bytes());
}

fn refuse(reason: &str) -> ! {
    arch::disable_interrupts();
    arch::present(b"CONDUIT_LOONGARCH64_REFUSAL ");
    arch::present(reason.as_bytes());
    arch::present(b"\n");
    loop {
        core::hint::spin_loop();
    }
}

struct Output {
    bytes: [u8; 1024],
    len: usize,
}
impl Output {
    const fn new() -> Self {
        Self {
            bytes: [0; 1024],
            len: 0,
        }
    }
    fn push(&mut self, value: &[u8]) {
        let end = self.len + value.len();
        if end > self.bytes.len() {
            refuse("sign-capacity-exhausted");
        }
        self.bytes[self.len..end].copy_from_slice(value);
        self.len = end;
    }
    fn hex(&mut self, value: u64) {
        let mut bytes = [0_u8; 16];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = b"0123456789abcdef"[((value >> ((15 - index) * 4)) & 0xf) as usize];
        }
        self.push(&bytes);
    }
    fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    refuse("panic")
}
