//! Actual ESP32 board entry; shared Source composition owns admitted work.
use alloc::{string::String, vec, vec::Vec};
use conduit_core::{BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration};
use conduit_esp32_electrical::{uart::Uart0Carrier, Esp32PulseCapture, Esp32Tm1637};
use conduit_mcu_host_runtime::{
    root::{Configuration, RootStorage},
    seed::Seed,
    transport::Link,
};
use embassy_time::Timer;
pub async fn run(
    seed: Seed<'static>,
    host: String,
    nonce: [u8; 16],
    carrier: Uart0Carrier<'static>,
    equipment: (Esp32PulseCapture<'static>, Esp32Tm1637<'static>),
    flash: esp_hal::peripherals::FLASH<'static>,
) -> ! {
    // SAFETY: same original checked IMAGE constructed the sole physical heap.
    let image_memory = unsafe {
        conduit_mcu_host_runtime::image_memory::StagedImageMemory::verified_heap(
            crate::config::HEAP_BYTES,
        )
    }
    .expect("original IMAGE arena");
    // One coarse reservation covers initialization, preparation and retained
    // execution before any Ready owner is constructed. It never claims peak fit.
    let arena = image_memory
        .reserve_remaining()
        .expect("whole original IMAGE arena");
    let boot = nonce.iter().fold(String::from("esp32/boot/"), |mut s, b| {
        use core::fmt::Write;
        write!(&mut s, "{b:02x}").expect("String write");
        s
    });
    let mut actual = HostAdvertisement {
        declared_name: None,
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from(crate::config::CONFIGURATION_ID),
        bases: Vec::new(),
        resources: vec![conduit_core::resource_offer(
            crate::config::WORK_POOL,
            conduit_core::COOPERATIVE_STEP_WORK_CLASS_ID,
            crate::config::WORK_UNITS,
        )],
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    };
    let slot = conduit_esp32_snapshot_nor::SlotGeometry::checked(
        conduit_esp32_snapshot_nor::DURABLE_START,
        conduit_esp32_snapshot_nor::DURABLE_END - conduit_esp32_snapshot_nor::DURABLE_START,
    )
    .expect("checked durable partition extent");
    let storage = conduit_esp32_snapshot_nor::hardware::prepare(flash, slot)
        .map_err(|(_, refusal)| refusal)
        .expect("actual sole flash owner");
    let equipment =
        crate::equipment::Equipment::new(&actual, &seed.body_id, equipment.0, equipment.1, storage);
    equipment
        .publish(&mut actual)
        .expect("bounded actual native publication");
    let mut storage = RootStorage::<{ crate::config::MAX_ASSIGNMENT_BYTES }>::new();
    let root = storage
        .initialize(
            seed,
            actual,
            Configuration {
                image_id: crate::config::IMAGE_ID,
                configuration_id: crate::config::CONFIGURATION_ID,
                friendly_label: "Blue",
                preparation: crate::config::PREPARATION.as_ref(),
                execution: crate::config::EXECUTION.as_ref(),
            },
        )
        .expect("original checked Root");
    let mut link = Link::new(carrier);
    let future = core::pin::pin!(shared::run(root, equipment, arena, &mut link));
    conduit_mcu_host_runtime::poll_boundary::separate(future).await
}
mod shared {
    include!(concat!(env!("OUT_DIR"), "/source_runtime.rs"));
}
/// Owns an initialized ESP RTOS monotonic timer capability. A cancelled wait
/// drops the Embassy timer and leaves no hardware effect awaiting retry.
pub struct MonotonicTimer {
    _slot: u8,
}
impl MonotonicTimer {
    pub(crate) fn new(slot: usize) -> Self {
        assert!(slot < crate::config::WAIT_SLOTS);
        Self { _slot: slot as u8 }
    }
}
impl conduit_monotonic_wait::MonotonicWaitProvider for MonotonicTimer {
    async fn wait(
        &mut self,
        duration_ms: u64,
    ) -> Result<(), conduit_monotonic_wait::WaitProviderFailure> {
        Timer::after_millis(duration_ms).await;
        Ok(())
    }
}

/// Shared preparation charges the actual initialized allocator, not a declared
/// envelope or accumulated guessed allocation sizes.
pub fn allocator_used() -> usize {
    let used = esp_alloc::HEAP.used();
    crate::panic_capture::observe_heap(used);
    used
}

pub fn now_ms() -> u64 {
    conduit_esp32_electrical::platform::monotonic_millis()
}
pub fn remaining() -> usize {
    crate::config::HEAP_BYTES.saturating_sub(allocator_used())
}

pub fn fresh_challenge() -> [u8; 32] {
    let mut challenge = [0; 32];
    challenge[..16]
        .copy_from_slice(&conduit_esp32_electrical::platform::fresh_boot_nonce().expect("entropy"));
    challenge[16..]
        .copy_from_slice(&conduit_esp32_electrical::platform::fresh_boot_nonce().expect("entropy"));
    challenge
}
pub fn observe_receipt(_digest: Option<[u8; 32]>) {}
// The ESP allocator exposes current usage, not an independently measured peak.
pub fn allocator_peak() -> Option<usize> {
    None
}

/// Fixed, once-per-stage startup diagnostic; never execution authority.
#[inline(always)]
pub fn stack_checkpoint(phase: usize) {
    let _ = phase;
}
