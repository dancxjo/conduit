//! Same local task planner, current offers and authority; no radio effects.
extern crate alloc;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "../src/network_planning.rs"]
#[allow(dead_code)]
mod planning;
use conduit_core as c;
use conduit_net::*;
use conduit_network_join_call::{installation::*, *};
struct Inert;
impl NetworkJoinProvider for Inert {
    async fn join(
        &mut self,
        _: NetworkJoinRequest<'_>,
    ) -> Result<AttachmentRecord, StationAttemptFailure> {
        panic!("planning has no radio effects")
    }
    async fn cleanup(&mut self) -> Result<(), StationFailure> {
        panic!("planning has no cleanup effects")
    }
}
impl CredentialSource for Inert {
    fn take_acknowledged(&mut self, _: &mut [u8]) -> Result<usize, CredentialSourceRefusal> {
        panic!("planning cannot consume secrets")
    }
}
fn identity(role: &str) -> NetworkNativeIdentity {
    NetworkNativeIdentity {
        host: "fixture/host".into(),
        boot: "fixture/boot".into(),
        base: format!("fixture/base/{role}").into(),
        instance: format!("fixture/instance/{role}").into(),
        provider_generation: 1,
        resource: format!("fixture/resource/{role}").into(),
        resource_generation: c::ResourceGenerationId(format!("fixture/generation/{role}")),
        envelope: format!("fixture/envelope/{role}").into(),
        artifact: format!("fixture/artifact/{role}").into(),
        implementation: format!("fixture/implementation/{role}").into(),
        base_implementation: format!("fixture/base-implementation/{role}").into(),
    }
}
#[test]
fn indexed_task_planning_matches_owned_plan_and_preserves_refusals() {
    let source = conduit_environment_make::compile_network_setup().unwrap();
    let expected = c::PlotIdentity {
        source_document_id: source.source_document_id.clone(),
        checked_plot_id: source.checked_plot_id.clone(),
        expanded_plot_id: source.expanded_plot_id.clone(),
    };
    let bytes = conduit_plot::flat_image::indexed::encode(&source).unwrap();
    // SAFETY: this fixture just checked and encoded the complete Source.
    let image =
        unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&bytes) }
            .unwrap();
    let mut host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/network".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    // SAFETY: inert exclusively owned CPU fixtures; effects panic above.
    let (join, credentials) = unsafe {
        (
            ReadyNetworkJoinBase::new(identity("join"), 1, Inert).unwrap(),
            credential_installation::ReadyCredentialBase::new(identity("credentials"), Inert)
                .unwrap(),
        )
    };
    join.append_to_advertisement(&mut host).unwrap();
    credentials.append_to_advertisement(&mut host).unwrap();
    host.capabilities.push(network_attachment_sign_local_offer(
        "fixture/sign".into(),
        "fixture/sign-implementation".into(),
        "fixture/artifact".into(),
    ));
    host.bases.sort_by(|a, b| a.base_id.cmp(&b.base_id));
    host.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    let grants = [
        c::AuthorityGrant {
            grant_id: "grant/credentials".into(),
            contract_id: NETWORK_CREDENTIALS_AUTHORITY.into(),
            host_call_contract_id: NETWORK_CREDENTIALS_HOST_CALL.into(),
            subject_kind: NETWORK_CREDENTIALS_SUBJECT.into(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            capability_id: credential_installation::CREDENTIAL_CAPABILITY.into(),
        },
        c::AuthorityGrant {
            grant_id: "grant/join".into(),
            contract_id: NETWORK_CONFIG_AUTHORITY.into(),
            host_call_contract_id: NETWORK_JOIN_HOST_CALL.into(),
            subject_kind: NETWORK_CONFIG_SUBJECT.into(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            capability_id: JOIN_CAPABILITY.into(),
        },
    ];
    let bases = [
        "conduit.base/local@1".into(),
        "fixture/base-implementation/join".into(),
        "fixture/base-implementation/credentials".into(),
    ];
    assert_linked_network_conformance(&source, &expected, &host, &bases, &grants);
    let owned = planning::PreparedNetworkSetup::from_checked_for_identity(
        &expected, &source, &host, &bases, &grants,
    )
    .unwrap();
    let (indexed, allocation) = allocation_probe::observe(|| {
        planning::PreparedNetworkSetup::from_indexed_for_identity(
            &expected, &image, &host, &bases, &grants,
        )
        .unwrap()
    });
    let (kernel, kernel_allocation) = allocation_probe::observe(|| {
        PreparedNetworkKernel::prepare(&indexed.plan().fragments[0], indexed.lowered()).unwrap()
    });
    // CPU fixture sizes diagnose simultaneous preparation phases; they do not
    // admit ESP32 heap fragmentation, concurrent Mask scratch or native owners.
    println!("network indexed CPU planning peak={} retained={} allocations={} kernel-additional-peak={} kernel-retained={} kernel-fixed-and-sign-bytes={}",
        allocation.peak_bytes, allocation.retained_bytes(), allocation.allocations,
        kernel_allocation.peak_bytes, kernel_allocation.retained_bytes(), CONTROL_KERNEL_BYTES);
    assert!(allocation.peak_bytes >= allocation.retained_bytes());
    assert!(kernel_allocation.retained_bytes() >= CONTROL_SIGN_BACKING_BYTES);
    drop(kernel);
    assert_eq!(indexed.plan(), owned.plan());
    assert_eq!(indexed.lowered(), owned.lowered());
    assert!(c::verify_plan(indexed.plan()));
    assert!(planning::PreparedNetworkSetup::from_indexed_for_identity(
        &expected,
        &image,
        &host,
        &bases,
        &[]
    )
    .is_err());
    let mut wrong = expected.clone();
    wrong.expanded_plot_id = "wrong/source".into();
    assert!(planning::PreparedNetworkSetup::from_indexed_for_identity(
        &wrong, &image, &host, &bases, &grants
    )
    .is_err());
    host.capabilities
        .retain(|offer| offer.capability_id.as_str() != JOIN_CAPABILITY);
    assert!(planning::PreparedNetworkSetup::from_indexed_for_identity(
        &expected, &image, &host, &bases, &grants
    )
    .is_err());
}

fn assert_linked_network_conformance(
    source: &conduit_plot::ExpandedCanonicalPlot,
    expected: &c::PlotIdentity,
    host: &c::HostAdvertisement,
    bases: &[c::BaseImplementationId],
    grants: &[c::AuthorityGrant],
) {
    use std::{fs, path::Path, process::Command};
    let material = conduit_environment_make::NetworkSourceMaterial::prepare(source).unwrap();
    assert_eq!(
        material.planning_image(),
        conduit_plot::flat_image::indexed::encode(source).unwrap()
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/network-linked-conformance");
    fs::create_dir_all(directory.join("src")).unwrap();
    let path = |suffix: &str| root.join(suffix).to_string_lossy().into_owned();
    fs::write(
        directory.join("Cargo.toml"),
        format!(
            r#"
[package]
name = "conduit-network-linked-conformance"
version = "0.0.0"
edition = "2021"
[workspace]
[dependencies]
conduit-core = {{path={:?}}}
conduit-plot = {{path={:?},features=["flat-image"]}}
conduit-planner = {{path={:?},features=["indexed-source"]}}
conduit-plan-lowering = {{path={:?}}}
conduit-net = {{path={:?}}}
serde_json = "1"
[profile.dev.build-override]
opt-level = 1
"#,
            path("architecture/core"),
            path("architecture/plot"),
            path("architecture/planner"),
            path("architecture/plan-lowering"),
            path("semantics/net")
        ),
    )
    .unwrap();
    fs::write(directory.join("image.bin"), material.planning_image()).unwrap();
    let save = |name: &str, value: &serde_json::Value| {
        fs::write(directory.join(name), serde_json::to_vec(value).unwrap()).unwrap()
    };
    save("gears.json", &serde_json::to_value(&source.gears).unwrap());
    save("identity.json", &serde_json::to_value(expected).unwrap());
    save("bases.json", &serde_json::to_value(bases).unwrap());
    save("grants.json", &serde_json::to_value(grants).unwrap());
    let mut hosts = vec![host.clone(); 4];
    hosts[2].capabilities.pop();
    hosts[3].resources.clear();
    let results = hosts
        .iter()
        .enumerate()
        .map(|(index, host)| {
            planning::PreparedNetworkSetup::from_checked_for_identity(
                expected,
                source,
                host,
                bases,
                if index == 1 { &[] } else { grants },
            )
            .map(|prepared| prepared.plan().clone())
            .map_err(|error| error.code().to_owned())
        })
        .collect::<Vec<_>>();
    assert!(results[0].is_ok());
    assert!(results[1..].iter().all(Result::is_err));
    save("hosts.json", &serde_json::to_value(hosts).unwrap());
    save("expected.json", &serde_json::to_value(results).unwrap());
    let mut rust = format!(
        "extern crate alloc;\n#[path={:?}] mod planning;\nmod material {{\n",
        path("bodies/environment/make/src/network_planning.rs")
    );
    material.write_rust(&mut rust).unwrap();
    rust.push_str(r#"
}
fn main() {
    use conduit_plot::flat_image::indexed::IndexedSourceImage;
    // SAFETY: this CPU fixture encoded the same checked Source above.
    let image = unsafe { IndexedSourceImage::from_make_image(include_bytes!("../image.bin")).unwrap()
        .with_make_gear_decoder(material::decode_planning_gear)
        .with_make_configuration_decoder(material::decode_planning_configuration) };
    let gears: Vec<conduit_plot::CheckedGear> = serde_json::from_slice(include_bytes!("../gears.json")).unwrap();
    for (index, gear) in gears.iter().enumerate() {
        assert_eq!(&image.gear(index).unwrap(),gear);
        let configuration = image.gear_configuration(index).unwrap();
        assert_eq!(configuration.gear_id,gear.gear_id);
        assert_eq!(configuration.configuration,gear.configuration);
    }
    assert!(material::decode_planning_gear(gears.len()).is_err());
    assert!(material::decode_planning_configuration(gears.len()).is_err());
    let identity: conduit_core::PlotIdentity = serde_json::from_slice(include_bytes!("../identity.json")).unwrap();
    let bases: Vec<conduit_core::BaseImplementationId> = serde_json::from_slice(include_bytes!("../bases.json")).unwrap();
    let grants: Vec<conduit_core::AuthorityGrant> = serde_json::from_slice(include_bytes!("../grants.json")).unwrap();
    let hosts: Vec<conduit_core::HostAdvertisement> = serde_json::from_slice(include_bytes!("../hosts.json")).unwrap();
    let expected: Vec<Result<conduit_core::Plan,String>> = serde_json::from_slice(include_bytes!("../expected.json")).unwrap();
    for (index,(host,expected)) in hosts.iter().zip(expected).enumerate() {
        let actual = planning::PreparedNetworkSetup::from_indexed_for_identity(&identity,&image,host,&bases,if index==1 {&[]} else {&grants})
            .map(|prepared|prepared.plan().clone()).map_err(|error|error.code().to_owned());
        assert_eq!(actual,expected);
    }
    // Malformed construction-only metadata remains refused by the later
    // planning projection, even though the preliminary pin check is narrow.
    let original = include_bytes!("../image.bin");
    let count = u16::from_le_bytes(original[8..10].try_into().unwrap()) as usize;
    let start = 16 + (count + 1) * 4;
    let length = u32::from_le_bytes(original[12..16].try_into().unwrap()) as usize;
    let baseline: serde_json::Value = serde_json::from_slice(&original[start..start+length]).unwrap();
    for scenario in 0..4 {
        let mut metadata = baseline.clone();
        match scenario {
            0 => metadata["version"] = serde_json::json!(2),
            1 => metadata["gears"] = serde_json::json!([gears[0]]),
            2 => { metadata["provenance"].as_array_mut().unwrap().pop(); },
            3 => {
                let connection = metadata["connections"][0].clone();
                metadata["connections"] = serde_json::json!(vec![connection;65]);
            },
            _ => unreachable!(),
        }
        let metadata = serde_json::to_vec(&metadata).unwrap();
        let mut damaged = original[..start].to_vec();
        damaged[12..16].copy_from_slice(&(metadata.len() as u32).to_le_bytes());
        damaged.extend_from_slice(&metadata);
        damaged.extend_from_slice(&original[start+length..]);
        // SAFETY: inert malformed CPU fixture, never an admitted Source image.
        if let Ok(bad) = unsafe { IndexedSourceImage::from_make_image(&damaged) } {
            assert!(bad.metadata().is_err());
            assert!(bad.planning_metadata().is_err());
            assert!(planning::PreparedNetworkSetup::from_indexed_for_identity(&identity,&bad,&hosts[0],&bases,&grants).is_err());
        }
    }
    let mut wrong = identity;
    wrong.checked_plot_id = "wrong/source".into();
    assert_eq!(planning::PreparedNetworkSetup::from_indexed_for_identity(&wrong,&image,&hosts[0],&bases,&grants).err().unwrap().code(),"source-mismatch");
}
"#);
    fs::write(directory.join("src/main.rs"), rust).unwrap();
    let output = Command::new("cargo")
        .args(["+stable", "run", "--offline", "--quiet", "--manifest-path"])
        .arg(directory.join("Cargo.toml"))
        .env("CARGO_INCREMENTAL", "0")
        .env_remove("CARGO_MAKEFLAGS")
        .env_remove("MAKEFLAGS")
        .args(["--jobs", "2"])
        .env(
            "CARGO_TARGET_DIR",
            root.join("target/network-source-conformance"),
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
