use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-env-changed=CONDUIT_HOST_CONFIGURATION");
    println!("cargo:rerun-if-env-changed=CONDUIT_SOURCE_IDENTITY");
    println!("cargo:rerun-if-env-changed=CONDUIT_HOST_IMAGE_ID");
    let image_id =
        env::var("CONDUIT_HOST_IMAGE_ID").expect("use ordinary Make with its checked IMAGE");
    let source_identity =
        env::var("CONDUIT_SOURCE_IDENTITY").expect("use cargo xtask make host build");
    let profile = env::var_os("CONDUIT_HOST_CONFIGURATION")
        .map(PathBuf::from)
        .unwrap_or_else(|| "../../profiles/pico-w-house-green.host.conduit".into());
    let source = fs::read_to_string(&profile).expect("explicit Host Source");
    let packages =
        conduit_host_make::MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage])
            .unwrap();
    let catalog = conduit_host_make::MakeCatalog::canonical().with_packages(&packages);
    let configuration =
        conduit_host_make::parse_host_configuration_conduit(&source).expect("Host Source syntax");
    let checked = conduit_host_make::check_host_configuration(configuration, &catalog, &packages)
        .expect("checked Host Source");
    conduit_host_rp2040::check_retained_wifi_construction(&checked)
        .expect("checked retained network attachment");
    println!("cargo:rustc-check-cfg=cfg(display_tm1637)");
    let tm = checked
        .resolved_bases()
        .iter()
        .any(|(_, implementation)| implementation == conduit_host_rp2040::TM1637_IMPLEMENTATION);
    println!("cargo:rustc-check-cfg=cfg(sensor_dht11)");
    println!("cargo:rustc-check-cfg=cfg(shared_source_execution)");
    let dht = checked
        .resolved_bases()
        .iter()
        .any(|(_, implementation)| implementation == conduit_host_rp2040::DHT11_IMPLEMENTATION);
    let dht = dht.then(|| {
        conduit_host_rp2040::CheckedDht11Tm1637Construction::from_configuration(&checked)
            .expect("checked DHT11/TM1637 construction")
    });
    if dht.is_some() {
        println!("cargo:rustc-cfg=sensor_dht11");
        println!("cargo:rustc-cfg=display_tm1637");
    }
    let display_gpio = if tm && dht.is_none() {
        println!("cargo:rustc-cfg=display_tm1637");
        Some(
            conduit_host_rp2040::CheckedBoschTm1637Construction::from_configuration(&checked)
                .expect("checked Bosch/TM1637 physical bindings"),
        )
    } else {
        None
    };
    let (bus, sensor_address, display_address, brightness) = if let Some(dht) = &dht {
        // Shared type plumbing only. No I2C peripheral is consumed, initialized,
        // advertised or granted for a GPIO-only construction.
        (
            conduit_host_rp2040::CheckedI2cConstruction {
                configuration_id: checked.configuration_id().into(),
                controller: 1,
                sda_gpio: 6,
                scl_gpio: 7,
                frequency_hz: 100000,
                maximum_transaction_polls: 1,
                call_address_range: None,
                maximum_call_operations: 0,
            },
            0,
            0,
            dht.display.brightness,
        )
    } else if let Some(tm) = &display_gpio {
        (tm.bus.clone(), tm.sensor_address, 0, tm.display.brightness)
    } else {
        let h = conduit_host_rp2040::CheckedBoschHt16k33Construction::from_configuration(&checked)
            .expect("checked actual peripheral bindings");
        (h.bus, h.sensor_address, h.display_address, h.brightness)
    };
    let work = conduit_host_rp2040::CheckedResidentWorkPool::from_configuration(&checked)
        .expect("explicit resident work-pool construction budget");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let renderer = dht.as_ref().map(|_| {
        conduit_host_rp2040::CheckedRendererConstruction::from_configuration(
            &checked,
            conduit_host_rp2040::RENDERER_IMPLEMENTATION,
        )
        .expect("checked renderer construction")
    });
    let resident_images = renderer.map(|renderer| {
        prepare_dht_workset(
            &checked,
            &catalog,
            &image_id,
            &source_identity,
            &out,
            renderer,
        )
    });

    let resident = if let Some(images) = &resident_images {
        conduit_embedded_build::generate_resident_library(images, 2 * 1024 * 1024)
            .map_err(|e| format!("resident library: {e:?}"))
    } else if tm {
        // No HT16K33 computations or TM1637 Back are installed for this profile.
        conduit_embedded_build::generate_resident_library(&[], 2 * 1024 * 1024)
            .map_err(|e| format!("empty resident inventory unavailable: {e:?}"))
    } else {
        conduit_host_rp2040::checked_ht16k33_resident_library(&checked)
    }
    .expect("checked resident protocol closure");
    fs::write(out.join("resident.rs"), resident).unwrap();
    let native = if dht.is_some() {
        None
    } else {
        conduit_host_rp2040::checked_i2c_offer_image(&checked, &image_id, 24 * 1024)
            .expect("checked native definition, not runtime readiness")
    };
    let native_source = match native {
        Some(bytes) => format!("pub static I2C_OFFER_IMAGE:Option<&[u8]>=Some(&{bytes:?});"),
        None => "pub static I2C_OFFER_IMAGE:Option<&[u8]>=None;".into(),
    };
    fs::write(out.join("native_offer.rs"), native_source).unwrap();
    let c = bus.controller;
    let sda = bus.sda_gpio;
    let scl = bus.scl_gpio;
    let renderer_source=match renderer {
        Some(r)=>format!("pub const RENDERER_FACE_BYTES:u32={};pub const RENDERER_SHOW_BYTES:u32={};pub const RENDERER_RESOURCE_BYTES:u64={};pub const RENDERER_MAXIMUM_FRAMES:u32={};pub const RENDERER_PAGE_PERIOD_MS:u64={};pub const RENDERER_SHOW_LIFETIME_MS:u64={};pub const RENDERER_FAHRENHEIT:bool={};",r.face_bytes,r.show_bytes,r.resource_bytes,r.maximum_frames,r.page_period_ms,r.show_lifetime_ms,matches!(r.temperature_unit,conduit_host_rp2040::RendererTemperatureUnit::Fahrenheit)),
        None=>String::new(),
    };
    fs::write(out.join("renderer.rs"), renderer_source).unwrap();
    let recording = conduit_host_rp2040::source_build::native_build::checked_recording(
        &checked,
        conduit_host_rp2040::RECORDING_IMPLEMENTATION,
        8192,
        589824,
    )
    .expect("checked finite recording construction");
    let recording_prefix = recording.map_or(0, |r| r.prefix_bytes);
    let recording_slots = recording.map_or(0, |r| r.slots);
    let preparation = conduit_host_make::rust_preparation_delegation(&checked);
    let execution = conduit_host_make::rust_execution_delegation(&checked);
    let heap = checked.configuration().limits.heap_arena_bytes;
    assert!((32768..=98304).contains(&heap), "finite preparation arena");
    let local_io = if let Some(dht) = &dht {
        let d = &dht.display;
        format!(
            r#"
pub const DHT_START_LOW_US:u32={start};
pub const DHT_TRANSACTION_US:u32={deadline};
pub type SensorData=embassy_rp::peripherals::PIN_{data};
pub const TM_HALF_PERIOD_US:u32={half};
pub const TM_SETTLED_US:u32={settled};
pub const TM_CLOCK_RISE_US:u32={rise};
pub const TM_TRANSACTION_US:u32={transaction};
pub const TM_WEAK_PULLUPS:bool={pullups};
pub type DisplayClock=embassy_rp::peripherals::PIN_{clk};
pub type DisplayData=embassy_rp::peripherals::PIN_{dio};
macro_rules! start_local_io {{($spawner:expr,$p:ident)=>{{ crate::environment::initialize($p.PIO1,$p.DMA_CH2,$p.PIN_{data},$p.PIN_{clk},$p.PIN_{dio}); }}}}
pub(crate) use start_local_io;
"#,
            start = dht.sensor.start_low_us,
            deadline = dht.sensor.maximum_transaction_us,
            data = dht.sensor.data_gpio,
            half = d.half_period_us,
            settled = d.settled_us,
            rise = d.maximum_clock_rise_us,
            transaction = d.maximum_transaction_us,
            pullups = d.weak_pullups,
            clk = d.clock_gpio,
            dio = d.data_gpio
        )
    } else if let Some(tm) = &display_gpio {
        let d = &tm.display;
        format!(
            r#"
pub const TM_HALF_PERIOD_US:u32={half};
pub const TM_SETTLED_US:u32={settled};
pub const TM_CLOCK_RISE_US:u32={rise};
pub const TM_TRANSACTION_US:u32={transaction};
pub const TM_WEAK_PULLUPS:bool={pullups};
pub type DisplayClock=embassy_rp::peripherals::PIN_{clk};
pub type DisplayData=embassy_rp::peripherals::PIN_{dio};
macro_rules! start_local_io {{($spawner:expr,$p:ident)=>{{{{$spawner.spawn(crate::environment::run($p.I2C{c},$p.PIN_{scl},$p.PIN_{sda},$p.PIN_{clk},$p.PIN_{dio}).unwrap());}}}}}}
pub(crate) use start_local_io;
"#,
            half = d.half_period_us,
            settled = d.settled_us,
            rise = d.maximum_clock_rise_us,
            transaction = d.maximum_transaction_us,
            pullups = d.weak_pullups,
            clk = d.clock_gpio,
            dio = d.data_gpio
        )
    } else {
        format!(
            r#"
macro_rules! start_local_io {{($spawner:expr,$p:ident)=>{{{{$spawner.spawn(crate::environment::run($p.I2C{c},$p.PIN_{scl},$p.PIN_{sda}).unwrap());}}}}}}
pub(crate) use start_local_io;
"#
        )
    };
    let generated = format!(
        r#"
include!(concat!(env!("OUT_DIR"), "/native_offer.rs"));
pub const RECORDING_PREFIX_BYTES:u32={recording_prefix};
pub const RECORDING_SLOTS:u32={recording_slots};
pub const CONFIGURATION_ID:&str={configuration:?};
pub const IMAGE_ID:&str={image_id:?};
pub const HEAP_BYTES:usize={heap};
pub const TIMER_SLOTS:usize={timer_slots};
pub const PREPARATION_DELEGATION:Option<conduit_body::HostPreparationDelegation>={preparation};
pub const EXECUTION_DELEGATION:Option<conduit_body::HostExecutionDelegation>={execution};
pub const RESIDENT_WORK_POOL:&str={work_pool:?};
pub const RESIDENT_WORK_UNITS:u32={work_units};
// Checked private computation bookkeeping only, not whole-Root heap permission.
pub const RESIDENT_WORK_BOOKKEEPING_BYTES:u64={storage_bytes};
pub const RESIDENT_PREPARATION_STORAGE_BYTES:usize={storage_bytes};
// Representation ceilings from checked construction, not new execution grants.
pub const RESIDENT_PREPARATION_MAXIMA:conduit_core::AssignedPlanMaxima=conduit_core::AssignedPlanMaxima{{
 encoded_bytes:{encoded},runtime_state_bytes:{runtime},
 counts:[{nodes},{ports},{nodes},{queues},{nodes},{queues},0,{nodes},{signs},{ports},{nodes},{nodes},0],
}};
// The installed mixed profile needs one Host Call, bounded by Source's
// operation slots and explicit native operation policy. All other ceilings stay.
pub const MIXED_PREPARATION_MAXIMA:conduit_core::AssignedPlanMaxima={{
 let mut limits=RESIDENT_PREPARATION_MAXIMA;
 limits.counts[6]={mixed_calls};
 limits
}};
pub const ROLE:&str="conduit.rp2040-host@1";
pub const I2C_CONTROLLER:u8={c};
pub const I2C_CALL_ADDRESS_RANGE:Option<(u8,u8)>={call_address_range:?};
pub const I2C_CALL_MAXIMUM_OPERATIONS:u32={call_maximum_operations};
pub const FREQUENCY:u32={frequency};
pub const MAXIMUM_I2C_POLLS:core::num::NonZeroU32=core::num::NonZeroU32::new({polls}).expect("checked nonzero I2C work bound");
pub const SENSOR_ADDRESS:u8={sensor};
pub const DISPLAY_ADDRESS:u8={display};
pub const BRIGHTNESS:u8={brightness};
pub const DIAGNOSTIC_HTTP_BUFFER_LIMIT:u64={buffers};
pub fn display_profile()->conduit_ht16k33::Profile{{conduit_ht16k33::Profile::adafruit_alpha4()}}
pub type Bus=embassy_rp::peripherals::I2C{c};
pub type Scl=embassy_rp::peripherals::PIN_{scl};
pub type Sda=embassy_rp::peripherals::PIN_{sda};
embassy_rp::bind_interrupts!(pub struct SensorIrqs {{ I2C{c}_IRQ => embassy_rp::i2c::InterruptHandler<Bus>; }});
{local_io}
"#,
        mixed_calls = if bus.maximum_call_operations == 0 {
            0
        } else {
            checked.configuration().limits.operation_slots.min(1)
        },
        call_address_range = bus.call_address_range,
        call_maximum_operations = bus.maximum_call_operations,
        image_id = image_id,
        timer_slots = checked.configuration().limits.timer_slots,
        configuration = checked.configuration_id(),
        frequency = bus.frequency_hz,
        polls = bus.maximum_transaction_polls,
        sensor = sensor_address,
        display = display_address,
        brightness = brightness,
        buffers = checked.configuration().limits.buffered_bytes,
        work_pool = work.id,
        work_units = work.capacity_units,
        storage_bytes = work.bookkeeping_bytes,
        encoded = checked
            .configuration()
            .limits
            .buffered_bytes
            .min(u64::from(u16::MAX)),
        runtime = work.bookkeeping_bytes.min(u64::from(u16::MAX)),
        nodes = checked.configuration().limits.active_instances.min(32),
        ports = (checked
            .configuration()
            .limits
            .active_instances
            .saturating_mul(2))
        .min(16),
        queues = checked.configuration().limits.queue_items.min(255),
        signs = checked.configuration().limits.evidence_items.min(255)
    );
    fs::write(out.join("host.rs"), generated).unwrap();
    fs::write(
        out.join("memory.x"),
        conduit_rp2040_flash_layout::LINKER_MEMORY_X,
    )
    .unwrap();
    fs::write(out.join("bounds.x"), format!("ASSERT(__euninit - __sdata <= {}, \"Host static bound exceeded\");\nASSERT(_stack_start - __euninit >= 16384, \"Host stack below16KiB\");\n",checked.configuration().limits.static_memory_bytes)).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rustc-link-arg=-T{}", out.join("bounds.x").display());
    println!("cargo:rustc-link-arg=--nmagic");
    let mut digest = Sha256::new();
    digest.update(source_identity.as_bytes());
    digest.update(source.as_bytes());
    digest.update(image_id.as_bytes());
    let mut entries = fs::read_dir("src")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.extension().is_some_and(|x| x == "rs") {
            println!("cargo:rerun-if-changed={}", path.display());
            digest.update(fs::read(path).unwrap());
        }
    }
    println!("cargo:rerun-if-changed={}", profile.display());
    println!(
        "cargo:rustc-env=CONDUIT_RP2040_BUILD_ID={:x}",
        digest.finalize()
    );
}

