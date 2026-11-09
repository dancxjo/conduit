//! RP2040 composition of the shared authenticated Root and actual providers.
//! This construction is not deployable until its complete execution factory is
//! connected; no diagnostic electrical task is used as a substitute for Play.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration};
use conduit_mcu_host_runtime::root::{Configuration, RootStorage};
pub async fn run(
    seed: crate::seed::Seed<'static>,
    host: String,
    nonce: [u64; 2],
    link: &mut crate::transport::Link,
    flash: crate::credential_source::Device,
) -> ! {
    // SAFETY: this Root is the sole owner of the arena initialized from the
    // same independently checked IMAGE; no received assignment selects its size.
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
    let mut actual = HostAdvertisement {
        declared_name: None,
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(format!("rp2040/boot/{:016x}{:016x}", nonce[0], nonce[1])),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from(crate::config::CONFIGURATION_ID),
        bases: Vec::new(),
        resources: vec![conduit_core::resource_offer(
            crate::config::RESIDENT_WORK_POOL,
            conduit_core::COOPERATIVE_STEP_WORK_CLASS_ID,
            crate::config::RESIDENT_WORK_UNITS,
        )],
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    };
    let equipment = crate::equipment::Equipment::new(&actual, &seed.body_id, flash)
        .expect("actual initialized Yellow providers");
    equipment
        .publish(&mut actual)
        .expect("bounded compact native facts");
    let mut storage = RootStorage::<16384>::new();
    let root = storage
        .initialize(
            seed.into_shared(),
            actual,
            Configuration {
                image_id: crate::config::IMAGE_ID,
                configuration_id: crate::config::CONFIGURATION_ID,
                friendly_label: "Yellow",
                preparation: crate::config::PREPARATION_DELEGATION.as_ref(),
                execution: crate::config::EXECUTION_DELEGATION.as_ref(),
            },
        )
        .expect("original checked Root");
    let future = core::pin::pin!(shared::run(root, equipment, arena, link));
    conduit_mcu_host_runtime::poll_boundary::separate(future).await
}
mod shared {
    include!(concat!(env!("OUT_DIR"), "/source_runtime.rs"));
}

pub fn now_ms() -> u64 {
    embassy_time::Instant::now().as_millis()
}
pub fn allocator_used() -> usize {
    crate::heap::usage().0
}

pub fn remaining() -> usize {
    crate::config::HEAP_BYTES.saturating_sub(allocator_used())
}

pub fn fresh_challenge() -> [u8; 32] {
    let mut rng = embassy_rp::clocks::RoscRng;
    let mut challenge = [0; 32];
    for chunk in challenge.chunks_exact_mut(8) {
        chunk.copy_from_slice(&rng.next_u64().to_le_bytes());
    }
    challenge
}
pub fn observe_receipt(digest: Option<[u8; 32]>) {
    crate::membership::observe_receipt(digest)
}
pub fn allocator_peak() -> Option<usize> {
    Some(crate::heap::usage().1)
}
