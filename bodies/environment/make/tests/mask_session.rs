//! Ordinary Mask reuse preserves Host request and output generations.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "support/mask.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_face as f;
use conduit_kernel::{self as k, scheduler::*, SignSink, ValueStorage};
#[test]
fn ordinary_mask_reuses_its_kernel_without_resetting_requests_or_discarding_evidence() {
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let host = fixture::host();
    let planned = source.plan(&host, 8192, &[]).unwrap();
    let face = fixture::face();
    let bytes = serde_json::to_vec(&face).unwrap();
    let shows: Vec<_> = (1..=4)
        .map(|sequence| {
            f::MaskShow::prepared(
                &planned,
                &face,
                c::bind_active_play(
                    &planned.plan.plan_id,
                    &host.host_id,
                    &host.boot_id,
                    sequence,
                ),
                "environment".into(),
                "fixture/display".into(),
                "fixture/prepared".into(),
            )
            .unwrap()
        })
        .collect();
    let cache = f::PreparedMaskPlanJson::prepare(&planned, 8192).unwrap();
    let expected: Vec<_> = shows
        .iter()
        .map(|show| serde_json::to_vec(show).unwrap())
        .collect();
    let encoders: Vec<_> = shows
        .iter()
        .map(|show| cache.bind(show, 8192).unwrap())
        .collect();
    let prepared =
        conduit_environment_runtime::mask_kernel::PreparedMaskKernel::prepare(&planned).unwrap();
    let local = local_kernel(&source, &planned).unwrap();
    let rich_parts =
        conduit_environment_runtime::mask_kernel::PreparedMaskKernel::prepare_parts(&planned)
            .unwrap();
    let local_parts = local_kernel_parts(&source, &planned).unwrap();
    assert_eq!(
        rich_parts.required_heap_bytes(),
        local_parts.required_heap_bytes()
    );
    assert!(
        local_parts.required_heap_bytes().unwrap()
            >= 16384 + core::mem::size_of::<conduit_environment_runtime::mask_kernel::Kernel>()
    );
    // The helper has dropped its verified Plan, lowering and Source material.
    let reserved =
        conduit_environment_runtime::mask_kernel::PreparedMaskValueBacking::prepare(8192, 8192)
            .unwrap();
    let staged_local = local_parts.finish_with_backing(reserved).unwrap();
    let staged_rich = rich_parts.finish().unwrap();
    assert_eq!(prepared.face, local.face);
    assert_eq!(prepared.show, local.show);
    assert_eq!(prepared.interaction, local.interaction);
    let mut traces = Vec::new();
    for prepared in [prepared, local, staged_rich, staged_local] {
        traces.extend(run_kernel(prepared, &encoders, &expected, &bytes));
    }
    let mut backing = k::FixedValueStore::<2, 8192>::new(16384).unwrap();
    let fixed = local_kernel_parts(&source, &planned)
        .unwrap()
        .finish_with_fixed_storage(&mut backing)
        .unwrap();
    traces.extend(run_kernel(fixed, &encoders, &expected, &bytes));
    assert_eq!(backing.used_items(), 0);
    for trace in traces.as_chunks::<4>().0.iter().skip(1) {
        assert_eq!(&traces[..4], trace);
    }
}

