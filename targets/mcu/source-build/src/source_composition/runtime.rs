// One Source-specialized checked Root composition, with board-owned providers.
use crate::runtime::{allocator_used, now_ms, remaining};
use crate::workset::{generated as source, renderer as display};
use conduit_environment_runtime::{
    publication::PublicationSequence, publication_face_custody::LocalPublicationFace,
    retained_publication_face::RetainedPublicationBacking,
};
use conduit_mcu_host_runtime::{
    execution::Policy,
    execution_factory::SoftwareInventory,
    execution_mask::ReadyMaskRouting,
    native_authority::{Issuers, Operations},
    root::{Request, Root},
};
use core::cell::RefCell;
use embassy_time::{with_timeout, Duration, Timer};
const W: usize = crate::equipment::WAIT_SLOTS;
mod recording {
    include!(concat!(env!("OUT_DIR"), "/source_play_recording.rs"));
}
pub async fn run<'s>(
    root: &mut Root<'s, { crate::config::MAX_ASSIGNMENT_BYTES }>,
    equipment: crate::equipment::Equipment,
    arena: conduit_mcu_host_runtime::image_memory::ImageMemoryLease<'_>,
    link: &mut crate::transport::Link,
) -> ! {
    let mut residents =
        crate::resident::inventory().expect("actual initialized resident inventory");
    let mut routing = ReadyMaskRouting::new(
        source::mask_layout::MASK_ROLES.expect("checked actual Mask roles"),
        display::RENDERER_FACE_BYTES,
        display::RENDERER_SHOW_BYTES,
    )
    .expect("actual initialized Mask routing");
    let clocks: &[(u16, u64, u64); W] = source::sampling::CLOCKS
        .try_into()
        .expect("checked Source clocks");
    let (sampling_values, parameters) = crate::workset::values(
        source::sampling_layout::CORD_VALUE_SLOT_CAPACITIES,
        source::sampling_layout::STEP_VALUE_SLOT_CAPACITIES,
        source::sampling_layout::HOST_CALL_VALUE_SLOT_CAPACITIES,
        clocks,
        remaining(),
    )
    .expect("funded canonical sampling storage");
    let (output_values, _) = crate::workset::values(
        source::output_layout::CORD_VALUE_SLOT_CAPACITIES,
        source::output_layout::STEP_VALUE_SLOT_CAPACITIES,
        source::output_layout::HOST_CALL_VALUE_SLOT_CAPACITIES,
        &[],
        remaining(),
    )
    .expect("funded canonical output storage");
    let definitions =
        source::sampling::definitions(&parameters).expect("checked software definitions");
    let recording_operation = recording::operation(&definitions, source::sampling::NATIVE_KINDS)
        .expect("independently compiled finite recording operation");
    let mut append_output =
        [0u8; conduit_resource_snapshot::native_append::APPEND_RESULT_BYTES as usize];
    let mut append_status = Some(
        conduit_mcu_host_runtime::execution_inspection::PreparedAppendStatus::prepare(
            &source::sampling_layout::fore(),
            source::sampling_layout::append_status_fore(),
        )
        .expect("exact installed recording status Fore"),
    );
    let maximum_events = definitions
        .iter()
        .flatten()
        .filter_map(|d| match d {
            conduit_mcu_host_runtime::execution_factory::SoftwareDefinition::Publication {
                maximum_events,
                ..
            } => Some(*maximum_events),
            _ => None,
        })
        .max()
        .expect("actual Source publication");
    let face = LocalPublicationFace::from_backing(
        RetainedPublicationBacking::prepare(maximum_events, display::RENDERER_SHOW_LIFETIME_MS)
            .expect("funded inert Face storage"),
    );
    let (publisher, reader) = face.claim().expect("exclusive local Face custody");
    let publisher = RefCell::new(Some(publisher));
    let sequence = PublicationSequence::prepare(u64::from(maximum_events));
    let clock_identities: [_; W] = core::array::from_fn(|slot| {
        let actual = equipment.waits[slot]
            .as_ref()
            .expect("actual wait slot")
            .native_identity();
        conduit_plan_lowering::resident_inventory::OtherNodeIdentity {
            node: clocks[slot].0,
            kind: conduit_core::AssignedIdentity::from_text(conduit_time::TICK_KIND),
            implementation: conduit_core::AssignedIdentity::from_text(
                actual.implementation_id.as_str(),
            ),
            artifact: conduit_core::AssignedIdentity::from_text(actual.artifact_id.as_str()),
        }
    });
    // SAFETY: definitions, identities and configuration hashes come from the
    // same independently checked Source/lowered layout. This executor is the
    // only allocator user during each synchronous inventory initialization.
    let mut software = unsafe {
        SoftwareInventory::initialize(
            definitions,
            source::sampling::identities(&clock_identities)
                .expect("actual Source clock identities"),
            &publisher,
            sequence.clone(),
            now_ms,
            allocator_used,
            source::sampling::CONFIGURATION,
        )
    }
    .expect("actual initialized sampling Backs");
    let mut output_software = unsafe {
        SoftwareInventory::initialize(
            source::output::definitions(&[]).expect("checked output definitions"),
            source::output::identities(&[]).expect("checked output identities"),
            &publisher,
            sequence,
            now_ms,
            allocator_used,
            source::output::CONFIGURATION,
        )
    }
    .expect("actual initialized output Backs");
    let operations = Operations::from_source(
        source::sampling::NATIVE_KINDS,
        source::mask::NATIVE_KINDS,
        source::output::NATIVE_KINDS,
        core::array::from_fn(|slot| Some(clocks[slot])),
        display::RENDERER_MAXIMUM_FRAMES,
        display::RENDERER_FACE_BYTES,
        display::RENDERER_SHOW_BYTES,
    )
    .expect("finite independently compiled operations");
    let mut issuers = Issuers::<W>::new();
    let mut response = [0u8; 4096];
    // Equipment remains owned here; only the shared whole-workset factory may
    // withdraw its providers after original admission and authenticated Start.
    let mut equipment = equipment;
    let mut sampling_values = Some(sampling_values);
    let mut output_values = Some(output_values);
    let mut reader = Some(reader);
    let mut optional =
        Some(conduit_kernel::scheduler::prepared_session::OptionalOutputStorage::<6>::prepare());
    // Cold schema preparation runs before deep group construction, then its
    // original owners are consumed and dropped before execution allocation.
    let mut contracts = Some(
        conduit_mcu_host_runtime::execution_group::NativeContracts::prepare_with_recording(
            source::sampling::NATIVE_KINDS
                .iter()
                .any(|(_, kind)| *kind == "resource/try-append-record"),
        )
        .expect("funded native contract preparation"),
    );
    let mut last_refusal = None;
    loop {
        let result = match root.receive_request(link).await {
            Ok(Request::Inspect) => {
                use conduit_mcu_host_runtime::transport::MessageTransport;
                use core::fmt::Write;
                let mut status = heapless::String::<384>::new();
                let _ = write!(status,
                    "ROOT_INSPECTION@1 unsigned phase=not-started current_member={} retained_input={} heap_used={} last_refusal={:?}",
                    root.has_current_member(), root.retained_input(), allocator_used(), last_refusal);
                link.send(status.as_bytes())
                    .await
                    .map_err(|_| conduit_mcu_host_runtime::root::Refusal::Carrier)
            }
            Ok(Request::Spawn) => root.spawn(link, &mut response).await,
            Ok(Request::Return) => {
                let policy = Policy {
                    preparation: crate::config::PREPARATION_DELEGATION.as_ref(),
                    execution: crate::config::EXECUTION_DELEGATION.as_ref(),
                    active_worksets: 0,
                    maximum_retained_bytes: arena.bytes(),
                };
                let authorized = root.return_context(now_ms()).and_then(|context| {
                    issuers
                        .authorize(
                            context,
                            policy,
                            equipment.pulse.as_ref().expect("ready pulse"),
                            equipment.display.as_ref().expect("ready display"),
                            &equipment.renderer,
                            &equipment.waits,
                            &operations,
                            remaining,
                        )
                        .map_err(|_| conduit_mcu_host_runtime::root::Refusal::Scope)?;
                    match (equipment.recording.selected(), recording_operation.as_ref()) {
                        (false, None) => Ok(()),
                        (true, Some(operation)) => equipment
                            .recording
                            .authorize(
                                context,
                                policy,
                                &mut issuers.recording,
                                operation,
                                remaining(),
                            )
                            .map_err(|_| conduit_mcu_host_runtime::root::Refusal::Scope),
                        _ => Err(conduit_mcu_host_runtime::root::Refusal::Scope),
                    }
                });
                match authorized {
                    Err(error) => Err(error),
                    Ok(()) => {
                        let native = [
                            crate::workset::native_images::PULSE_IMAGE,
                            crate::workset::native_images::DISPLAY_IMAGE,
                            crate::workset::native_images::WAIT_IMAGE,
                            crate::workset::native_images::RENDERER_IMAGE,
                        ];
                        let resident_images = residents.offers();
                        let images = resident_images
                            .into_iter()
                            .flatten()
                            .chain(native.into_iter())
                            .chain(crate::workset::native_images::APPEND_IMAGE)
                            .chain(source::sampling::OFFER_IMAGES.iter().copied())
                            .chain(source::mask::OFFER_IMAGES.iter().copied())
                            .chain(source::mask::MASK_ROUTING_IMAGES.iter().copied())
                            .chain(source::output::OFFER_IMAGES.iter().copied());
                        root.return_member(
                            link,
                            images,
                            || {
                                issuers.copy_grants(remaining()).map_err(|_| {
                                    conduit_mcu_host_runtime::member_return::Refusal::Scope
                                })
                            },
                            &mut response,
                        )
                        .await
                    }
                }
            }
            Ok(Request::Prepare) => {
                let challenge = crate::runtime::fresh_challenge();
                match with_timeout(
                    Duration::from_secs(10),
                    root.receive_preparation(link, challenge, now_ms),
                )
                .await
                {
                    Ok(result) => result,
                    Err(_) => {
                        root.clear_unretained_input();
                        Err(conduit_mcu_host_runtime::root::Refusal::Preparation)
                    }
                }
            }
            Err(error) => Err(error),
        };
        crate::runtime::observe_receipt(root.receipt_digest());
        if let Err(error) = result {
            // An idle carrier timeout must not erase the preceding transaction refusal.
            if error != conduit_mcu_host_runtime::root::Refusal::Carrier || last_refusal.is_none() {
                last_refusal = Some(error);
            }
            link.reset();
        } else if root.retained_input() {
            let mut refusal_detail = heapless::String::<192>::new();
            let prepared_outcome = {
                let future = async {
                    let parts = root.execution_parts().map_err(|_| "execution-custody")?;
                    let policy = Policy {
                        preparation: crate::config::PREPARATION_DELEGATION.as_ref(),
                        execution: crate::config::EXECUTION_DELEGATION.as_ref(),
                        active_worksets: 0,
                        // A logical complete-owner receipt is bounded separately
                        // from ASG representation. Physical backing is exclusively
                        // held by the original whole-arena lease.
                        maximum_retained_bytes: arena.bytes(),
                    };
                    let identity = conduit_core::AssignedIdentity::from_text;
                    let mut held_pools = heapless::Vec::<_, 6>::from_slice(&[
                        identity(crate::config::RESIDENT_WORK_POOL),
                        identity(
                            equipment
                                .pulse
                                .as_ref()
                                .ok_or("pulse-busy")?
                                .identity()
                                .resource_pool_id
                                .as_str(),
                        ),
                        identity(
                            equipment
                                .display
                                .as_ref()
                                .ok_or("display-busy")?
                                .native_identity()
                                .resource_pool_id
                                .as_str(),
                        ),
                        identity(
                            equipment.waits[0]
                                .as_ref()
                                .ok_or("wait-busy")?
                                .native_identity()
                                .resource_pool_id
                                .as_str(),
                        ),
                        identity(
                            equipment
                                .renderer
                                .native_identity()
                                .resource_pool_id
                                .as_str(),
                        ),
                    ])
                    .expect("five original held pools");
                    if let Some(pool) = equipment.recording.held_pool() {
                        held_pools
                            .push(identity(pool.as_str()))
                            .map_err(|_| "held-pools")?;
                    }
                    let context = conduit_body::NativePreparationContext {
                        bootstrap: parts.bootstrap,
                        member: parts.member,
                        actual: parts.resources.host(),
                        image_id: parts.configuration.image_id,
                        configuration_id: parts.configuration.configuration_id,
                        now_millis: now_ms(),
                    };
                    let prepared = crate::play::prepare(
                        &*parts.workspace,
                        context,
                        policy,
                        &mut residents,
                        &mut software,
                        &mut output_software,
                        &mut routing,
                        &mut equipment.pulse,
                        &mut equipment.display,
                        &mut equipment.waits,
                        equipment.recording.ready_slot(),
                        &mut issuers.recording,
                        recording_operation.as_ref(),
                        append_output.len(),
                        &mut issuers.pulse,
                        &mut issuers.write,
                        &mut issuers.waits,
                        &operations,
                        sampling_values.take().ok_or("sampling-values-busy")?,
                        output_values.take().ok_or("output-values-busy")?,
                        contracts.take().ok_or("native-contracts-busy")?,
                        &mut refusal_detail,
                    )
                    .map_err(|error| {
                        use core::fmt::Write;
                        let _ = write!(&mut refusal_detail, "{error:?}");
                        "complete-source-preparation"
                    })?;
                    Ok((
                        prepared,
                        &*parts.workspace,
                        parts.bootstrap,
                        parts.member,
                        parts.configuration,
                        parts.resources,
                        policy,
                        held_pools,
                    ))
                };
                let mut future = core::pin::pin!(future);
                conduit_mcu_host_runtime::poll_boundary::separate(future.as_mut()).await
            };
            // Successive polls retire preparation scratch before admission and
            // provider binding need their own stack. Custody moves once; no
            // command, resource ledger, or execution grant is reconstructed.
            let outcome = match prepared_outcome {
                Err(reason) => Err(reason),
                Ok((
                    prepared,
                    workspace,
                    bootstrap,
                    member,
                    configuration,
                    resources,
                    policy,
                    held_pools,
                )) => {
                    let future = async {
                        crate::play_service::start(
                            prepared,
                            workspace,
                            bootstrap,
                            member,
                            configuration,
                            resources,
                            policy,
                            &mut equipment.renderer,
                            &mut issuers.renderer,
                            &operations.renderer,
                            reader.take().ok_or("reader-busy")?,
                            &face,
                            &held_pools,
                            append_output.len() + core::mem::size_of::<conduit_mcu_host_runtime::execution_inspection::PreparedAppendStatus>(),
                        )
                        .map_err(|error| {
                            use core::fmt::Write;
                            let _ = write!(&mut refusal_detail, "{error:?}");
                            "complete-admission-start"
                        })
                    };
                    let mut future = core::pin::pin!(future);
                    conduit_mcu_host_runtime::poll_boundary::separate(future.as_mut()).await
                }
            };
            match outcome {
                Ok((started, scopes)) => {
                    let evidence = started.evidence();
                    let (((sample, sample_calls), mask, (output, output_calls)), renderer) =
                        started.into_kernels();
                    // Construct the continuing future only inside its own poll
                    // frame. Constructing it here reserves its large return
                    // scratch during unrelated cold preparation as well.
                    let future = async {
                        crate::play_run::run(
                            sample,
                            sample_calls,
                            mask,
                            output,
                            output_calls,
                            renderer,
                            optional.take().expect("original optional tracking"),
                            scopes,
                            evidence,
                            arena,
                            &face,
                            link,
                            &equipment.activity,
                            &mut append_output,
                            append_status
                                .take()
                                .expect("original prepared status custody"),
                        )
                        .await
                    };
                    let mut future = core::pin::pin!(future);
                    return conduit_mcu_host_runtime::poll_boundary::separate(future.as_mut())
                        .await;
                }
                Err(reason) => {
                    // No partial execution: retain the original signed input and
                    // any fenced reservations. Read-only refusal stays observable
                    // on the same carrier; recovery remains the original reboot.
                    loop {
                        core::hint::black_box(&arena);
                        link.connected().await;
                        let mut input = [0; 128];
                        while link.receive(&mut input).await.is_ok() {
                            #[derive(serde::Serialize)]
                            struct Status<'a> {
                                phase: &'a str,
                                reason: &'a str,
                                detail: &'a str,
                                heap_used_bytes: usize,
                                heap_peak_bytes: Option<usize>,
                                heap_limit_bytes: usize,
                            }
                            let mut response = [0u8; 512];
                            let count = serde_json_core::to_slice(
                                &Status {
                                    phase: "refused-before-play",
                                    reason,
                                    detail: refusal_detail.as_str(),
                                    heap_used_bytes: allocator_used(),
                                    heap_peak_bytes: crate::runtime::allocator_peak(),
                                    heap_limit_bytes: crate::config::HEAP_BYTES,
                                },
                                &mut response,
                            )
                            .expect("bounded static refusal");
                            if link.send(&response[..count]).await.is_err() {
                                break;
                            }
                        }
                        link.reset();
                        Timer::after_millis(10).await;
                    }
                }
            }
        }
        Timer::after_millis(10).await;
    }
}
