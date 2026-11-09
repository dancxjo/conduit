//! Checked physical Host construction, separate from runtime admission.
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rustc-link-arg=-Tlinkall.x");
    for key in [
        "CONDUIT_HOST_CONFIGURATION",
        "CONDUIT_SOURCE_IDENTITY",
        "CONDUIT_HOST_IMAGE_ID",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let image = env::var("CONDUIT_HOST_IMAGE_ID").expect("use checked Make IMAGE");
    let source_identity =
        env::var("CONDUIT_SOURCE_IDENTITY").expect("use checked committed Source");
    let path = PathBuf::from(
        env::var_os("CONDUIT_HOST_CONFIGURATION").expect("explicit checked Host Source"),
    );
    println!("cargo:rerun-if-changed={}", path.display());
    let source = fs::read_to_string(&path).expect("Host Source");
    let packages =
        conduit_host_make::MakePackageSet::compose(&[&conduit_host_esp32_make::Esp32MakePackage])
            .unwrap();
    let catalog = conduit_host_make::MakeCatalog::canonical().with_packages(&packages);
    let parsed =
        conduit_host_make::parse_host_configuration_conduit(&source).expect("Host Source syntax");
    let checked = conduit_host_make::check_host_configuration(parsed, &catalog, &packages)
        .expect("checked physical Host Source");
    let physical =
        conduit_host_esp32_make::physical::CheckedPhysicalConstruction::from_configuration(
            &checked,
        )
        .expect("exact owned equipment");
    let renderer =
        conduit_mcu_source_build::renderer::CheckedRendererConstruction::from_configuration(
            &checked,
            conduit_host_esp32_make::physical::RENDERER,
        )
        .expect("checked finite Face renderer construction");
    let config = checked.configuration();
    let mut work = config
        .resources
        .iter()
        .filter(|r| r.class == conduit_core_work_class());
    let work = work
        .next()
        .filter(|_| work.next().is_none())
        .expect("one finite admitted work-pool construction");
    assert!(work.bytes <= config.limits.heap_arena_bytes && work.slots > 0);
    assert!(
        config
            .preparation
            .as_ref()
            .expect("explicit preparation")
            .maximum_assignment_bytes
            <= 16384
    );
    assert!(config.limits.buffered_bytes <= config.limits.heap_arena_bytes);
    assert!(config.limits.heap_arena_bytes <= 98304);
    let preparation = conduit_host_make::rust_preparation_delegation(&checked);
    let execution = conduit_host_make::rust_execution_delegation(&checked);
    let generated = format!(
        r#"
pub const CONFIGURATION_ID:&str={configuration:?};
pub const IMAGE_ID:&str={image:?};
pub const SOURCE_IDENTITY:&str={source_identity:?};
pub const HEAP_BYTES:usize={heap};
pub const STATIC_MEMORY_BYTES:usize={static_bytes};
pub const MAX_ASSIGNMENT_BYTES:usize={assignment};
pub const MAX_RUNTIME_STATE_BYTES:usize={runtime};
pub const WORK_POOL:&str={pool:?};
pub const WORK_UNITS:u32={units};
pub const WORK_STORAGE_BYTES:usize={storage};
pub const PREPARATION:Option<conduit_body::HostPreparationDelegation>={preparation};
pub const EXECUTION:Option<conduit_body::HostExecutionDelegation>={execution};
pub const BOOTSTRAP_ADDRESS:u32={bootstrap};
pub const WAIT_SLOTS:usize={wait_slots};
pub const MAXIMUM_WAIT_MS:u64={maximum_wait};
pub const DHT_START_LOW_US:u32={start};
pub const DHT_TRANSACTION_US:u32={capture};
pub const DHT_MAXIMUM_INTERRUPT_EXCLUSION_US:u32={exclusion};
pub const TM_HALF_PERIOD_US:u16={half};
pub const TM_BRIGHTNESS:u8={brightness};
pub const TM_WEAK_PULLUPS:bool={pullups};
pub const RENDERER_FACE_BYTES:u32={face_bytes};
pub const RENDERER_SHOW_BYTES:u32={show_bytes};
pub const RENDERER_RESOURCE_BYTES:u64={renderer_bytes};
pub const RENDERER_MAXIMUM_FRAMES:u32={maximum_frames};
pub const RENDERER_PAGE_PERIOD_MS:u64={page_period};
pub const RENDERER_SHOW_LIFETIME_MS:u64={show_lifetime};
pub const RENDERER_FAHRENHEIT:bool={fahrenheit};
macro_rules! own_equipment {{($p:ident)=>{{(
 conduit_esp32_electrical::Esp32PulseCapture::new($p.GPIO{data},true,crate::config::DHT_TRANSACTION_US,crate::config::DHT_MAXIMUM_INTERRUPT_EXCLUSION_US).expect("checked capture timing"),
 conduit_esp32_electrical::Esp32Tm1637::new($p.GPIO{clk},$p.GPIO{dio},crate::config::TM_HALF_PERIOD_US,crate::config::TM_WEAK_PULLUPS).expect("checked timing")
)}}}}
pub(crate) use own_equipment;
"#,
        configuration = checked.configuration_id(),
        heap = config.limits.heap_arena_bytes,
        static_bytes = config.limits.static_memory_bytes,
        assignment = config
            .preparation
            .as_ref()
            .expect("explicit preparation")
            .maximum_assignment_bytes,
        runtime = config
            .preparation
            .as_ref()
            .expect("explicit preparation policy")
            .maximum_runtime_state_bytes,
        pool = work.id,
        units = work.slots,
        storage = work.bytes,
        bootstrap = conduit_host_esp32_make::NATIVE_SPORE_REGION_START,
        wait_slots = physical.wait_slots,
        maximum_wait = physical.maximum_wait_ms,
        start = physical.start_low_us,
        capture = physical.maximum_capture_transaction_us,
        exclusion = physical.maximum_interrupt_exclusion_us,
        half = physical.half_period_us,
        brightness = physical.brightness,
        pullups = physical.weak_pullups,
        face_bytes = renderer.face_bytes,
        show_bytes = renderer.show_bytes,
        renderer_bytes = renderer.resource_bytes,
        maximum_frames = renderer.maximum_frames,
        page_period = renderer.page_period_ms,
        show_lifetime = renderer.show_lifetime_ms,
        fahrenheit = renderer.temperature_unit
            == conduit_mcu_source_build::renderer::TemperatureUnit::Fahrenheit,
        data = physical.sensor_data,
        clk = physical.display_clock,
        dio = physical.display_data
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let arena_script = format!(
        r#"
ASSERT(ORIGIN(dram_seg) == 0x3ffb0000, "unexpected primary DRAM origin");
ASSERT(LENGTH(dram_seg) == 196608, "unexpected primary DRAM size");
ASSERT(SIZEOF(.data) + SIZEOF(.bss) + SIZEOF(.noinit) <= 131072, "primary static DRAM exceeds checked physical share");
ASSERT(ORIGIN(dram2_seg) == 0x3ffe7e30, "unexpected secondary DRAM origin");
ASSERT(LENGTH(dram2_seg) == 98768, "unexpected secondary DRAM size");
ASSERT(ADDR(.dram2_uninit) == ORIGIN(dram2_seg), "heap not at secondary region origin");
ASSERT(SIZEOF(.dram2_uninit) == {heap}, "secondary DRAM has another owner or heap size differs");
ASSERT(SIZEOF(.dram2_uninit) <= LENGTH(dram2_seg), "secondary heap exceeds physical region");
ASSERT(SIZEOF(.stack) >= 16384, "primary stack reserve below 16KiB");
ASSERT(SIZEOF(.data) + SIZEOF(.bss) + SIZEOF(.noinit) + SIZEOF(.dram2_uninit) <= {static_bytes}, "static DRAM exceeds checked IMAGE");
"#,
        heap = config.limits.heap_arena_bytes,
        static_bytes = config.limits.static_memory_bytes
    );
    fs::write(out.join("image-arena.x"), arena_script).expect("checked physical arena assertions");
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg=-Timage-arena.x");

    println!("cargo:rustc-check-cfg=cfg(assigned_workset)");
    let mut workset = conduit_mcu_source_build::workset::from_environment(Some(
        conduit_mcu_source_build::workset::MaskBounds {
            face_bytes: renderer.face_bytes,
            show_bytes: renderer.show_bytes,
        },
    ))
    .expect("explicit hash-bound workset Source")
    .expect("native execution requires --workset-source and all selected entries");
    println!("cargo:rustc-cfg=assigned_workset");
    fs::write(out.join("host.rs"), &generated).unwrap();
    let resident_images = workset
        .prepare_selected_residents(conduit_mcu_source_build::dht11_tm1637_resident_bounds())
        .expect("actual combined checked Source resident definitions");
    let resident =
        conduit_embedded_build::generate_resident_library(&resident_images, 2 * 1024 * 1024)
            .expect("finite resident inventory emission");
    fs::write(out.join("resident.rs"), resident).unwrap();
    // Immutable descriptive cards are fabricated once offdevice. Root may only
    // publish each card while retaining the matching actually initialized Base.
    let artifact = conduit_core::ArtifactId::from(image.as_str());
    let pulse = conduit_pulse_capture::installation::capability_offer(
        &conduit_pulse_capture::PulseReadContract::prepare().unwrap(),
        &artifact,
        &conduit_core::ImplementationId::from(conduit_host_esp32_make::physical::DHT11),
    )
    .unwrap();
    let display = conduit_tm1637_wire::installation::capability_offer(
        &conduit_tm1637_wire::WriteContract::prepare().unwrap(),
        &artifact,
        &conduit_core::ImplementationId::from(conduit_host_esp32_make::physical::TM1637),
    )
    .unwrap();
    let wait = conduit_monotonic_wait::capability_offer(
        &artifact,
        &conduit_core::ImplementationId::from(conduit_host_esp32_make::physical::CLOCK),
    )
    .unwrap();
    let renderer_offer = conduit_mcu_host_runtime::execution_mask::capability_offer(
        conduit_host_esp32_make::physical::RENDERER.into(),
        artifact.clone(),
        conduit_mcu_host_runtime::execution_mask::Bounds {
            face_bytes: renderer.face_bytes,
            show_bytes: renderer.show_bytes,
            resource_bytes: renderer.resource_bytes,
            maximum_frames: renderer.maximum_frames,
        },
    );
    let host_image = conduit_mcu_source_build::image_input::from_environment(
        &checked,
        &catalog,
        &image,
        &source_identity,
    )
    .expect("exact original captured Make IMAGE");
    use conduit_mcu_source_build::native_calibration::{NativeDescription, Slot};
    let descriptions = [
        NativeDescription {
            slot: Slot {
                base: "esp32/gpio-pulse",
                pool: "pool/esp32/gpio-pulse",
                implementation: conduit_host_esp32_make::physical::DHT11,
            },
            class: conduit_pulse_capture::installation::PULSE_ATTACHMENT_CLASS,
            definition: pulse.clone(),
        },
        NativeDescription {
            slot: Slot {
                base: "esp32/tm1637",
                pool: "pool/esp32/tm1637",
                implementation: conduit_host_esp32_make::physical::TM1637,
            },
            class: conduit_tm1637_wire::installation::WRITE_ATTACHMENT_CLASS,
            definition: display.clone(),
        },
        NativeDescription {
            slot: Slot {
                base: "esp32/face-renderer",
                pool: "pool/esp32/face-renderer",
                implementation: conduit_host_esp32_make::physical::RENDERER,
            },
            class: conduit_mcu_host_runtime::execution_mask::ATTACHMENT,
            definition: renderer_offer.clone(),
        },
        NativeDescription {
            slot: Slot {
                base: "esp32/wait/1",
                pool: "pool/esp32/wait/1",
                implementation: conduit_host_esp32_make::physical::CLOCK,
            },
            class: conduit_monotonic_wait::WAIT_ATTACHMENT_CLASS,
            definition: wait.clone(),
        },
    ];
    let routing = conduit_mcu_host_runtime::execution_mask::routing_offers(
        renderer.face_bytes,
        renderer.show_bytes,
    )
    .expect("same actual Mask routing definitions");
    let shared_resources = [conduit_core::resource_offer(
        &work.id,
        &work.class,
        work.slots,
    )];
    let calibration = conduit_mcu_source_build::native_calibration::compile(
        &workset,
        conduit_mcu_source_build::native_calibration::Input {
            checked: &checked,
            image: &host_image,
            catalog: &catalog,
            shared_resources: &shared_resources,
            native: &descriptions,
            other: &routing,
            resident: &resident_images,
            bounds: conduit_mcu_source_build::native_calibration::Bounds::for_workset(
                &checked,
                &workset,
                conduit_embedded_build::EmbeddedComputationBounds {
                    maximum_program_bytes: conduit_mcu_source_build::dht11_tm1637_resident_bounds()
                        .maximum_program_bytes,
                    maximum_total_program_bytes:
                        conduit_mcu_source_build::dht11_tm1637_resident_bounds()
                            .maximum_program_bytes,
                },
            )
            .expect("explicit finite Host calibration bounds"),
        },
    )
    .expect("ordinary Planner and exact structural lowering must qualify complete native workset");
    calibration
        .apply_software_mapping(
            &mut workset,
            Some(conduit_mcu_source_build::workset::MaskBounds {
                face_bytes: renderer.face_bytes,
                show_bytes: renderer.show_bytes,
            }),
            24 * 1024,
            2 * 1024 * 1024,
        )
        .expect("exact Source to kernel ordinal software mapping");
    calibration.emit_continuation_identity(&out, workset.sha256).expect("checked continuation identity emission");
    calibration
        .emit_to(&out, 2 * 1024 * 1024)
        .expect("finite immutable Source layout emission");
    conduit_mcu_source_build::source_composition::emit_to(&out)
        .expect("shared Source composition emission");
    workset
        .emit_to(&out)
        .expect("calibrated Source factories only");
    let mut native = String::new();
    for (name, offer) in [
        ("PULSE_IMAGE", pulse),
        ("DISPLAY_IMAGE", display),
        ("WAIT_IMAGE", wait),
        ("RENDERER_IMAGE", renderer_offer),
    ] {
        let bytes = conduit_embedded_build::generate_capability_offer_image(&offer, 24 * 1024)
            .expect("finite checked native definition image");
        native.push_str(&format!("pub static {name}:&[u8]=&{bytes:?};\n"));
    }
    fs::write(out.join("native_images.rs"), native).unwrap();
    let mut hash = Sha256::new();
    hash.update(source_identity);
    hash.update(source);
    hash.update(generated);
    println!(
        "cargo:rustc-env=CONDUIT_ESP32_BUILD_ID={:x}",
        hash.finalize()
    );
}
fn conduit_core_work_class() -> &'static str {
    "kernel/cooperative-step-work@1"
}
