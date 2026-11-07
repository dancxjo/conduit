//! Shared authenticated Root entry. Physical effects require admitted execution.
use alloc::{string::String, vec, vec::Vec};
use conduit_core::{BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration};
use conduit_esp32_electrical::{uart::Uart0Carrier, Esp32PulseCapture, Esp32Tm1637};
use conduit_mcu_host_runtime::{
    root::{Configuration, Request, Root},
    seed::Seed,
    transport::Link,
};
use embassy_time::{with_timeout, Duration, Timer};

pub async fn run(
    seed: Seed<'static>,
    host: String,
    nonce: [u8; 16],
    carrier: Uart0Carrier<'static>,
    equipment: (Esp32PulseCapture<'static>, Esp32Tm1637<'static>),
) -> ! {
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
    let equipment = crate::equipment::Equipment::new(&actual, equipment.0, equipment.1);
    equipment
        .publish(&mut actual)
        .expect("bounded actual native publication");
    let mut root = Root::<{ crate::config::MAX_ASSIGNMENT_BYTES }>::new(
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
    let mut response = [0u8; crate::config::MAX_ASSIGNMENT_BYTES];
    // Owned here until the shared native issuer/factory consumes them. This
    // binary is not deployable until the whole-workset execution bridge is wired.
    let equipment = equipment;
    let mut issuers = crate::authority::NativeIssuers::new();
    let operations = crate::authority::operations();
    let residents = match crate::resident::Initialized::prepare() {
        Ok(initialized) => initialized,
        Err(_) => panic!("checked resident inventory initialization refused"),
    };
    loop {
        let result = match root.receive_request(&mut link).await {
            Ok(Request::Spawn) => root.spawn(&mut link, &mut response).await,
            Ok(Request::Prepare) => {
                let mut nonce = [0u8; 32];
                nonce[..16].copy_from_slice(
                    &conduit_esp32_electrical::platform::fresh_boot_nonce().expect("entropy"),
                );
                nonce[16..].copy_from_slice(
                    &conduit_esp32_electrical::platform::fresh_boot_nonce().expect("entropy"),
                );
                match with_timeout(
                    Duration::from_secs(10),
                    root.receive_preparation(
                        &mut link,
                        nonce,
                        conduit_esp32_electrical::platform::monotonic_millis,
                    ),
                )
                .await
                {
                    Ok(result) => result,
                    Err(_) => {
                        root.clear_unretained_input();
                        Err(conduit_mcu_host_runtime::root::Refusal::Preparation)
                    }
                }
            }
            Ok(Request::Return) => {
                match crate::authority::authorize(&mut root, &equipment, &mut issuers, &operations)
                {
                    Err(error) => Err(error),
                    Ok(()) => {
                        let images = residents
                            .offers()
                            .into_iter()
                            .flatten()
                            .chain([
                                crate::native_images::PULSE_IMAGE,
                                crate::native_images::DISPLAY_IMAGE,
                                crate::native_images::RENDERER_IMAGE,
                                crate::native_images::WAIT_IMAGE,
                            ])
                            .chain(crate::workset::sampling::OFFER_IMAGES.iter().copied())
                            .chain(crate::workset::mask::OFFER_IMAGES.iter().copied())
                            .chain(crate::workset::mask::MASK_ROUTING_IMAGES.iter().copied())
                            .chain(crate::workset::output::OFFER_IMAGES.iter().copied());
                        root.return_member(
                            &mut link,
                            images,
                            || {
                                issuers
                                    .copy_grants(
                                        crate::config::HEAP_BYTES.saturating_sub(allocator_used()),
                                    )
                                    .map_err(|_| {
                                        conduit_mcu_host_runtime::member_return::Refusal::Bounds
                                    })
                            },
                            &mut response,
                        )
                        .await
                    }
                }
            }
            Err(error) => Err(error),
        };
        if result.is_err() {
            link.reset();
        }
        Timer::after_millis(10).await;
    }
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
    esp_alloc::HEAP.used()
}
