// One Source-specialized checked Root composition, with board-owned providers.
use crate::runtime::{allocator_used, now_ms, remaining};
use crate::workset::{generated as source, renderer as display};
use conduit_environment_runtime::{
    publication::PublicationSequence, publication_face_custody::LocalPublicationFace,
    retained_publication_face::RetainedPublicationBacking,
};
use conduit_mcu_host_runtime::{
    execution::Policy,
    execution_mask::ReadyMaskRouting,
    native_authority::{Issuers, Operations},
    root::{Request, Root},
};
use core::cell::RefCell;
use embassy_time::{with_timeout, Duration, Timer};
const W: usize = crate::equipment::WAIT_SLOTS;
mod initialization {
    include!(concat!(env!("OUT_DIR"), "/source_play_initialization.rs"));
}
pub async fn run<'s>(
    root: &mut Root<'s, { crate::config::MAX_ASSIGNMENT_BYTES }>,
    equipment: crate::equipment::Equipment,
    arena: conduit_mcu_host_runtime::image_memory::ImageMemoryLease<'_>,
    link: &mut crate::transport::Link,
) -> ! {
    let mut residents =
        crate::resident::inventory().expect("actual initialized resident inventory");
    let mut routing = initialization::routing();
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
    let (maximum_events, recording_operation) = initialization::publication_bounds(&parameters);
    let mut append_output =
        [0u8; conduit_resource_snapshot::native_append::APPEND_RESULT_BYTES as usize];
    let mut append_status = Some(
        conduit_mcu_host_runtime::execution_inspection::PreparedAppendStatus::prepare(
            &source::sampling_layout::fore(),
            source::sampling_layout::append_status_fore(),
        )
        .expect("exact installed recording status Fore"),
    );
    let face = LocalPublicationFace::from_backing(
        RetainedPublicationBacking::prepare(maximum_events, display::RENDERER_SHOW_LIFETIME_MS)
            .expect("funded inert Face storage"),
    );
    let (publisher, reader) = face.claim().expect("exclusive local Face custody");
    let publisher = RefCell::new(Some(publisher));
    let sequence = PublicationSequence::prepare(u64::from(maximum_events));
    // Slots are the retained owners; cold constructors write directly into
    // them rather than return fat identity/Back arrays through this poll frame.
    let mut software_slot = None;
    let mut output_software_slot = None;
    initialization::sampling(
        &mut software_slot,
        &parameters,
        clocks,
        &equipment,
        &publisher,
        sequence.clone(),
    );
    initialization::output(&mut output_software_slot, &publisher, sequence);
    let software = software_slot.as_mut().expect("initialized sampling slot");
    let output_software = output_software_slot
        .as_mut()
        .expect("initialized output slot");
    let operations = initialization::operations(clocks);
    let mut issuers = initialization::issuers();
    let mut response = initialization::response_buffer();
    // Equipment remains owned here; only the shared whole-workset factory may
    // withdraw its providers after original admission and authenticated Start.
    let mut equipment = equipment;
    let mut sampling_values = Some(sampling_values);
    let mut output_values = Some(output_values);
    let mut reader = Some(reader);
    let mut optional = Some(initialization::optional_outputs());
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
            // This composition has no installed cold-prefix transaction yet.
            // Root also refuses the request before intake when policy is absent.
            Ok(Request::InstallContinuation) => {
                Err(conduit_mcu_host_runtime::root::Refusal::Preparation)
            }
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
            // Caller-owned affine slots live in the original future storage.
            // Poll boundaries return only status, never another fat workset copy.
            let mut prepared_slot = None;
            let prepared_status = {
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
                        context.into(),
                        policy,
                        &mut residents,
                        &mut *software,
                        &mut *output_software,
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
                    prepared_slot = Some((
                        prepared,
                        &*parts.workspace,
                        parts.bootstrap,
                        parts.member,
                        parts.configuration,
                        parts.resources,
                        policy,
                        held_pools,
                    ));
                    Ok::<(), &'static str>(())
                };
                let mut future = core::pin::pin!(future);
                conduit_mcu_host_runtime::poll_boundary::separate(future.as_mut()).await
            };
            // Successive polls retire preparation scratch before admission and
            // provider binding need their own stack. Custody moves once; no
            // command, resource ledger, or execution grant is reconstructed.
            let mut presentation_slot = None;
            let prepared_status = match prepared_status {
                Err(reason) => Err(reason),
                Ok(()) => {
                    let future = crate::play_service::prepare_online_in_slot(
                        &prepared_slot,
                        &mut presentation_slot,
                        &mut equipment.renderer,
                        &mut issuers.renderer,
                        &operations.renderer,
                        reader.take().expect("original unclaimed Reader"),
                        &face,
                    );
                    conduit_mcu_host_runtime::poll_boundary::separate(core::pin::pin!(future))
                        .await
                        .map_err(|error| {
                            use core::fmt::Write;
                            let _ = write!(&mut refusal_detail, "{error:?}");
                            "renderer-preparation"
                        })
                }
            };
            let mut started_slot = None;
            let mut scopes_slot = None;
            let outcome = match prepared_status {
                Err(reason) => Err(reason),
                Ok(()) => {
                    let future = async {
                        let (
                            prepared,
                            workspace,
                            bootstrap,
                            member,
                            configuration,
                            resources,
                            policy,
                            held_pools,
                        ) = prepared_slot.take().ok_or("prepared-slot-empty")?;
                        crate::play_service::start_prepared_into(
                            prepared,
                            workspace,
                            bootstrap,
                            member,
                            configuration,
                            resources,
                            policy,
                            presentation_slot.take().ok_or("presentation-slot-empty")?,
                            &face,
                            &held_pools,
                            append_output.len() + core::mem::size_of::<conduit_mcu_host_runtime::execution_inspection::PreparedAppendStatus>(),
                            &mut started_slot,
                            &mut scopes_slot,
                        )
                        .map_err(|error| {
                            use core::fmt::Write;
                            let _ = write!(&mut refusal_detail, "{error:?}");
                            "complete-admission-start"
                        })?;
                        Ok::<(), &'static str>(())
                    };
                    let mut future = core::pin::pin!(future);
                    conduit_mcu_host_runtime::poll_boundary::separate(future.as_mut()).await
                }
            };
            match outcome {
                Ok(()) => {
                    // Withdraw the Started payload only inside the continuing
                    // poll, so the parent never needs its by-value return scratch.
                    let future = async {
                        let started = started_slot.take().expect("original Started output slot");
                        let scopes = scopes_slot.take().expect("original Scopes output slot");
                        let evidence = started.evidence();
                        let (((sample, sample_calls), mask, (output, output_calls)), renderer) =
                            started.into_kernels();
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