#[allow(clippy::too_many_arguments)]
fn prepare_dht_workset(
    checked: &conduit_host_make::CheckedHostConfiguration,
    catalog: &conduit_host_make::MakeCatalog,
    image_id: &str,
    source_identity: &str,
    out: &std::path::Path,
    renderer: conduit_host_rp2040::CheckedRendererConstruction,
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
    let append = workset
        .entries
        .iter()
        .any(|entry| {
            entry.plot.expanded.gears.iter().any(|g| {
                g.kind_id.as_str() == conduit_resource_snapshot::native_append::APPEND_KIND
            })
        })
        .then(|| {
            conduit_resource_snapshot::native_append::capability_offer(
                &artifact,
                &conduit_core::ImplementationId::from("rp2040/nor-record-append@1"),
            )
            .expect("finite immutable append definition")
        });
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
    if let Some(definition) = &append {
        descriptions.push(calibration::NativeDescription {
            slot: calibration::Slot {
                base: "rp2040/record-append",
                pool: "pool/rp2040/record-append",
                implementation: "rp2040/nor-record-append@1",
            },
            class: conduit_resource_snapshot::native_append::APPEND_CLASS,
            mechanism: conduit_resource_snapshot::native_append::APPEND_MECHANISM,
            definition: definition.clone(),
        });
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
