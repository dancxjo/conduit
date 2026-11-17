//! Actual compiler and ambient owners share one original construction counter.
//! The existing compiler catalog fixture needs its existing 603979776-byte heap;
//! this proves preservation of original Root custody, not 128MiB compiler fit.
use super::*;
use crate::source_check::*;
use conduit_plot::*;
pub fn owner(source: &str, generation: u64) -> (SourceCheckProjection, SourceCheckRequest) {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profiles).unwrap();
    let context = SourceCheckContext::new(
        CheckContextRevision {
            grammar_edition: "conduitese-v1".into(),
            catalog: "fixture/source-check@1".into(),
            kinds_fores: "reviewed/text-pipeline@1".into(),
            types_laws: "reviewed/text-types@1".into(),
            backs: "empty@1".into(),
        },
        startup,
        profiles,
        CanonicalBackCatalog::new(),
    )
    .unwrap();
    let owner =
        SourceCheckProjection::new("fixture/authorized-source", generation, source, context)
            .unwrap();
    let request = SourceCheckRequest {
        snapshot: owner.snapshot().clone(),
        context: owner.context().clone(),
        entry: "hello".into(),
        limits: SourceCheckLimits::default(),
    };
    (owner, request)
}
pub fn definition_for(
    host: &HostAdvertisement,
    kind: Kind,
    bounds: SourceCheckPreparationBounds,
    resource_bytes: u64,
) -> NativeSourceDefinition {
    let offer = conduit_core::capability_offer_from_parts! {
        startup_parameters: vec![], shorthand: None,
        capability_id: "fixture/source-check".into(), kind_id: kind.kind_id.clone(),
        kind_contract_revision: kind.kind_contract_revision.clone(), inputs: kind.inputs.clone(), outputs: kind.outputs.clone(), semantic_contract: kind.semantic_contract(),
        implementation: conduit_core::ImplementationOffer { execution_profile_id: "fixture/prepared-source-check@1".into(), implementation_id: "fixture/source-check".into(), artifact_id: "fixture/canonical-rust-checker".into() },
        host_calls: vec![HostCallRequirement { contract_id: SOURCE_CHECK_CALL.into(), target_kind: Some(kind.kind_id.clone()), maximum_in_flight: 1, maximum_input_bytes: bounds.request_bytes, maximum_output_bytes: bounds.report_bytes }],
        resource_requirements: vec![ResourceRequirement { class_id: SOURCE_CHECK_RESOURCE.into(), units: 1, protected_role: None, compute: None, content: None }],
        authority_requirements: vec![AuthorityRequirement { contract_id: SOURCE_CHECK_AUTHORITY.into(), host_call_contract_id: SOURCE_CHECK_CALL.into(), subject_kind: kind.kind_id.clone() }],
        limits: kind.limits.clone(),
    };
    let grant = AuthorityGrant {
        grant_id: "fixture/independent-source-read-grant".into(),
        contract_id: SOURCE_CHECK_AUTHORITY.into(),
        host_call_contract_id: SOURCE_CHECK_CALL.into(),
        subject_kind: kind.kind_id.clone(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    NativeSourceDefinition {
        kind,
        grant,
        base: BaseProviderEntry {
            base_id: "fixture/source-check-base".into(),
            provider_instance_id: "fixture/source-check-provider".into(),
            provider_generation: 1,
            implementation_id: "fixture/independent-source-read-owner".into(),
            mechanism_family: "fixture/prepared-analysis".into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![ResourceOffer {
                pool_id: "fixture/immutable-analyses".into(),
                class_id: SOURCE_CHECK_RESOURCE.into(),
                capacity_units: 1,
                compute: None,
                content: None,
            }],
        },
        resource_generation: ResourceGenerationId(
            "fixture/immutable-source-generation-set@1".into(),
        ),
        configuration: vec![],
        input_bytes: bounds.request_bytes,
        output_bytes: bounds.report_bytes,
        resource_bytes,
    }
}

struct Compiler {
    provider: PreparedSourceCheckProvider,
    _original_guard: crate::body_execution::ImagePreparedValue<Vec<u8>>,
    dropped: Arc<AtomicUsize>,
}
unsafe impl ImmediateNativeSource for Compiler {
    fn read(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, String> {
        self.provider.read(input, output)
    }
}
impl Drop for Compiler {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::AcqRel);
    }
}
#[test]
fn original_compiler_guard_and_worker_survive_together_on_the_same_counter() {
    let mut f = fixture();
    let source = r#"host compiler {
      schema = 1
      target = {architecture: "x86_64", machine: "computer", os: "linux"}
      base = {kind: "language/source-compiler", implementation: "std/bounded-source-check@1", initialization: {executable: "/unused-fixture", executable_sha256: "0000000000000000000000000000000000000000000000000000000000000000", maximum_executable_bytes: 536870912, address_space_bytes: 536870912, cpu_seconds: 10, deadline_milliseconds: 20000, maximum_startup_bytes: 16384, preparation_bytes: 570425344, retained_bytes: 1048576, request_bytes: 16384, reply_bytes: 65536}}
      base = {kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}
      capability = {kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}
      capability = {kind: "language/source-check", pinned_implementation: "std/bounded-source-check@1"}
      need = {id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 1, bytes: 512}
      need = {id: "source/results", class: "language/prepared-source-check@1", slots: 1, bytes: 1048576}
      need = {id: "source/memory", class: "conduit.resource/runtime-memory@1", slots: 570425344, bytes: 570425344}
      limits = {static_memory_bytes: 16777216, heap_arena_bytes: 603979776, queue_items: 32, buffered_bytes: 1048576, active_instances: 16, operation_slots: 16, timer_slots: 16, line_sessions: 16, evidence_items: 16}
    }"#;

    let compiler_config = parse_host_configuration_conduit(source).unwrap();
    let mut configuration = f.checked.configuration().clone();
    configuration.bases.extend(
        compiler_config
            .bases
            .into_iter()
            .filter(|b| b.kind == "language/source-compiler"),
    );
    configuration.resources.extend(
        compiler_config
            .resources
            .into_iter()
            .filter(|r| r.id.starts_with("source/")),
    );
    configuration.capabilities.extend(
        compiler_config
            .capabilities
            .into_iter()
            .filter(|c| c.kind == "language/source-check"),
    );
    configuration.limits.heap_arena_bytes = 603979776;
    f.checked = check_host_configuration(
        configuration,
        &f.catalog,
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    f.image = build_default_host_image(
        f.checked.profile().clone(),
        &f.catalog,
        &conduit_workspace_make::package_set(),
        &BuildInputs {
            source_identity: "fixture/original-compiler-and-ambient".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    let ambient_base = f
        .image
        .manifest
        .base_selections
        .iter()
        .find(|b| b.kind == AMBIENT_SOURCE_BASE_KIND)
        .unwrap();
    f.definition.base.base_id = ambient_base.id.as_str().into();
    let compiler_base = f
        .image
        .manifest
        .base_selections
        .iter()
        .find(|b| b.kind == "language/source-compiler")
        .unwrap()
        .clone();
    let Fixture {
        image,
        catalog,
        checked,
        config,
        definition,
    } = f;
    let compiler_dropped = Arc::new(AtomicUsize::new(0));
    let worker_dropped = Arc::new(AtomicUsize::new(0));
    let compiler_probe = compiler_dropped.clone();
    let worker_probe = worker_dropped.clone();
    let config_for_grant = config.clone();
    let host = unsafe {
        StdHost::from_selected_timer_and_immediate_and_worker_source_factories_image(
            config,
            &image,
            &catalog,
            &checked,
            ImmediateSourcePreparationBudget {
                preparation_bytes: 4 * 1024 * 1024,
                retained_bytes: 4 * 1024 * 1024,
            },
            None,
            None,
            move |_, scope| {
                // This actual caller guard is admitted before construction on the
                // same original counter, and moved into the actual compiler owner.
                let guard = scope.prepare_value(65568, 65568, || Ok(vec![0x51; 65536]))?;
                let bounds = SourceCheckPreparationBounds {
                    request_bytes: 16384,
                    report_bytes: 65536,
                    retained_bytes: 131072,
                };
                let schema = SourceCheckNativeSchema::prepare()
                    .map_err(|e| format!("fixture schema: {e:?}"))?;
                let kind = PreparedSourceChecker::kind_for_bounds(&schema, bounds)?;
                let reference = StdHost::new();
                let mut ad = reference.advertisement().clone();
                ad.host_id = config_for_grant.host_id;
                ad.boot_id = config_for_grant.boot_id;
                let mut d = definition_for(&ad, kind, bounds, 131072);
                d.base.base_id = compiler_base.id.as_str().into();
                d.base.mechanism_family = compiler_base.kind.as_str().into();
                d.base.implementation_id = compiler_base.driver.as_str().into();
                d.base.capabilities[0].implementation.implementation_id =
                    "std/bounded-source-check@1".into();
                d.base.resources[0].pool_id = "source/results".into();
                d.base.resources.push(ResourceOffer {
                    pool_id: "source/memory".into(),
                    class_id: RUNTIME_MEMORY_RESOURCE_CLASS.into(),
                    capacity_units: 570425344,
                    compute: None,
                    content: None,
                });
                Ok((
                    d,
                    ImmediateSourcePreparationBudget {
                        preparation_bytes: 570425344,
                        retained_bytes: 1048576,
                    },
                    move || {
                        let (mut source_owner, request) =
                            owner(include_str!("../../../../../plots/hello/main.conduit"), 7);
                        let prepared = PreparedSourceChecker::prepare(
                            &mut [SourceCheckPreparation {
                                owner: &mut source_owner,
                                request: &request,
                            }],
                            bounds,
                        )?;
                        Ok(Box::new(Compiler {
                            provider: prepared.into_provider(),
                            _original_guard: guard,
                            dropped: compiler_probe,
                        }) as Box<dyn ImmediateNativeSource>)
                    },
                ))
            },
            move |_, _| {
                Ok((
                    definition,
                    ImmediateSourcePreparationBudget {
                        preparation_bytes: AMBIENT_SOURCE_PROVIDER_PREPARATION_BYTES,
                        retained_bytes: RETAINED,
                    },
                    budget(),
                    move || {
                        Ok(Box::new(Provider {
                            inner: PreparedAmbientObservation::prepare(
                                "America/Los_Angeles",
                                "127.0.0.1:43210".parse().unwrap(),
                                offline_deck(),
                                10,
                            )?,
                            dropped: worker_probe,
                        })
                            as Box<dyn NativeSourceWorkerProvider>)
                    },
                ))
            },
        )
    }
    .unwrap();
    assert_eq!(host.native_source_authority_grants().count(), 2);
    assert!(host.native_source_worker_inspection().is_some());
    let owners = host
        .native_source
        .as_ref()
        .unwrap()
        .owners()
        .collect::<Vec<_>>();
    assert_eq!(owners.len(), 2);
    assert!(owners.iter().all(|o| o.has_image_backing()));
    assert_ne!(
        owners[0].definition.grant.grant_id,
        owners[1].definition.grant.grant_id
    );
    assert_eq!(compiler_dropped.load(Ordering::Acquire), 0);
    assert_eq!(worker_dropped.load(Ordering::Acquire), 0);
    let observed = host.kernel_resources.image_memory_observer();
    let retained = observed.load(Ordering::Acquire);
    assert!(
        retained
            >= 4 * 1024 * 1024
                + 65536
                + 1048576
                + RETAINED as usize
                + AMBIENT_SOURCE_WORKER_BACKING_BYTES as usize
    );
    drop(host);
    assert_eq!(compiler_dropped.load(Ordering::Acquire), 1);
    assert_eq!(worker_dropped.load(Ordering::Acquire), 1);
    assert_eq!(observed.load(Ordering::Acquire), 0);
}
