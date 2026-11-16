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
mod transactions {
    include!(concat!(env!("OUT_DIR"), "/source_runtime_transactions.rs"));
}
pub mod continuation {
    include!(concat!(env!("OUT_DIR"), "/source_runtime_continuation.rs"));
}
pub async fn run<'s>(
    root: &mut Root<'s, { crate::config::MAX_ASSIGNMENT_BYTES }>,
    mut equipment: crate::equipment::Equipment,
    arena: conduit_mcu_host_runtime::image_memory::ImageMemoryLease<'_>,
    link: &mut crate::transport::Link,
    installed: conduit_mcu_host_runtime::continuation_startup::Startup<'_>,
) -> ! {
    crate::runtime::stack_checkpoint(1);
    let mut residents =
        crate::resident::inventory().expect("actual initialized resident inventory");
    // These immutable flash descriptions survive exclusive initialized-Back loans.
    // They are not availability evidence or native authority.
    let return_images = residents.offers();
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
    #[cfg(record_service)]
    let mut recording_resource = None;
    let (maximum_events, recording_operation) = initialization::publication_bounds(&parameters);
    let mut append_output = [0u8; crate::play::RECORDING_OUTPUT_BYTES];
    let mut append_status = Some(
        conduit_mcu_host_runtime::execution_inspection::PreparedAppendStatus::prepare_selected(
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
    crate::runtime::stack_checkpoint(2);
    if let Some(installed) = installed.accepted() {
        let policy = Policy {
            preparation: crate::config::PREPARATION_DELEGATION.as_ref(),
            execution: crate::config::EXECUTION_DELEGATION.as_ref(),
            active_worksets: 0,
            maximum_retained_bytes: arena.bytes(),
        };
        let mut started = None;
        let mut scopes = None;
        let mut activity = None;
        let mut diagnostic = heapless::String::new();
        let outcome = continuation::prepare_into(
            root,
            installed,
            policy,
            continuation::Factory {
                residents: &mut residents,
                software,
                output_software,
                routing: &mut routing,
                equipment: &mut equipment,
                issuers: &mut issuers,
                operations: &operations,
                #[cfg(record_service)]
                recording_resource: &mut recording_resource,
                recording: recording_operation.as_ref(),
                sampling_values: sampling_values.take().expect("original sampling values"),
                output_values: output_values.take().expect("original output values"),
                contracts: &mut contracts,
                reader: reader.take().expect("original Reader"),
                face: &face,
                append_output_bytes: append_output.len(),
                diagnostic: &mut diagnostic,
            },
            &mut started,
            &mut scopes,
            &mut activity,
        )
        .await;
        match outcome {
            Ok(()) => {
                let started = started.take().expect("original continuation Started");
                let evidence = started.evidence();
                let ((sampling, mask, (output, output_calls)), renderer) = started.into_kernels();
                let (mut sample, mut sample_calls) =
                    sampling.into_local().expect("local-only continuation");
                return crate::play_run::run(
                    &mut *sample,
                    &mut sample_calls,
                    mask,
                    output,
                    output_calls,
                    renderer,
                    optional.take().expect("original optional tracking"),
                    scopes.take().expect("original Scopes"),
                    evidence,
                    arena,
                    &face,
                    crate::play_run::Carriage::Local {
                        link,
                        control: crate::play_run::ReturnControl::prepare(
                            root,
                            &issuers,
                            return_images,
                        ),
                    },
                    activity.take().expect("original native activity"),
                    &mut append_output,
                    append_status
                        .take()
                        .expect("original prepared status custody"),
                )
                .await;
            }
            Err(reason) => loop {
                core::hint::black_box(&arena);
                core::hint::black_box(installed);
                link.connected().await;
                use conduit_mcu_host_runtime::transport::MessageTransport;
                let mut input = [0; 128];
                while link.receive(&mut input).await.is_ok() {
                    let mut response = [0u8; 512];
                    let count =
                        continuation::refusal_response(reason, diagnostic.as_str(), &mut response);
                    let _ = link.send(&response[..count]).await;
                }
                link.reset();
                Timer::after_millis(10).await;
            },
        }
    }
    // Only successful execution consumes this original complete IMAGE lease.
    let mut arena = Some(arena);
    let mut last_refusal = None;
    loop {
        // Rejected media remains borrowed and immutable throughout recovery.
        core::hint::black_box(installed.prefix());
        let result = match root.receive_request(link).await {
            Ok(Request::InstallContinuation) => {
                transactions::install_continuation(
                    root,
                    link,
                    installed.accepted().map(|installed| &installed.acceptance),
                )
                .await
            }
            Ok(Request::Inspect) => {
                transactions::inspect(root, link, last_refusal, installed.refusal()).await
            }
            Ok(Request::Spawn) => root.spawn(link, &mut response).await,
            Ok(Request::Return) => {
                transactions::return_member(
                    root,
                    &mut equipment,
                    &mut issuers,
                    &operations,
                    &residents,
                    recording_operation.as_ref(),
                    arena.as_ref().expect("original retained IMAGE lease").bytes(),
                    link,
                    &mut response,
                )
                .await
            }
            Ok(Request::Prepare) => transactions::prepare(root, link).await,
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
            let arena_bytes = arena.as_ref().expect("original retained IMAGE lease").bytes();
            let reason = {
                // End the completed child future and all its original partial
                // preparation loans BEFORE reusing the same Root transport.
                let future = async {
                    include!(concat!(env!("OUT_DIR"), "/source_runtime_online_play.rs"))
                };
                let mut future = core::pin::pin!(future);
                conduit_mcu_host_runtime::poll_boundary::separate(future.as_mut()).await
            };
            let _ = conduit_mcu_host_runtime::preparation_refusal::send_once(
                link,
                &mut response,
                &conduit_mcu_host_runtime::preparation_refusal::Details {
                    reason,
                    detail: refusal_detail.as_str(),
                    heap_used_bytes: allocator_used(),
                    heap_peak_bytes: crate::runtime::allocator_peak(),
                    heap_limit_bytes: crate::config::HEAP_BYTES,
                },
            ).await;
            // Terminal refusal, not recovery/retry/release. Original signed
            // input, ledger, physical lease and semantic Face remain retained.
            loop {
                core::hint::black_box(&root);
                core::hint::black_box(&arena);
                core::hint::black_box(&face);
                core::hint::black_box(&equipment);
                core::hint::black_box(&installed);
                Timer::after_secs(1).await;
            }
        }
        Timer::after_millis(10).await;
    }
}
