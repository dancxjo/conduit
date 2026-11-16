//! RP2040 boot adapter to the ordinary Root and complete neutral factory.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration};
use conduit_mcu_host_runtime::root::{Configuration, RootStorage};
pub async fn run(
    seed: crate::seed::Seed<'static>,
    host: String,
    nonce: [u64; 2],
    link: &mut crate::transport::Link,
) -> ! {
    // SAFETY: the same original checked IMAGE owns the sole configured heap.
    let memory = unsafe {
        conduit_mcu_host_runtime::image_memory::StagedImageMemory::verified_heap(
            crate::config::HEAP_BYTES,
        )
    }
    .expect("original IMAGE");
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
    let mut equipment = crate::neutral_equipment::Equipment::prepare(&actual, remaining())
        .expect("original native owners");
    equipment
        .publish(&mut actual, remaining())
        .expect("exact native publication");
    let mut storage = RootStorage::<{ crate::config::MAX_ASSIGNMENT_BYTES }>::new();
    let root = storage
        .initialize(
            seed.into_shared(),
            actual,
            Configuration {
                image_id: crate::config::IMAGE_ID,
                configuration_id: crate::config::CONFIGURATION_ID,
                friendly_label: "RP2040",
                preparation: crate::config::PREPARATION_DELEGATION.as_ref(),
                execution: crate::config::EXECUTION_DELEGATION.as_ref(),
            },
        )
        .expect("original authenticated Root");
    let mut residents = crate::resident::inventory().expect("original resident owners");
    // Charge all preexisting Host, Seed, Root and inventory allocations once
    // before leasing the remainder. Neither charge creates allocator backing.
    let baseline_bytes = allocated();
    let _baseline = if baseline_bytes == 0 {
        None
    } else {
        Some(
            memory
                .reserve(baseline_bytes)
                .expect("original Root baseline backing"),
        )
    };
    let arena = memory
        .reserve_remaining()
        .expect("original preparation arena");
    let future = core::pin::pin!(crate::neutral_service::serve(
        root,
        &mut equipment,
        &mut residents,
        link,
        crate::neutral_service::Runtime {
            now: now_ms,
            allocated_bytes: allocated,
            challenge
        },
        arena.bytes()
    ));
    conduit_mcu_host_runtime::poll_boundary::separate(future).await
}
fn allocated() -> usize {
    crate::heap::usage().0
}
fn remaining() -> usize {
    crate::config::HEAP_BYTES.saturating_sub(allocated())
}
fn now_ms() -> u64 {
    embassy_time::Instant::now().as_millis()
}
fn challenge() -> [u8; 32] {
    let mut rng = embassy_rp::clocks::RoscRng;
    let mut result = [0; 32];
    for chunk in result.chunks_exact_mut(8) {
        chunk.copy_from_slice(&rng.next_u64().to_le_bytes());
    }
    result
}
