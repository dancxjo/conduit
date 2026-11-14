//! Existing DHT/TM selected-Source fabrication. Native descriptions remain
//! inert exact checked IMAGE inputs; actual provider readiness/admission is Root-owned.
use super::recording_build;

#[allow(clippy::too_many_arguments)]
pub fn prepare(
    checked: &conduit_host_make::CheckedHostConfiguration,
    catalog: &conduit_host_make::MakeCatalog,
    image_id: &str,
    source_identity: &str,
    out: &std::path::Path,
    renderer: conduit_host_rp2040::CheckedRendererConstruction,
    recording: recording_build::Recording,
) -> Vec<conduit_embedded_build::PreparedResidentExpression> {
    use conduit_host_rp2040::source_build::{
        image_input, native_calibration as calibration, workset,
    };
    use conduit_mcu_host_runtime::execution_mask;
    let image = image_input::from_environment(checked, catalog, image_id, source_identity)
        .expect("original ordinary Make IMAGE");
    let clock = conduit_host_rp2040::CheckedClockConstruction::from_configuration(
        checked,
        conduit_host_rp2040::CLOCK_IMPLEMENTATION,
    )
    .expect("checked finite timer slots");
    let mask = Some(workset::MaskBounds {
        face_bytes: renderer.face_bytes,
        show_bytes: renderer.show_bytes,
    });
    let mut workset=workset::from_environment(mask).expect("explicit checked workset Source").expect("DHT execution requires explicit --workset-source and all selected --workset-entry inputs");
    println!("cargo:rustc-cfg=shared_source_execution");
    let resident = workset
        .prepare_selected_residents(
            conduit_host_rp2040::source_build::dht11_tm1637_resident_bounds(),
        )
        .expect("actual combined checked Source resident definitions");
    let artifact = conduit_core::ArtifactId::from(image_id);
    let pulse = conduit_pulse_capture::installation::capability_offer(
        &conduit_pulse_capture::PulseReadContract::prepare().unwrap(),
        &artifact,
        &conduit_core::ImplementationId::from(conduit_host_rp2040::DHT11_IMPLEMENTATION),
    )
    .unwrap();
    let display = conduit_tm1637_wire::installation::capability_offer(
        &conduit_tm1637_wire::WriteContract::prepare().unwrap(),
        &artifact,
        &conduit_core::ImplementationId::from(conduit_host_rp2040::TM1637_IMPLEMENTATION),
    )
    .unwrap();
    let wait = conduit_monotonic_wait::capability_offer(
        &artifact,
        &conduit_core::ImplementationId::from(conduit_host_rp2040::CLOCK_IMPLEMENTATION),
    )
    .unwrap();
    let renderer_offer = execution_mask::capability_offer(
        conduit_host_rp2040::RENDERER_IMPLEMENTATION.into(),
        artifact.clone(),
        execution_mask::Bounds {
            face_bytes: renderer.face_bytes,
            show_bytes: renderer.show_bytes,
            resource_bytes: renderer.resource_bytes,
            maximum_frames: renderer.maximum_frames,
        },
    );
    conduit_host_rp2040::source_build::native_build::check_source_clocks(&workset, clock)
        .expect("checked Source clock construction");
    let recorder = recording
        .description(
            workset
                .entries
                .iter()
                .flat_map(|entry| &entry.plot.expanded.gears)
                .map(|gear| gear.kind_id.as_str()),
            &artifact,
        )
        .expect("Source recorder matches explicit checked Base");
    let append = recorder
        .as_ref()
        .map(|description| description.definition.clone());
    let mut descriptions = vec![
        calibration::NativeDescription {
            slot: calibration::Slot {
                base: "rp2040/gpio-pulse",
                pool: "pool/rp2040/gpio-pulse",
                implementation: conduit_host_rp2040::DHT11_IMPLEMENTATION,
            },
            class: conduit_pulse_capture::installation::PULSE_ATTACHMENT_CLASS,
            mechanism: conduit_pulse_capture::installation::PULSE_ATTACHMENT_CLASS,
            definition: pulse,
        },
        calibration::NativeDescription {
            slot: calibration::Slot {
                base: "rp2040/tm1637",
                pool: "pool/rp2040/tm1637",
                implementation: conduit_host_rp2040::TM1637_IMPLEMENTATION,
            },
            class: conduit_tm1637_wire::installation::WRITE_ATTACHMENT_CLASS,
            mechanism: conduit_tm1637_wire::installation::WRITE_ATTACHMENT_CLASS,
            definition: display,
        },
        calibration::NativeDescription {
            slot: calibration::Slot {
                base: "rp2040/wait/0",
                pool: "pool/rp2040/wait/0",
                implementation: conduit_host_rp2040::CLOCK_IMPLEMENTATION,
            },
            class: conduit_monotonic_wait::WAIT_ATTACHMENT_CLASS,
            mechanism: conduit_monotonic_wait::WAIT_ATTACHMENT_CLASS,
            definition: wait,
        },
        calibration::NativeDescription {
            slot: calibration::Slot {
                base: "rp2040/face-renderer",
                pool: "pool/rp2040/face-renderer",
                implementation: conduit_host_rp2040::RENDERER_IMPLEMENTATION,
            },
            class: execution_mask::ATTACHMENT,
            mechanism: execution_mask::ATTACHMENT,
            definition: renderer_offer,
        },
    ];
    if let Some(description) = recorder {
        descriptions.push(description);
    }

    conduit_host_rp2040::source_build::native_build::emit_capability_images(
        out,
        &[
            ("PULSE_IMAGE", &descriptions[0].definition),
            ("DISPLAY_IMAGE", &descriptions[1].definition),
            ("WAIT_IMAGE", &descriptions[2].definition),
            ("RENDERER_IMAGE", &descriptions[3].definition),
        ],
        append.as_ref(),
        24 * 1024,
    )
    .expect("finite checked native images");
    let work = conduit_host_rp2040::CheckedResidentWorkPool::from_configuration(checked).unwrap();
    let shared = [conduit_core::resource_offer(
        &work.id,
        conduit_core::COOPERATIVE_STEP_WORK_CLASS_ID,
        work.capacity_units,
    )];
    let routing = execution_mask::routing_offers(renderer.face_bytes, renderer.show_bytes).unwrap();
    let bounds = calibration::Bounds::for_workset(
        checked,
        &workset,
        conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: conduit_host_rp2040::source_build::dht11_tm1637_resident_bounds(
            )
            .maximum_program_bytes,
            maximum_total_program_bytes:
                conduit_host_rp2040::source_build::dht11_tm1637_resident_bounds()
                    .maximum_program_bytes,
        },
    )
    .expect("finite checked Source lowering bounds");
    conduit_host_rp2040::source_build::native_build::calibrate_and_emit(
        &mut workset,
        mask,
        out,
        calibration::Input {
            checked,
            image: &image,
            catalog,
            shared_resources: &shared,
            native: &descriptions,
            other: &routing,
            resident: &resident,
            bounds,
        },
    )
    .expect("independent native calibration and emission");
    resident
}
