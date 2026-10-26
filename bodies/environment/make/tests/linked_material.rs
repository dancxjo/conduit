//! Compile and execute the emitted Rust, then compare complete records and RAM.
//! This proves Make representation equality, not physical startup or possession.
use std::{fs, path::Path, process::Command};

mod plan {
    pub const PREFIX: &str = "fixture/linked-material";
}
#[allow(dead_code, reason = "only native advertisement construction is used")]
#[path = "../../runtime/tests/appliance_support/mod.rs"]
mod appliance_support;
#[allow(dead_code, reason = "only inert native advertisement custody is used")]
#[path = "display_support/fixture.rs"]
mod display_fixture;
#[allow(dead_code, reason = "only inert advertisement construction is used")]
#[path = "../../runtime/tests/support/pulse/provider.rs"]
mod pulse;

#[test]
fn emitted_linked_material_preserves_records_without_copying_program_text() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    let source = conduit_environment_runtime::construction::compile_default().unwrap();
    let recipe = conduit_environment_make::NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    let directory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("target/linked-material-conformance");
    fs::create_dir_all(directory.join("src")).unwrap();
    let path = |suffix: &str| root.join(suffix).to_string_lossy().replace('\\', "\\\\");
    fs::write(
        directory.join("Cargo.toml"),
        format!(
            r#"
[package]
name = "conduit-linked-material-conformance"
version = "0.0.0"
edition = "2021"
[workspace]
[features]
target-memory-check = []
[dependencies]
conduit-core = {{ path = {:?} }}
conduit-planner = {{ path = {:?}, features = ["indexed-source"] }}
conduit-plot = {{ path = {:?}, features = ["flat-image"] }}
conduit-kernel = {{ path = {:?}, features = ["alloc"] }}
conduit-time = {{ path = {:?}, default-features = false }}
conduit-environment-runtime = {{ path = {:?}, features = ["local-planning", "display-output"] }}
serde_json = "1"
conduit-tm1637-wire = {{ path = {:?} }}
[profile.dev]
debug = 0
incremental = false
[profile.dev.build-override]
opt-level = 1
"#,
            path("architecture/core"),
            path("architecture/planner"),
            path("architecture/plot"),
            path("architecture/kernel"),
            path("semantics/time"),
            path("bodies/environment/runtime"),
            path("mechanisms/io/tm1637")
        ),
    )
    .unwrap();
    let (provider, effects) = pulse::ready(0);
    fs::write(
        directory.join("native-host.json"),
        serde_json::to_vec(&appliance_support::native_advertisement(&provider)).unwrap(),
    )
    .unwrap();
    fs::write(
        directory.join("grants.json"),
        serde_json::to_vec(&[pulse::grant()]).unwrap(),
    )
    .unwrap();
    assert_eq!(effects.get(), 0);
    let mut generated = String::from("extern crate alloc;\n");
    recipe.write_source_rust(&mut generated).unwrap();
    assert!(
        !generated.contains("serde_json::from"),
        "native sampler callbacks must contain no JSON decoder"
    );
    let display_source = conduit_environment_make::DisplayOutputSource::compile().unwrap();
    let display_material =
        conduit_environment_make::DisplaySourceMaterial::prepare(&display_source).unwrap();
    let display_fixture = display_fixture::fixture(&display_source);
    fs::write(
        directory.join("display-host.json"),
        serde_json::to_vec(&display_fixture.host).unwrap(),
    )
    .unwrap();
    fs::write(
        directory.join("display-grants.json"),
        serde_json::to_vec(&[display_fixture.grant]).unwrap(),
    )
    .unwrap();
    generated.push_str("\n#[allow(dead_code)] mod display {\n");
    display_material.write_rust(&mut generated).unwrap();
    generated.push_str("\n}\n");

    generated.push_str(&format!(
        "\n#[path = {:?}] mod allocation_probe;\n",
        path("architecture/plot/tests/common/allocation_probe.rs")
    ));
    generated.push_str(r#"
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn plan<'a>(image: &'a conduit_plot::flat_image::indexed::IndexedSourceImage<'a>, host: &c::HostAdvertisement, grants: &[c::AuthorityGrant], consuming: bool) -> Result<conduit_planner::indexed_source::CompactIndexedPlan<'a>, conduit_planner::PlannerError> {
    use std::collections::BTreeMap;
    // Count the same incoming advertisement ownership in both measurements.
    let owned_host = host.clone();
    let hosts = std::slice::from_ref(&owned_host);
    let placements = conduit_planner::indexed_source::default_placements(image, hosts)?;
    let metadata = image.metadata().unwrap();
    let queues: BTreeMap<_, _> = metadata.connections().iter().map(|cord| {
        let sink = (0..image.gear_count()).map(|i| image.gear(i).unwrap()).find(|gear| gear.gear_id == cord.sink_gear_id).unwrap();
        let channel = conduit_environment_runtime::publication::PublicationChannel::Reading;
        let capacity = if sink.kind_id.as_str() == channel.kind_id() { channel.maximum_bytes() as u32 } else { 1024 };
        ((cord.source_gear_id.clone(), cord.source_port_id.clone(), cord.sink_gear_id.clone(), cord.sink_port_id.clone()), conduit_planner::ConnectionQueueLimits { item_capacity: 1, byte_capacity: capacity })
    }).filter(|(_, limits)| limits.item_capacity != 1 || limits.byte_capacity != 1024).collect();
    drop(metadata);
    let empty = BTreeMap::new(); let lines = BTreeMap::new();
    let options = conduit_planner::PlanningOptions {
        connection_bases: &empty, line_candidates: &lines, connection_item_capacity: 1, connection_byte_capacity: 1024,
        authority_grants: grants, protected_resource_grants: &[], line_offers: &[],
    };
    if consuming {
        conduit_planner::indexed_source::plan_compact_consuming_with_connection_limits(image, vec![owned_host], &placements, &["conduit.base/local@1".into()], options, &queues)
    } else {
        conduit_planner::indexed_source::plan_compact_with_connection_limits(image, hosts, &placements, &["conduit.base/local@1".into()], options, &queues)
    }
}
fn display_plan<'a>(image: &'a conduit_plot::flat_image::indexed::IndexedSourceImage<'a>, host: &c::HostAdvertisement, grants: &[c::AuthorityGrant]) -> Result<conduit_planner::indexed_source::CompactIndexedPlan<'a>, conduit_planner::PlannerError> {
    use std::collections::BTreeMap;
    let boundary = display::planning_boundary().unwrap();
    let choices = conduit_planner::indexed_source::default_placements(image, std::slice::from_ref(host))?;
    let fore = boundary.bindings().map(|(direction, port, track)| (conduit_planner::ForeBoundaryKey { direction, front_port_id: port.clone(), track }, conduit_planner::ConnectionQueueLimits { item_capacity: 1, byte_capacity: if direction == c::PortDirection::Input { conduit_tm1637_wire::MAXIMUM_BYTES } else { 64 } })).collect();
    let empty = BTreeMap::new(); let lines = BTreeMap::new(); let queues = BTreeMap::new();
    conduit_planner::indexed_source::plan_compact_authoring_with_connection_limits(image, &boundary, std::slice::from_ref(host), &choices, &["conduit.base/local@1".into(), "fixture/base".into()], conduit_planner::PlanningOptions {
        connection_bases: &empty, line_candidates: &lines, connection_item_capacity: 1, connection_byte_capacity: conduit_tm1637_wire::MAXIMUM_BYTES, authority_grants: grants, protected_resource_grants: &[], line_offers: &[],
    }, &queues, &fore)
}
fn main() {
    // Synthetic construction input for compile-only target checks; never deploy
    // this fixture as continuation of an existing physical Body.
    let sign = c::bind_sign(&"fixture/linked-material-host".into(), &"fixture/linked-material-boot".into(), None, 1);
    std::fs::write(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture-birth-sign.json"), serde_json::to_vec(&sign).unwrap()).unwrap();
    // SAFETY: this exact image and decoder were emitted together by checked Make.
    let ordinary = unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(planning_source_image()) }.unwrap();
    let linked = unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(planning_source_image()).unwrap().with_make_gear_decoder(decode_planning_gear).with_make_configuration_decoder(decode_planning_configuration) };
    let (full_metadata, full_metadata_memory) = allocation_probe::observe(|| linked.metadata().unwrap());
    let (planning_metadata, planning_metadata_memory) = allocation_probe::observe(|| linked.planning_metadata().unwrap());
    assert_eq!(full_metadata.identity(), planning_metadata.identity());
    assert_eq!(full_metadata.name(), planning_metadata.name());
    assert_eq!(full_metadata.completion(), planning_metadata.completion());
    assert_eq!(full_metadata.connections(), planning_metadata.connections());
    for origin in full_metadata.provenance() {
        assert_eq!(Some(origin.source_span), planning_metadata.source_span(&origin.gear_id));
    }
    assert!(planning_metadata_memory.retained_bytes() < full_metadata_memory.retained_bytes());
    println!("full_metadata_retained={} planning_metadata_retained={}", full_metadata_memory.retained_bytes(), planning_metadata_memory.retained_bytes());
    drop((full_metadata, planning_metadata));
    let mut largest_rich_peak = 0;
    let mut largest_linked_peak = 0;
    for index in 0..ordinary.gear_count() {
        let (rich, rich_memory) = allocation_probe::observe(|| ordinary.gear(index).unwrap());
        let (compact, compact_memory) = allocation_probe::observe(|| linked.gear(index).unwrap());
        assert_eq!(rich, compact);
        let projected = ordinary.gear_configuration(index).unwrap();
        let (linked_configuration, config_memory) = allocation_probe::observe(|| linked.gear_configuration(index).unwrap());
        assert_eq!(projected.gear_id, rich.gear_id);
        assert_eq!(projected.configuration, rich.configuration);
        assert_eq!(linked_configuration.gear_id, rich.gear_id);
        assert_eq!(linked_configuration.configuration, rich.configuration);
        assert!(config_memory.peak_bytes <= 4096, "configuration-only decode: {config_memory:?}");
        assert_eq!(serde_json::to_vec(&rich).unwrap(), serde_json::to_vec(&compact).unwrap());
        largest_rich_peak = largest_rich_peak.max(rich_memory.peak_bytes);
        largest_linked_peak = largest_linked_peak.max(compact_memory.peak_bytes);
    }
    assert!(linked.gear(linked.gear_count()).is_err());
    assert!(largest_linked_peak * 2 < largest_rich_peak);
    let software = conduit_environment_runtime::appliance::assigned::PreparedResidentSoftware::prepare_source(&spec(), computations(), conduit_environment_runtime::appliance::prepare_value_storage().unwrap()).unwrap();
    let ready = software.source_offers(&spec(), offers()).unwrap();
    let (rich, rich_memory) = allocation_probe::observe(|| ready.iter().map(|json| serde_json::from_slice::<c::CapabilityOffer>(json).unwrap()).collect::<Vec<_>>());
    let (compact, compact_memory) = allocation_probe::observe(|| ready.decoded().map(Result::unwrap).collect::<Vec<_>>());
    assert_eq!(rich, compact);
    assert_eq!(serde_json::to_vec(&rich).unwrap(), serde_json::to_vec(&compact).unwrap());
    assert!(rich_memory.retained_bytes() > compact_memory.retained_bytes() + 30_000);
    assert!(compact_memory.peak_bytes < rich_memory.peak_bytes);
    assert!(compact_memory.retained_bytes() <= 24 * 1024);
    assert!(compact_memory.peak_bytes <= 32 * 1024);
    assert!(largest_linked_peak <= 8 * 1024);
    let native: c::HostAdvertisement = serde_json::from_slice(include_bytes!("../native-host.json")).unwrap();
    let grants: Vec<c::AuthorityGrant> = serde_json::from_slice(include_bytes!("../grants.json")).unwrap();
    let mut rich_host = native.clone(); rich_host.capabilities.extend(rich);
    let mut linked_host = native; linked_host.capabilities.extend(compact);
    let (rich_plan, rich_plan_memory) = allocation_probe::observe(|| plan(&ordinary, &rich_host, &grants, false).unwrap());
    let (linked_plan, linked_plan_memory) = allocation_probe::observe(|| plan(&linked, &linked_host, &grants, false).unwrap());
    assert_eq!(rich_plan.materialize().unwrap(), linked_plan.materialize().unwrap());
    let (consuming_plan, consuming_memory) = allocation_probe::observe(|| plan(&linked, &linked_host, &grants, true).unwrap());
    assert_eq!(linked_plan.materialize().unwrap(), consuming_plan.materialize().unwrap());
    assert!(consuming_memory.peak_bytes < linked_plan_memory.peak_bytes);
    assert_eq!(plan(&linked, &linked_host, &[], true).err().unwrap(), plan(&linked, &linked_host, &[], false).err().unwrap());
    println!("consuming_planning_peak_including_advertisement_clone={}", consuming_memory.peak_bytes);

    assert!(linked_plan_memory.peak_bytes < rich_plan_memory.peak_bytes);
    assert_eq!(plan(&ordinary, &rich_host, &[], false).err().unwrap(), plan(&linked, &linked_host, &[], false).err().unwrap());
    let mut stale = grants; stale[0].boot_id = "stale/boot".into();
    assert_eq!(plan(&ordinary, &rich_host, &stale, false).err().unwrap(), plan(&linked, &linked_host, &stale, false).err().unwrap());
    assert_eq!(plan(&linked, &linked_host, &stale, true).err().unwrap(), plan(&linked, &linked_host, &stale, false).err().unwrap());

    // Decode only while the independently initialized display owner is retained.
    let mut display_host: c::HostAdvertisement = serde_json::from_slice(include_bytes!("../display-host.json")).unwrap();
    display_host.capabilities.retain(|offer| offer.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND);
    let mut output = display::prepare_compact(display_host.host_id.clone(), display_host.boot_id.clone()).unwrap();
    let ready = output.current_offers(&display_host.host_id, &display_host.boot_id, display_host.offer_generation).unwrap();
    let (raw, raw_memory) = allocation_probe::observe(|| ready.iter().map(|wire| serde_json::from_slice::<c::CapabilityOffer>(wire).unwrap()).collect::<Vec<_>>());
    let (linked_offers, linked_memory) = allocation_probe::observe(|| ready.decoded().map(Result::unwrap).collect::<Vec<_>>());
    assert_eq!(raw, linked_offers);
    assert!(linked_memory.retained_bytes() + 10_000 < raw_memory.retained_bytes());
    assert!(display::decode_current_offer(b"{}").is_err());
    // SAFETY: the same checked Make producer emitted both complete encodings.
    let raw_display = unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(display::planning_source_image()) }.unwrap();
    let linked_display = unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(display::planning_source_image()).unwrap().with_make_gear_decoder(display::decode_planning_gear).with_make_configuration_decoder(display::decode_planning_configuration) };
    for index in 0..raw_display.gear_count() {
        let full = raw_display.gear(index).unwrap();
        assert_eq!(full, linked_display.gear(index).unwrap());
        let raw_configuration = raw_display.gear_configuration(index).unwrap();
        let (configuration, memory) = allocation_probe::observe(|| linked_display.gear_configuration(index).unwrap());
        assert_eq!(raw_configuration.gear_id, full.gear_id);
        assert_eq!(raw_configuration.configuration, full.configuration);
        assert_eq!(configuration.gear_id, full.gear_id);
        assert_eq!(configuration.configuration, full.configuration);
        assert!(memory.peak_bytes <= 4096, "display configuration-only decode: {memory:?}");
    }
    let mut raw_display_host = display_host.clone(); raw_display_host.capabilities.extend(raw);
    display_host.capabilities.extend(linked_offers);
    let grants: Vec<c::AuthorityGrant> = serde_json::from_slice(include_bytes!("../display-grants.json")).unwrap();
    assert_eq!(display_plan(&raw_display, &raw_display_host, &grants).unwrap().materialize().unwrap(), display_plan(&linked_display, &display_host, &grants).unwrap().materialize().unwrap());
    assert_eq!(display_plan(&raw_display, &raw_display_host, &[]).err(), display_plan(&linked_display, &display_host, &[]).err());
    let mut stale = grants; stale[0].boot_id = "stale/display".into();
    assert_eq!(display_plan(&raw_display, &raw_display_host, &stale).err(), display_plan(&linked_display, &display_host, &stale).err());
    println!("display_offers_raw_retained={} display_offers_linked_retained={} display_offers_linked_peak={}", raw_memory.retained_bytes(), linked_memory.retained_bytes(), linked_memory.peak_bytes);
    println!("rich_planning_peak={} linked_planning_peak={} linked_plan_retained={}", rich_plan_memory.peak_bytes, linked_plan_memory.peak_bytes, linked_plan_memory.retained_bytes());
    println!("offers_rich_retained={} offers_linked_retained={} offers_linked_peak={} largest_rich_gear_peak={} largest_linked_gear_peak={}", rich_memory.retained_bytes(), compact_memory.retained_bytes(), compact_memory.peak_bytes, largest_rich_peak, largest_linked_peak);
}
"#);
    fs::write(directory.join("src/main.rs"), generated).unwrap();
    let output = Command::new("cargo")
        .args([
            "+stable",
            "run",
            "--offline",
            "--quiet",
            "--features",
            "target-memory-check",
            "--manifest-path",
        ])
        .arg(directory.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", directory.join("target"))
        .output()
        .unwrap();
    fs::write(directory.join("proof.stdout"), &output.stdout).unwrap();
    fs::write(directory.join("proof.stderr"), &output.stderr).unwrap();
    assert!(
        output.status.success(),
        "generated Make material failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{}", String::from_utf8_lossy(&output.stdout));
}
