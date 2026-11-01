//! Executed emitted-Rust conformance, with inert provider metadata only.
use conduit_environment_make::{SevenSegmentMaskMaterial, SevenSegmentMaskSource};
use std::{fs, path::Path, process::Command};
#[allow(dead_code, reason = "only inert metadata is used")]
#[path = "support/mask.rs"]
mod fixture;

#[test]
fn emitted_ordinary_mask_preserves_complete_records_fore_role_plans_and_refusals() {
    let source = SevenSegmentMaskSource::compile().unwrap();
    let material = SevenSegmentMaskMaterial::prepare(&source).unwrap();
    assert_eq!(material.role(), &source.role);
    assert_eq!(source.plot.expanded.gears.len(), 3);
    let mut forged = SevenSegmentMaskSource::compile().unwrap();
    forged.role.plot_name.push_str("forged");
    assert!(SevenSegmentMaskMaterial::prepare(&forged).is_err());
    let mut forged_boundary = SevenSegmentMaskSource::compile().unwrap();
    forged_boundary.plot.input_bindings[0].gear_id = "foreign/gear".into();
    assert!(SevenSegmentMaskMaterial::prepare(&forged_boundary).is_err());
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/mask-material-conformance");
    fs::create_dir_all(directory.join("src")).unwrap();
    let path = |p: &str| root.join(p).to_string_lossy().into_owned();
    fs::write(
        directory.join("Cargo.toml"),
        format!(
            r#"
[package]
name = "conduit-mask-material-conformance"
version = "0.0.0"
edition = "2021"
[workspace]
[dependencies]
conduit-core = {{ path = {:?} }}
conduit-plot = {{ path = {:?}, features = ["flat-image"] }}
conduit-planner = {{ path = {:?}, features = ["indexed-source"] }}
conduit-face = {{ path = {:?} }}
serde_json = "1"
[profile.dev]
debug = 0
incremental = false
[profile.dev.build-override]
opt-level = 1
"#,
            path("architecture/core"),
            path("architecture/plot"),
            path("architecture/planner"),
            path("semantics/face")
        ),
    )
    .unwrap();
    let save = |name: &str, value: &serde_json::Value| {
        fs::write(directory.join(name), serde_json::to_vec(value).unwrap()).unwrap()
    };
    fs::write(directory.join("source.bin"), material.planning_image()).unwrap();
    save(
        "gears.json",
        &serde_json::to_value(&source.plot.expanded.gears).unwrap(),
    );
    save("role.json", &serde_json::to_value(&source.role).unwrap());
    save(
        "front.json",
        &serde_json::to_value(&source.plot.front).unwrap(),
    );
    save(
        "inputs.json",
        &serde_json::to_value(format!("{:?}", source.plot.input_bindings)).unwrap(),
    );
    save(
        "outputs.json",
        &serde_json::to_value(format!("{:?}", source.plot.output_bindings)).unwrap(),
    );
    let mut hosts = Vec::new();
    let mut expected = Vec::new();
    for scenario in 0..4 {
        let mut host = fixture::host();
        match scenario {
            1 => host.resources.clear(),
            2 => host
                .capabilities
                .retain(|offer| offer.kind_id.as_str() != conduit_face::RENDERER_KIND),
            3 => host
                .capabilities
                .iter_mut()
                .for_each(|offer| offer.limits.max_queue_bytes = 0),
            _ => (),
        }
        expected.push(source.plan(&host, 8192, &[]).map(|planned| planned.plan));
        hosts.push(host);
    }
    assert!(expected[0].is_ok());
    assert!(expected[1..].iter().all(Result::is_err));
    save("hosts.json", &serde_json::to_value(hosts).unwrap());
    save("expected.json", &serde_json::to_value(expected).unwrap());
    let mut rust = String::from("extern crate alloc;\nmod material {\n");
    material.write_rust(&mut rust).unwrap();
    rust.push_str(r#"
}
use conduit_core as c;
use conduit_planner::{indexed_source as indexed, PlanningOptions, ConnectionQueueLimits, ForeBoundaryKey};
use std::collections::BTreeMap;
fn main() {
    assert_eq!(material::planning_source_image(), include_bytes!("../source.bin"));
    assert_eq!(material::planning_source_sha256(), c::assigned_plan_payload_digest(material::planning_source_image()));
    assert_eq!(serde_json::to_value(material::role().unwrap()).unwrap(), serde_json::from_slice::<serde_json::Value>(include_bytes!("../role.json")).unwrap());
    assert_eq!(serde_json::from_slice::<serde_json::Value>(material::FRONT).unwrap(), serde_json::from_slice::<serde_json::Value>(include_bytes!("../front.json")).unwrap());
    assert_eq!(format!("{:?}", material::planning_inputs()), serde_json::from_slice::<String>(include_bytes!("../inputs.json")).unwrap());
    assert_eq!(format!("{:?}", material::planning_outputs()), serde_json::from_slice::<String>(include_bytes!("../outputs.json")).unwrap());
    let gears: Vec<conduit_plot::CheckedGear> = serde_json::from_slice(include_bytes!("../gears.json")).unwrap();
    // SAFETY: this executed test verifies the original immutable image digest
    // and exact emitted callbacks against every complete original Gear below.
    let image = unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(material::planning_source_image()).unwrap()
        .with_make_gear_decoder(material::decode_planning_gear)
        .with_make_configuration_decoder(material::decode_planning_configuration) };
    assert_eq!(image.gear_count(), 3);
    assert_eq!(image.identity().unwrap(), material::source());
    for (index, expected) in gears.iter().enumerate() {
        assert_eq!(&image.gear(index).unwrap(), expected);
        let configuration = image.gear_configuration(index).unwrap();
        assert_eq!(configuration.gear_id, expected.gear_id);
        assert_eq!(configuration.configuration, expected.configuration);
    }
    assert!(material::decode_planning_gear(3).is_err());
    assert!(material::decode_planning_configuration(3).is_err());
    let hosts: Vec<c::HostAdvertisement> = serde_json::from_slice(include_bytes!("../hosts.json")).unwrap();
    let expected: Vec<Result<c::Plan,String>> = serde_json::from_slice(include_bytes!("../expected.json")).unwrap();
    let boundary = material::planning_boundary().unwrap();
    for (host, expected) in hosts.into_iter().zip(expected) {
        let actual = (|| -> Result<c::Plan,String> {
            let placements = indexed::default_placements(&image, std::slice::from_ref(&host)).map_err(|e|format!("{e:?}"))?;
            let limits: BTreeMap<_,_> = boundary.bindings().map(|(direction, port, track)| (ForeBoundaryKey {direction, front_port_id:port.clone(),track}, ConnectionQueueLimits {item_capacity:1,byte_capacity:8192})).collect();
            let empty = BTreeMap::new();
            let bases = ["conduit.base/local@1".into()];
            let plan = indexed::plan_compact_authoring_consuming_with_connection_limits(&image, &boundary, vec![host], &placements, &bases, PlanningOptions {
                connection_bases:&empty, line_candidates:&BTreeMap::new(), connection_item_capacity:1,connection_byte_capacity:8192,
                authority_grants:&[],protected_resource_grants:&[],line_offers:&[],
            }, &BTreeMap::new(), &limits).and_then(|p|p.materialize()).map_err(|e|format!("{e:?}"))?;
            conduit_face::PlannedMaskPlot::admit(&material::role().unwrap(), &plan).map_err(|e|format!("{e:?}"))?;
            Ok(plan)
        })();
        assert_eq!(actual, expected);
    }
}
"#);
    fs::write(directory.join("src/main.rs"), rust).unwrap();
    let output = Command::new("cargo")
        .args(["+stable", "run", "--offline", "--quiet", "--manifest-path"])
        .arg(directory.join("Cargo.toml"))
        .env("CARGO_INCREMENTAL", "0")
        .env(
            "CARGO_TARGET_DIR",
            Path::new(env!("CARGO_MANIFEST_DIR")).join("target/linked-material-conformance/target"),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
