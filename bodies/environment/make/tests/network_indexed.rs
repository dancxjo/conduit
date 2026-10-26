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
// Compile the exact native projection against exclusively retained inert owners.
#[path = "../../../../targets/esp32/proof/tm1637/src/native_network_offer.rs"]
mod native_projection;
struct NativeRoot {
    host: c::HostId,
    boot: c::BootId,
    offer_generation: u64,
    credentials: Option<credential_installation::ReadyCredentialBase<Inert>>,
    play: SignOffer,
    network: JoinOffer,
}
struct SignOffer;
impl SignOffer {
    fn append_offer(&self, host: &mut c::HostAdvertisement) {
        host.capabilities.push(network_attachment_sign_local_offer(
            "fixture/sign".into(),
            "fixture/sign-implementation".into(),
            "fixture/artifact".into(),
        ));
    }
}
struct JoinOffer(Option<ReadyNetworkJoinBase<Inert>>);
impl JoinOffer {
    fn append_to_advertisement(&self, host: &mut c::HostAdvertisement) -> Result<(), ()> {
        if let Some(owner) = &self.0 {
            owner.append_to_advertisement(host).map_err(|_| ())?;
        }
        Ok(())
    }
}
impl conduit_monotonic_wait::MonotonicWaitProvider for Inert {
    async fn wait(&mut self, _: u64) -> Result<(), conduit_monotonic_wait::WaitProviderFailure> {
        panic!("planning cannot run unrelated timer")
    }
}
fn unrelated_wait(
    host: &c::HostAdvertisement,
) -> conduit_monotonic_wait::ReadyMonotonicWaitBase<Inert> {
    use conduit_monotonic_wait::*;
    // SAFETY: exclusively owned CPU fixture; its effect panics above.
    unsafe {
        ReadyMonotonicWaitBase::new(
            WaitNativeIdentity {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                base_id: "fixture/wait-base".into(),
                provider_instance_id: "fixture/wait-instance".into(),
                provider_generation: 1,
                resource_pool_id: "fixture/wait-pool".into(),
                resource_generation_id: c::ResourceGenerationId("fixture/wait-generation".into()),
                envelope_id: "fixture/wait-envelope".into(),
                artifact_id: "fixture/wait-artifact".into(),
                implementation_id: "fixture/wait-implementation".into(),
                base_implementation_id: "fixture/wait-base-implementation".into(),
            },
            WaitAttachment {
                generation: 1,
                resource_bytes: 64,
                maximum_duration_ms: 1000,
            },
            Inert,
        )
        .unwrap()
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
        profile: "environment/esp32-native-root@1".into(),
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
    credentials.append_to_advertisement(&mut host).unwrap();
    host.capabilities.push(network_attachment_sign_local_offer(
        "fixture/sign".into(),
        "fixture/sign-implementation".into(),
        "fixture/artifact".into(),
    ));
    join.append_to_advertisement(&mut host).unwrap();
    host.bases.sort_by(|a, b| a.base_id.cmp(&b.base_id));
    host.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    let mut root = NativeRoot {
        host: host.host_id.clone(),
        boot: host.boot_id.clone(),
        offer_generation: host.offer_generation.0,
        credentials: Some(credentials),
        play: SignOffer,
        network: JoinOffer(Some(join)),
    };
    let projected = root.network_advertisement().unwrap();
    assert_eq!(host, projected);
    root.boot = "wrong/boot".into();
    assert!(root.network_advertisement().is_err());
    root.boot = host.boot_id.clone();

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
    let mut full = projected.clone();
    unrelated_wait(&full)
        .append_to_advertisement(&mut full)
        .unwrap();
    full.bases.sort_by(|a, b| a.base_id.cmp(&b.base_id));
    full.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    for drop_kind in 0..3 {
        let mut narrow = projected.clone();
        let mut broad = full.clone();
        for advertisement in [&mut narrow, &mut broad] {
            if drop_kind == 0 {
                advertisement
                    .capabilities
                    .retain(|offer| offer.capability_id.as_str() != JOIN_CAPABILITY);
            }
            if drop_kind == 1 {
                advertisement
                    .resources
                    .retain(|offer| offer.pool_id.as_str() != "fixture/resource/join");
            }
        }
        let authority = if drop_kind == 2 { &[][..] } else { &grants[..] };
        let narrow = planning::PreparedNetworkSetup::from_indexed_for_identity(
            &expected, &image, &narrow, &bases, authority,
        )
        .err()
        .unwrap();
        let broad = planning::PreparedNetworkSetup::from_indexed_for_identity(
            &expected, &image, &broad, &bases, authority,
        )
        .err()
        .unwrap();
        assert_eq!(narrow.code(), broad.code());
    }
    let (_, scoped_allocation) =
        allocation_probe::observe(|| root.network_advertisement().unwrap());
    let (_, complete_allocation) = allocation_probe::observe(|| {
        let mut ad = root.network_advertisement().unwrap();
        unrelated_wait(&ad)
            .append_to_advertisement(&mut ad)
            .unwrap();
        ad
    });
    assert!(scoped_allocation.retained_bytes() < complete_allocation.retained_bytes());
    println!("CPU current-offer projection retained={} versus network+oneunrelatedtimer={} (not target admission)",scoped_allocation.retained_bytes(),complete_allocation.retained_bytes());
    let full_plan = planning::PreparedNetworkSetup::from_indexed_for_identity(
        &expected, &image, &full, &bases, &grants,
    )
    .unwrap();
    assert_eq!(full_plan.plan(), owned.plan());
    assert_eq!(full_plan.lowered(), owned.lowered());
    // Exact retained owner identities remain untouched, including generations.
    assert_eq!(
        root.credentials.as_ref().unwrap().native_identity(),
        &identity("credentials")
    );
    assert_eq!(
        root.network.0.as_ref().unwrap().native_identity(),
        &identity("join")
    );
    for base in &projected.bases {
        assert_eq!(
            full.bases.iter().find(|b| b.base_id == base.base_id),
            Some(base)
        );
    }
    // Complete and scoped projections retain the same selected resource facts.
    for resource in &projected.resources {
        assert_eq!(
            full.resources
                .iter()
                .find(|r| r.pool_id == resource.pool_id),
            Some(resource)
        );
    }
    let retained = root.credentials.take();
    let absent = root.network_advertisement().unwrap();
    assert!(planning::PreparedNetworkSetup::from_indexed_for_identity(
        &expected, &image, &absent, &bases, &grants
    )
    .is_err());
    root.credentials = retained;
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
