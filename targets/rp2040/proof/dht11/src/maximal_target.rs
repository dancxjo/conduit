//! Comparative target-memory gate, not a delivered Host or a started Play.
//! Retains actual PIO/timer owners, checked software, installed Body and radio.
extern crate alloc;
use crate::{probe_heap, usb_link::UsbLinkSession};
use alloc::{boxed::Box, format};
use conduit_core as c;
use conduit_monotonic_wait::*;
use conduit_pulse_capture::{installation::*, owner::PulseAttachment, PulseReadContract};
use core::fmt::Write;
use embassy_time::{Duration, Instant, Timer};
#[path = "maximal_planning.rs"]
mod planning;
mod material {
    include!(concat!(env!("OUT_DIR"), "/appliance-software.rs"));
}
struct Wait;
impl MonotonicWaitProvider for Wait {
    async fn wait(&mut self, ms: u64) -> Result<(), WaitProviderFailure> {
        if ms > 60000 {
            return Err(WaitProviderFailure::ProviderUnavailable);
        }
        Timer::after_millis(ms).await;
        Ok(())
    }
}
pub(super) async fn mark(link: &mut UsbLinkSession, phase: &str, item: usize) {
    let (peak, retained, _) = probe_heap::ALLOCATOR.observation();
    let mut text = heapless::String::<384>::new();
    writeln!(text,"{{\"event\":\"pico-maximal-memory\",\"phase\":\"{phase}\",\"item\":{item},\"heap_capacity\":{},\"heap_peak\":{peak},\"heap_retained\":{retained},\"play_started\":false}}",probe_heap::CAPACITY).unwrap();
    let _ = link.write_text(&text).await;
}
pub async fn run(
    electrical: conduit_rp2040_pulse_capture::PicoPulseCapture<
        'static,
        embassy_rp::peripherals::PIO1,
        0,
    >,
    link: &mut UsbLinkSession,
    network: crate::http_radio::Resources,
    uid: [u8; 8],
) {
    link.wait_connection().await;
    while !link.class.dtr() {
        Timer::after_millis(25).await;
    }
    if let Some(fault) = crate::kernel_fault::take() {
        let mut text = heapless::String::<384>::new();
        writeln!(text,"{{\"event\":\"pico-maximal-fault\",\"kind\":{},\"heap_peak\":{},\"heap_retained\":{},\"requested\":{},\"line\":{}}}",fault[1],fault[2],fault[3],fault[4],fault[6]).unwrap();
        let _ = link.write_text(&text).await;
        loop {
            let _ = crate::bootsel::wait_for_request(link).await;
        }
    }
    mark(link, "begin", 0).await;
    let host = format!(
        "rp2040/{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        uid[0], uid[1], uid[2], uid[3], uid[4], uid[5], uid[6], uid[7]
    );
    let mut rng = embassy_rp::clocks::RoscRng;
    let boot = format!("boot/{:016x}{:016x}", rng.next_u64(), rng.next_u64());
    let mut ad = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: host.into(),
        boot_id: boot.into(),
        offer_generation: c::OfferGeneration(1),
        profile: "environment/rp2040-maximal-memory@1".into(),
        bases: alloc::vec![],
        resources: alloc::vec![],
        capabilities: alloc::vec![],
        planner_capabilities: alloc::vec![],
    };
    let pulse_identity = PulseNativeIdentity {
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        base_id: "base/rp2040/gp2".into(),
        provider_instance_id: "instance/rp2040/gp2".into(),
        provider_generation: 1,
        resource_pool_id: "pool/rp2040/gp2".into(),
        resource_generation_id: c::ResourceGenerationId("generation/gp2/1".into()),
        envelope_id: "envelope/rp2040/gp2".into(),
        artifact_id: "native/rp2040-pio-pulse@1".into(),
        implementation_id: "rp2040/gp2-pulse@1".into(),
        base_implementation_id: "rp2040/gp2-pulse-base@1".into(),
    };
    // SAFETY: consumes this Boot's actual PIO1/SM0, DMA2 and GP2 provider;
    // its bounded capture and cancellation laws are owned by that adapter.
    let pulse = unsafe {
        ReadyPulseBase::new(
            pulse_identity,
            PulseAttachment {
                generation: 1,
                resource_bytes: 64,
                limits: conduit_rp2040_pulse_capture::LIMITS,
            },
            electrical,
        )
    }
    .unwrap();
    pulse
        .append_to_advertisement(&mut ad, &PulseReadContract::prepare().unwrap())
        .unwrap();
    let mut provider = Wait;
    let wait_bytes = core::mem::size_of_val(&provider.wait(60000)) as u64;
    // SAFETY: Embassy initialized the hardware monotonic clock before main;
    // this unique bounded provider owns one cancellation-safe pending wait.
    let wait = unsafe {
        ReadyMonotonicWaitBase::new(
            WaitNativeIdentity {
                host_id: ad.host_id.clone(),
                boot_id: ad.boot_id.clone(),
                base_id: "base/rp2040/wait".into(),
                provider_instance_id: "instance/rp2040/wait".into(),
                provider_generation: 1,
                resource_pool_id: "pool/rp2040/wait".into(),
                resource_generation_id: c::ResourceGenerationId("generation/wait/1".into()),
                envelope_id: "envelope/rp2040/wait".into(),
                artifact_id: "native/rp2040-wait@1".into(),
                implementation_id: "rp2040/wait@1".into(),
                base_implementation_id: "rp2040/wait-base@1".into(),
            },
            WaitAttachment {
                generation: 1,
                resource_bytes: wait_bytes,
                maximum_duration_ms: 60000,
            },
            provider,
        )
    }
    .unwrap();
    wait.append_to_advertisement(&mut ad).unwrap();
    ad.bases.sort_by(|a, b| a.base_id.cmp(&b.base_id));
    ad.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    mark(link, "native-offers", ad.capabilities.len()).await;
    let evidence: conduit_body::BodyBiographyEvidence = serde_json::from_slice(include_bytes!(
        concat!(env!("OUT_DIR"), "/body-biography.json")
    ))
    .unwrap();
    let body = Box::new(
        conduit_environment_runtime::body_image::open_installed_image_sealed(evidence, &ad)
            .unwrap(),
    );
    let mut report = heapless::String::<512>::new();
    writeln!(report,"{{\"event\":\"pico-body-loaded\",\"body\":\"{}\",\"host\":\"{}\",\"boot\":\"{}\",\"member_admitted\":true,\"play_started\":false}}",body.evidence().body_id.as_str(),ad.host_id.as_str(),ad.boot_id.as_str()).unwrap();
    let _ = link.write_text(&report).await;
    let software = Box::new(
        conduit_environment_runtime::appliance::assigned::PreparedResidentSoftware::prepare_source(
            &material::spec(),
            material::computations(),
            conduit_environment_runtime::appliance::prepare_value_storage().unwrap(),
        )
        .unwrap(),
    );
    mark(link, "resident-software-body", 0).await;
    crate::http_radio::launch(
        network,
        conduit_face::FaceBasis {
            body_id: Some(body.evidence().body_id.clone()),
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: alloc::vec![],
        },
    );
    let deadline = Instant::now() + Duration::from_secs(55);
    while !crate::http_view::ap_ready() && Instant::now() < deadline {
        Timer::after_millis(50).await;
    }
    if !crate::http_view::ap_ready() {
        mark(link, "radio-unavailable", 0).await;
    } else {
        mark(link, "initialized-body-software-radio", 0).await;
        let offers = software
            .source_offers(&material::spec(), material::offers())
            .unwrap();
        for (i, json) in offers.iter().enumerate() {
            mark(link, "before-software-offer", i).await;
            ad.capabilities.push(serde_json::from_slice(json).unwrap());
            mark(link, "after-software-offer", i).await;
        }
        planning::measure(link, &ad).await;
    }
    // Retain the actual owners for the complete measurement, without starting
    // a fixture Play or issuing native authority from a serialized grant.
    core::hint::black_box((&pulse, &wait, &software, &body));
    mark(link, "checkpoint-finished", 0).await;
    loop {
        let _ = crate::bootsel::wait_for_request(link).await;
    }
}