fn run_kernel<V: k::CanonicalEncodedValueStorage>(
    prepared: conduit_environment_runtime::mask_kernel::PreparedMaskKernel<V>,
    encoders: &[f::PreparedMaskShowJson<'_>],
    expected: &[Vec<u8>],
    bytes: &[u8],
) -> Vec<Vec<k::KernelEvent>> {
    let mut traces = Vec::new();
    use prepared_session::{OwnedValueBoundary as Boundary, PreparedKernelSession};
    let boundary = |fore: &conduit_plan_lowering::lowering::LoweredForePort| Boundary {
        endpoint: fore.endpoint,
        cord: fore.cord,
    };
    let face_fore = boundary(&prepared.face);
    let show_fore = boundary(&prepared.show);
    let interaction = boundary(&prepared.interaction);
    let mut session = PreparedKernelSession::new_with_owned_optional_value_outputs(
        prepared.kernel,
        &[face_fore, show_fore, interaction],
        &[interaction],
        12,
    )
    .unwrap_or_else(|(r, _)| panic!("{r:?}"));
    let fresh_log = || {
        k::FixedSignLog::<64, 12>::new_with_remote_storage(
            (64 * core::mem::size_of::<k::KernelEvent>()) as u32,
            12,
            k::remote_sign_storage_bytes(12).unwrap(),
        )
        .unwrap()
    };
    let mut evidence = [0; conduit_environment_runtime::display_evidence::MAXIMUM_BYTES];
    for (sequence, encoder) in encoders.iter().enumerate() {
        let new_log = fresh_log(); // cold reservation, admitted before Play
        let (_, memory) = allocation_probe::observe(|| {
            if sequence > 0 {
                let retired = session
                    .begin_next([(); 3], new_log)
                    .unwrap_or_else(|r| panic!("{:?}", r.reason));
                // Exact previous signs move to their caller; no silent clear.
                assert!(!retired.signs.is_empty());
                let length = conduit_environment_runtime::display_evidence::encode(
                    (sequence - 1) as u64,
                    retired.signs.events(),
                    &retired.signs,
                    &mut evidence,
                )
                .unwrap();
                let page =
                    conduit_environment_runtime::display_evidence::Page::parse(&evidence[..length])
                        .unwrap();
                assert_eq!(page.invocation(), (sequence - 1) as u64);
            }
            session
                .admit_owned_input_encoded(
                    face_fore.endpoint,
                    face_fore.cord,
                    sequence as u64,
                    8192,
                    |out| {
                        out[..bytes.len()].copy_from_slice(bytes);
                        Ok::<_, ()>(bytes.len())
                    },
                )
                .unwrap();
            session
                .close_owned_input(face_fore.endpoint, face_fore.cord)
                .unwrap();
            let mut drained = false;
            let mut emitted = false;
            for _ in 0..64 {
                let status = session.step().unwrap();
                if let Some(request) = session.next_host_request() {
                    assert_eq!(request.request, k::RequestId(sequence as u32));
                    assert_eq!(
                        session.kernel().host_value(request.input.value).unwrap(),
                        bytes
                    );
                    let value = session
                        .store_host_value_encoded(encoder.encoded_bytes(), |out| {
                            encoder.encode(out)
                        })
                        .unwrap();
                    session
                        .complete_host_call(
                            request.node,
                            request.request,
                            k::HostCallOutcome {
                                disposition: k::HostCallDisposition::Completed,
                                output: Some(k::BoundedValueRef::new(value, 8192).unwrap()),
                                failure: None,
                            },
                        )
                        .unwrap();
                }
                if !emitted {
                    if let Some(offer) = session
                        .owned_output(show_fore.endpoint, show_fore.cord)
                        .unwrap()
                    {
                        assert_eq!(offer.sequence, sequence as u64);
                        assert_eq!(
                            session.kernel().host_value(offer.value).unwrap(),
                            expected[sequence]
                        );
                        session
                            .accept_owned_output(offer.endpoint, offer.cord, offer.sequence)
                            .unwrap();
                        emitted = true;
                    }
                }
                if status == SchedulerStatus::Drained {
                    drained = true;
                    break;
                }
            }
            assert!(drained && emitted);
            assert_eq!(session.kernel().values().used_items(), 0);
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        traces.push(session.kernel().signs().events().collect::<Vec<_>>());
    }
    traces
}

fn verified(plan: &c::Plan) -> c::VerifiedPlanMetadata {
    let mut plan = plan.clone();
    let configurations = plan
        .fragments
        .iter_mut()
        .flat_map(|f| &mut f.placements)
        .map(|g| {
            (
                g.placement_id.clone(),
                core::mem::take(&mut g.configuration),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    c::verify_plan_metadata_with_configurations(plan, |gear| {
        Ok::<_, ()>(configurations[&gear.placement_id].clone())
    })
    .unwrap()
}
fn local_kernel(
    source: &conduit_environment_make::SevenSegmentMaskSource,
    planned: &f::PlannedMaskPlot,
) -> Result<conduit_environment_runtime::mask_kernel::PreparedMaskKernel, &'static str> {
    local_kernel_parts(source, planned)?.finish()
}
fn local_kernel_parts(
    source: &conduit_environment_make::SevenSegmentMaskSource,
    planned: &f::PlannedMaskPlot,
) -> Result<conduit_environment_runtime::mask_kernel::PreparedMaskKernelParts, &'static str> {
    let material = conduit_environment_make::SevenSegmentMaskMaterial::prepare(source).unwrap();
    // SAFETY: bytes were just encoded from this independently checked Source.
    let image = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
            material.planning_image(),
        )
    }
    .unwrap();
    let boundary = material.planning_boundary().unwrap();
    let verified = verified(&planned.plan);
    let lowered = conduit_plan_lowering::local_tables::VerifiedLocalLowering::prepare(
        &verified,
        &planned.plan.fragments[0].fragment_id,
    )
    .map_err(|_| "test-lowering")?;
    let host = fixture::host();
    conduit_environment_runtime::mask_kernel::PreparedMaskKernel::prepare_local_parts(
        conduit_environment_runtime::mask_kernel::LocalMaskInput {
            verified: &lowered,
            image: &image,
            boundary: &boundary,
            role: &planned.mask,
            scope: (&host.host_id, &host.boot_id, host.offer_generation),
        },
    )
}
#[test]
fn local_mask_refuses_wrong_role_source_configuration_fore_and_numeric_contracts() {
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let planned = source.plan(&fixture::host(), 8192, &[]).unwrap();
    for failure in [
        "source",
        "role",
        "configuration",
        "fore",
        "numeric",
        "semantic",
        "cord",
    ] {
        let mut wrong = planned.clone();
        match failure {
            "source" => wrong.mask.plot_identity.source_document_id = "foreign/source".into(),
            "role" => wrong.mask.face_input.gear_port_id = "foreign/face".into(),
            "configuration" => {
                wrong.plan.fragments[0].placements[0].configuration[0].value =
                    c::ConfigurationValue::Text("Foreign label".into())
            }
            "fore" => wrong.plan.fragments[0].fore_ports[0].front_port_id = "foreign/fore".into(),
            "numeric" => wrong.plan.fragments[0].fore_ports[0].byte_capacity = 8193,
            "semantic" => {
                wrong.plan.fragments[0].placements[0]
                    .semantic_contract
                    .configuration[0]
                    .default_value = c::ConfigurationValue::Text("Foreign default".into())
            }
            "cord" => wrong.plan.fragments[0].connections[0].source_port_id = "foreign/cord".into(),
            _ => unreachable!(),
        }
        wrong.plan = c::seal_plan_with_realization_backs_and_completion(
            c::PlotIdentity {
                source_document_id: wrong.plan.source_document_id.clone(),
                checked_plot_id: wrong.plan.checked_plot_id.clone(),
                expanded_plot_id: wrong.plan.expanded_plot_id.clone(),
            },
            wrong.plan.completion_policy,
            wrong.plan.realization_backs,
            wrong.plan.fragments,
        );
        assert!(c::verify_plan(&wrong.plan), "{failure}");
        assert!(local_kernel(&source, &wrong).is_err(), "{failure}");
        assert!(
            local_kernel_parts(&source, &wrong).is_err(),
            "staged {failure}"
        );
    }
}

#[test]
fn checked_mask_parts_defer_full_canonical_storage_until_finish() {
    use conduit_environment_runtime::mask_kernel::PreparedMaskKernel;
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let planned = source.plan(&fixture::host(), 8192, &[]).unwrap();
    let (rich, rich_memory) =
        allocation_probe::observe(|| PreparedMaskKernel::prepare(&planned).unwrap());
    let (parts, parts_memory) =
        allocation_probe::observe(|| PreparedMaskKernel::prepare_parts(&planned).unwrap());
    let (local, local_memory) =
        allocation_probe::observe(|| local_kernel_parts(&source, &planned).unwrap());
    assert!(rich_memory.retained_bytes() >= parts_memory.retained_bytes() + 16384);
    assert!(rich_memory.retained_bytes() >= local_memory.retained_bytes() + 16384);
    assert_eq!(parts.required_heap_bytes(), local.required_heap_bytes());
    // Both owned values outlive every input used to capture them.
    drop(planned);
    drop(source);
    let staged = parts.finish().unwrap();
    let staged_local = local.finish().unwrap();
    assert_eq!(rich.face, staged.face);
    assert_eq!(rich.show, staged.show);
    assert_eq!(rich.interaction, staged.interaction);
    assert_eq!(staged.face, staged_local.face);
    assert_eq!(staged.show, staged_local.show);
    assert_eq!(staged.interaction, staged_local.interaction);
}

#[test]
fn mask_value_backing_requires_exact_sealed_slot_capacities() {
    use conduit_environment_runtime::mask_kernel::{PreparedMaskKernel, PreparedMaskValueBacking};
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let planned = source.plan(&fixture::host(), 8192, &[]).unwrap();
    for (face, show) in [(4096, 8192), (8192, 4096)] {
        let parts = PreparedMaskKernel::prepare_parts(&planned).unwrap();
        let backing = PreparedMaskValueBacking::prepare(face, show).unwrap();
        assert!(matches!(
            parts.finish_with_backing(backing),
            Err("mask-values")
        ));
    }
    for (face, show) in [(0, 8192), (8192, 0), (8193, 8192), (8192, 8193)] {
        assert!(matches!(
            PreparedMaskValueBacking::prepare(face, show),
            Err("mask-values")
        ));
    }
}

#[test]
fn retained_fixed_construction_refuses_without_losing_original_storage_or_generations() {
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let planned = source.plan(&fixture::host(), 8192, &[]).unwrap();
    let parts = local_kernel_parts(&source, &planned).unwrap();
    let mut backing = Box::new(k::FixedValueStore::<2, 8192>::new(16384).unwrap());
    let address = &*backing as *const _ as usize;
    let previous = backing.store(&[7, 8, 9]).unwrap();
    let (parts, values, reason) = match parts.finish_with_fixed_storage_retained(&mut backing) {
        Err(refused) => refused,
        Ok(_) => panic!("occupied actual backing must refuse"),
    };
    assert_eq!(reason, "mask-values");
    assert_eq!(&*values as *const _ as usize, address);
    assert_eq!(values.get(previous).unwrap(), &[7, 8, 9]);
    values.release(previous).unwrap();
    let prepared = match parts.finish_with_fixed_storage_retained(values) {
        Ok(prepared) => prepared,
        Err(_) => panic!("same returned empty backing and checked tables"),
    };
    assert_eq!(&**prepared.kernel.values() as *const _ as usize, address);
    assert!(prepared.kernel.values().get(previous).is_err());
    let mut kernel = prepared.kernel;
    let mut reused = false;
    for _ in 0..2 {
        let fresh = kernel.store_host_value(&[10]).unwrap();
        if fresh.slot == previous.slot {
            assert!(fresh.generation > previous.generation);
            reused = true;
        }
    }
    assert!(reused);
    assert!(kernel.values().get(previous).is_err());
}
