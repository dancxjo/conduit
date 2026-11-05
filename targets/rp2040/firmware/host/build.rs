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
    let selected =
        conduit_host_rp2040::CheckedBoschHt16k33Construction::from_configuration(&checked)
            .expect("checked actual peripheral bindings");
    let work = conduit_host_rp2040::CheckedResidentWorkPool::from_configuration(&checked)
        .expect("explicit resident work-pool construction budget");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let resident = conduit_host_rp2040::checked_ht16k33_resident_library(&checked)
        .expect("checked resident protocol closure");
    fs::write(out.join("resident.rs"), resident).unwrap();
    let c = selected.bus.controller;
    let sda = selected.bus.sda_gpio;
    let scl = selected.bus.scl_gpio;
    let preparation = conduit_host_make::rust_preparation_delegation(&checked);
    let heap = checked.configuration().limits.heap_arena_bytes;
    assert!((32768..=98304).contains(&heap), "finite preparation arena");
    let generated = format!(
        r#"
pub const CONFIGURATION_ID:&str={configuration:?};
pub const IMAGE_ID:&str={image_id:?};
pub const HEAP_BYTES:usize={heap};
pub const PREPARATION_DELEGATION:Option<conduit_body::HostPreparationDelegation>={preparation};
pub const RESIDENT_WORK_POOL:&str={work_pool:?};
pub const RESIDENT_WORK_UNITS:u32={work_units};
// Representation ceilings from checked construction, not new execution grants.
pub const RESIDENT_PREPARATION_MAXIMA:conduit_core::AssignedPlanMaxima=conduit_core::AssignedPlanMaxima{{
 encoded_bytes:{encoded},runtime_state_bytes:{runtime},
 counts:[{nodes},{ports},{nodes},{queues},{nodes},{queues},0,{nodes},{signs},{ports},{nodes},{nodes},0],
}};
pub const ROLE:&str="conduit.rp2040-host@1";
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
macro_rules! start_local_io {{($spawner:expr,$p:ident)=>{{$spawner.spawn(crate::environment::run($p.I2C{c},$p.PIN_{scl},$p.PIN_{sda}).unwrap());}}}}
pub(crate) use start_local_io;
"#,
        image_id = image_id,
        configuration = checked.configuration_id(),
        frequency = selected.bus.frequency_hz,
        polls = selected.bus.maximum_transaction_polls,
        sensor = selected.sensor_address,
        display = selected.display_address,
        brightness = selected.brightness,
        buffers = checked.configuration().limits.buffered_bytes,
        work_pool = work.id,
        work_units = work.capacity_units,
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
