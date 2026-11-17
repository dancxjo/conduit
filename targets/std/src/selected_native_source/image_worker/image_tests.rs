//! Actual ambient owner and original 128MiB IMAGE construction, without IO.
use super::*;
use crate::{
    ambient_observation::PreparedAmbientObservation,
    ambient_wallpaper::{acquisition::*, MAXIMUM_APOD_RESPONSE_BYTES},
    *,
};
use ::image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
use conduit_core::*;
use conduit_host_make::*;
use conduit_std_offers::*;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
const RETAINED: u32 = 16 * 1024 * 1024;
fn budget() -> NativeWorkerBudget {
    NativeWorkerBudget {
        requested_stack_bytes: 2 * 1024 * 1024,
        maximum_stack_mapping_bytes: 2 * 1024 * 1024 + 64 * 1024,
        capture_timeout: std::time::Duration::from_secs(30),
        cancellation_timeout: std::time::Duration::from_secs(1),
    }
}
fn offline_deck() -> PreparedDeck {
    let admission = PreparationAdmission::admit(
        chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
        MAXIMUM_RETAINED_BYTES,
        PREPARATION_BYTES,
    )
    .unwrap();
    let mut number = 0;
    prepare_deck(admission,|url,bound| {
        if bound==MAXIMUM_APOD_RESPONSE_BYTES {Ok(serde_json::to_vec(&serde_json::json!({"date":url.rsplit('=').next().unwrap(),"title":"Actual fixture image","media_type":"image","hdurl":"https://apod.nasa.gov/fixture.png"})).unwrap())}
        else {number+=1;let mut bytes=Vec::new();PngEncoder::new(&mut bytes).write_image(&[number,2,3,255],1,1,ExtendedColorType::Rgba8).unwrap();Ok(bytes)}
    }).unwrap()
}
struct Provider {
    inner: PreparedAmbientObservation,
    dropped: Arc<AtomicUsize>,
}
// SAFETY: affine actual prepared ambient owner; delegates the reviewed bounded
// cooperative protocol. Constructor tests make no reads or network requests.
unsafe impl NativeSourceWorkerProvider for Provider {
    fn read(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        context: &NativeReadContext<'_>,
    ) -> Result<usize, WorkerFailure> {
        self.inner.read(input, output, context)
    }
}
impl Drop for Provider {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::AcqRel);
    }
}
struct Fixture {
    image: HostImage,
    catalog: MakeCatalog,
    checked: CheckedHostConfiguration,
    config: StdHostConfig,
    definition: NativeSourceDefinition,
}
fn fixture() -> Fixture {
    let source = r#"host ambient {
      schema = 1
      target = {architecture: "x86_64", machine: "computer", os: "linux"}
      base = {kind: "ambient/observation", implementation: "std/ambient-observation-worker@1", initialization: {timezone: "America/Los_Angeles", maximum_updates: 10, weather_endpoint: "127.0.0.1:43210", source_pool: "ambient/source", memory_pool: "ambient/memory", memory_bytes: 67108864, preparation_bytes: 58720256, retained_bytes: 16777216}}
      base = {kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}
      capability = {kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}
      capability = {kind: "ambient/home-observation", pinned_implementation: "std/ambient-observation-worker@1"}
      need = {id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 1, bytes: 512}
      need = {id: "ambient/source", class: "conduit.resource/ambient-observation-source@1", slots: 1, bytes: 8192}
      need = {id: "ambient/memory", class: "conduit.resource/runtime-memory@1", slots: 67108864, bytes: 67108864}
      limits = {static_memory_bytes: 16777216, heap_arena_bytes: 134217728, queue_items: 32, buffered_bytes: 1048576, active_instances: 16, operation_slots: 16, timer_slots: 16, line_sessions: 16, evidence_items: 16}
    }"#;
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let init =
        conduit_host_hosted::CheckedAmbientInitialization::from_configuration(&checked).unwrap();
    assert_eq!(init.memory_bytes, 64 * 1024 * 1024);
    let image = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/ambient-worker-image".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    let reference = StdHost::new();
    let ad = reference.advertisement();
    let config = StdHostConfig {
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        offer_generation: ad.offer_generation,
    };
    let kind = conduit_semantic_catalog::ambient_observe_kind().unwrap();
    let offer = ambient_source_offer().unwrap();
    let selected = image
        .manifest
        .base_selections
        .iter()
        .find(|b| b.kind == AMBIENT_SOURCE_BASE_KIND)
        .unwrap();
    // This current independent fixture authorization is deliberately supplied
    // apart from the checked equipment, offers and any Plan.
    let grant = AuthorityGrant {
        grant_id: "fixture/independent-ambient-read".into(),
        contract_id: AMBIENT_SOURCE_AUTHORITY.into(),
        host_call_contract_id: AMBIENT_SOURCE_CALL.into(),
        subject_kind: kind.kind_id.clone(),
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    let definition = NativeSourceDefinition {
        kind,
        grant,
        base: BaseProviderEntry {
            base_id: selected.id.as_str().into(),
            provider_instance_id: "fixture/actual-ambient-owner".into(),
            provider_generation: 1,
            implementation_id: AMBIENT_SOURCE_IMPLEMENTATION.into(),
            mechanism_family: AMBIENT_SOURCE_BASE_KIND.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![
                ResourceOffer {
                    pool_id: init.source_pool.into(),
                    class_id: AMBIENT_SOURCE_RESOURCE.into(),
                    capacity_units: 1,
                    compute: None,
                    content: None,
                },
                ResourceOffer {
                    pool_id: init.memory_pool.into(),
                    class_id: RUNTIME_MEMORY_RESOURCE_CLASS.into(),
                    capacity_units: init.memory_bytes,
                    compute: None,
                    content: None,
                },
            ],
        },
        resource_generation: ResourceGenerationId("fixture/ambient-generation@1".into()),
        configuration: vec![
            ConfigurationEntry {
                key: "timezone".into(),
                value: ConfigurationValue::Text(init.timezone),
            },
            ConfigurationEntry {
                key: "maximum-updates".into(),
                value: ConfigurationValue::U64(u64::from(match init.lifetime {
                    conduit_host_hosted::AmbientObservationLifetime::Finite(count) => count,
                    conduit_host_hosted::AmbientObservationLifetime::Resident => {
                        panic!("finite fixture")
                    }
                })),
            },
        ],
        input_bytes: 8,
        output_bytes: 8192,
        resource_bytes: 8192,
    };
    Fixture {
        image,
        catalog,
        checked,
        config,
        definition,
    }
}
fn construct(
    f: Fixture,
    called: &AtomicBool,
    dropped: Arc<AtomicUsize>,
    short: bool,
) -> Result<StdHost, String> {
    let Fixture {
        image,
        catalog,
        checked,
        config,
        definition,
    } = f;
    let init = conduit_host_hosted::CheckedAmbientInitialization::from_configuration(&checked)?;
    // SAFETY: fixture independently authorizes the exact Host/Boot/provider;
    // complete original metadata and provider peaks are admitted before work.
    unsafe {
        StdHost::from_selected_timer_and_worker_source_factory_image(
            config,
            &image,
            &catalog,
            &checked,
            ImmediateSourcePreparationBudget {
                preparation_bytes: 1048576,
                retained_bytes: 1048576,
            },
            None,
            None,
            move |_, _| {
                Ok((
                    definition,
                    ImmediateSourcePreparationBudget {
                        preparation_bytes: if short {
                            RETAINED - 1
                        } else {
                            AMBIENT_SOURCE_PROVIDER_PREPARATION_BYTES
                        },
                        retained_bytes: RETAINED,
                    },
                    budget(),
                    move || {
                        called.store(true, Ordering::Release);
                        let provider = PreparedAmbientObservation::prepare(
                            &init.timezone,
                            init.weather_endpoint,
                            offline_deck(),
                            match init.lifetime {
                                conduit_host_hosted::AmbientObservationLifetime::Finite(count) => {
                                    count
                                }
                                conduit_host_hosted::AmbientObservationLifetime::Resident => {
                                    panic!("finite fixture")
                                }
                            },
                        )?;
                        if provider.prepared_heap_bytes() + core::mem::size_of::<Provider>()
                            > RETAINED as usize
                        {
                            return Err("actual provider exceeds retained budget".into());
                        }
                        Ok(Box::new(Provider {
                            inner: provider,
                            dropped,
                        })
                            as Box<dyn NativeSourceWorkerProvider>)
                    },
                ))
            },
        )
    }
}
#[test]
fn actual_ambient_original_image_retains_shared_counter_and_affine_owner() {
    let called = AtomicBool::new(false);
    let dropped = Arc::new(AtomicUsize::new(0));
    let mut host = construct(fixture(), &called, dropped.clone(), false).unwrap();
    assert!(called.load(Ordering::Acquire));
    let observed = host.kernel_resources.image_memory_observer();
    let baseline = observed.load(Ordering::Acquire);
    let inspection = host.native_source_worker_inspection().unwrap();
    assert!(inspection.idle_backing_is_reserved);
    assert_eq!(
        inspection.backing.charged_bytes,
        AMBIENT_SOURCE_WORKER_BACKING_BYTES
    );
    assert_eq!(
        inspection.reserved_bytes,
        RETAINED + AMBIENT_SOURCE_WORKER_BACKING_BYTES
    );
    assert!(baseline >= inspection.reserved_bytes as usize + 1048576);
    assert_eq!(
        host.kernel_resources
            .admitted_image_bounds()
            .unwrap()
            .heap_arena_bytes,
        128 * 1024 * 1024
    );
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    // IMAGE retirement cannot become a reference reconfiguration bypass.
    assert!(host
        .retire_native_source_worker()
        .err()
        .unwrap()
        .contains("fresh checked construction"));
    assert!(host.native_source_worker_inspection().is_some());
    assert_eq!(observed.load(Ordering::Acquire), baseline);
    drop(host);
    assert_eq!(dropped.load(Ordering::Acquire), 1);
    // This observer still owns the original counter allocation itself.
    assert_eq!(
        observed.load(Ordering::Acquire),
        crate::kernel_preparation::StagedImageMemory::counter_backing_bytes()
    );
}
#[test]
fn wrong_host_boot_and_one_byte_short_provider_budget_refuse_before_provider() {
    for mode in 0..3 {
        let mut f = fixture();
        if mode == 0 {
            f.definition.grant.host_id = "fixture/wrong-host".into();
        }
        if mode == 1 {
            f.definition.grant.boot_id = "fixture/wrong-boot".into();
        }
        let called = AtomicBool::new(false);
        let dropped = Arc::new(AtomicUsize::new(0));
        assert!(construct(f, &called, dropped.clone(), mode == 2).is_err());
        assert!(!called.load(Ordering::Acquire));
        assert_eq!(dropped.load(Ordering::Acquire), 0);
    }
}
#[test]
fn invalid_or_oversized_construction_config_refuses_before_provider() {
    for oversized in [false, true] {
        let mut f = fixture();
        f.config.host_id = if oversized {
            "x".repeat(65537).into()
        } else {
            "".into()
        };
        let called = AtomicBool::new(false);
        assert!(construct(f, &called, Arc::new(AtomicUsize::new(0)), false).is_err());
        assert!(!called.load(Ordering::Acquire));
    }
}

#[path = "../image_pair/tests.rs"]
mod pair_tests;

#[test]
fn operation_envelope_uses_original_owner_configuration_and_never_reissues_possession() {
    use super::super::selection::operation_limit;
    let mut d = fixture().definition;
    assert_eq!(operation_limit(&d, true).unwrap(), 10);
    assert_eq!(operation_limit(&d, false).unwrap(), 1);
    let count = d
        .configuration
        .iter()
        .position(|v| v.key == "maximum-updates")
        .unwrap();
    for n in [0, 4097, u64::MAX] {
        d.configuration[count].value = ConfigurationValue::U64(n);
        assert!(operation_limit(&d, true).is_err());
    }
    d.configuration[count].value = ConfigurationValue::U64(4096);
    assert_eq!(operation_limit(&d, true).unwrap(), 4096);
    d.configuration.push(d.configuration[count].clone());
    assert!(operation_limit(&d, true).is_err());
    d.configuration.pop();
    d.configuration.remove(count);
    assert!(operation_limit(&d, true).is_err());
    d.kind = conduit_semantic_catalog::ambient_resident_observe_kind().unwrap();
    d.base.capabilities = vec![ambient_resident_source_offer().unwrap()];
    assert_eq!(operation_limit(&d, true).unwrap(), u32::MAX);
    d.configuration.push(ConfigurationEntry {
        key: "maximum-updates".into(),
        value: ConfigurationValue::U64(1440),
    });
    assert!(operation_limit(&d, true).is_err());
    d.configuration.pop();
    d.base.capabilities[0].implementation.execution_profile_id = "fixture/wrong-profile".into();
    assert!(operation_limit(&d, true).is_err());
}
