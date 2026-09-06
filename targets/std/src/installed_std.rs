mod alife_host;
mod alife_operations;
mod audio_play_operation;
mod bool_presentation;
mod calendar_proposal_codec;
pub(super) mod calendar_proposal_encoding;
mod calendar_proposal_operation;
mod calendar_provider_host;
mod calendar_provider_operation;
mod catalog;
pub(super) mod contract;
mod count_operations;
mod deadline_host;
mod external_websocket;
mod external_websocket_host;
mod facade;
mod factory;
mod final_normalized_pattern_operation;
mod flow_gate_operation;
mod flow_state_operations;
mod generate_text;
mod http;
mod http_host;
mod image_text_operation;
mod image_text_record_operation;
mod input_semantic_operations;
mod instrument_map_operation;
mod json_operations;
mod json_summary_operation;
mod kernel_preparation;
mod keyboard_input_host;
mod keyboard_input_operation;
mod layout_operations;
mod local_model_operation;
mod logic_operations;
mod math_host;
mod math_operations;
mod midi_input_operation;
mod midi_output_operation;
mod model_host;
mod operation;
mod operation_cancellation;
mod operation_capacity;
mod operation_kind;
mod pacing_operations;
mod pattern_comparison_operation;
mod preparation;
pub(super) use preparation::{
    lower_fragment_with_continuity, state_storage_profile, validate_retained_inputs,
};
mod retained_run;
#[cfg(test)]
pub(super) use retained_run::run_fragment;
pub(super) use retained_run::{InstalledRunHost, RunLifecycle};
mod presentation_composition;
mod presentation_construction_host;
mod pulse_observation_operation;
#[cfg(test)]
mod pulse_observation_sink;
mod quantity_mapping;
mod recurrence_codec;
mod recurrence_encoding;
mod recurrence_operation;
mod render_demand_operation;
pub(super) mod rhythm_compare_host;
mod rhythm_compare_operation;
mod robotics_effect;
mod robotics_operations;
mod sequence_normalization_operation;
mod state_select_operation;
mod structured_presentation_host;
mod structured_selector_operation;
mod structured_values_operation;
mod synth_operation;
mod synth_render;
mod template_storage_host;
mod template_storage_operation;
mod test_audio_source;
#[cfg(test)]
mod test_gate;
#[cfg(test)]
mod test_input_semantics;
#[cfg(test)]
mod test_json_codec;
#[cfg(any(test, feature = "local-model-proof"))]
pub(crate) mod test_local_model_io;
#[cfg(test)]
mod test_logic;
#[cfg(test)]
mod test_midi_source;
#[cfg(test)]
mod test_recurrence_sink;
#[cfg(test)]
mod test_scalar_flow;
#[cfg(test)]
pub(super) mod test_structured_selector;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod test_text_source;
#[cfg(test)]
mod test_timing_sink;
mod text_operations;
#[cfg(test)]
mod text_operations_tests;
mod tick_operations;
mod tick_presentation;
mod timed_button_attempt_host;
mod timed_button_attempt_operation;
mod timed_pattern_operation;
mod timing_configuration;
mod timing_operations;
mod toggle_operation;
mod typed_record_operation;
mod vector_search_host;
mod vector_search_operation;

pub(crate) use self::catalog::supports;
#[cfg(test)]
use self::contract::parse_tick_configuration;
use self::contract::{decode_tick, TICK_ENCODED_LEN};
use self::operation::InstalledOperation;
#[cfg(test)]
use self::tick_operations::{TEST_OBSERVER_IMPLEMENTATION, TICK_FACTORY};
use super::{
    RunControl, RunControlDisposition, RunControlReceipt, StdKernelExecutionReport, StdRunReport,
    TimerAdapter,
};
#[cfg(test)]
use conduit_core::present_host_operation_requirement;
use conduit_core::{
    bind_active_play, bind_sign, kind_id, wait_host_operation_requirement, CancellationReason,
    Observation, ObservationKind, PlanFragment, TerminalDisposition,
};
use conduit_kernel::scheduler::{
    FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationOutcome, HostedSignLog,
    HostedValueStore, SignSink, ValueStorage,
};
use conduit_plan_lowering::lowering::{
    KernelExecutionIdentityMap, FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
};
use std::io::Write;
use std::time::Duration;

const MAX_NODES: usize = 16;
const MAX_CORDS: usize = 16;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const MAX_QUEUE_SLOTS: usize = 64;
const ROUTE_SLOTS: usize = MAX_NODES * PORTS;
const ROUTE_TARGETS: usize = 64;

pub(crate) use facade::*;
const HOST_OPERATIONS_PER_NODE: u16 = 3;
const HOST_BINDING_SLOTS: usize = MAX_NODES * HOST_OPERATIONS_PER_NODE as usize;
const PENDING_REQUESTS: usize = MAX_NODES;

pub(in crate::installed_std) type InstalledScheduler = FixedScheduler<
    OperationDriver<InstalledOperation, PORTS>,
    HostedValueStore,
    HostedSignLog,
    MAX_NODES,
    MAX_CORDS,
    PORTS,
    MAX_QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

pub(super) use contract::every_offer;
pub(super) use contract::tick_offer;

pub(super) fn run_fragment_retaining<W: Write, T: TimerAdapter>(
    host: InstalledRunHost<'_, '_, '_>,
    fragment: &PlanFragment,
    play_sequence: u64,
    next_sign_sequence: &mut u64,
    _output: &mut W,
    timer: &mut T,
    lifecycle: RunLifecycle<'_>,
) -> Result<crate::state_value::RetainedStdRun, String> {
    let RunLifecycle { control, retained } = lifecycle;
    let InstalledRunHost {
        advertisement,
        playback,
        midi_input,
        midi_output,
        keyboard,
        mut local_model,
        mut vector_search,
        mut calendar,
    } = host;
    let lowered = preparation::lower_fragment_with_continuity(fragment, retained.is_some())?;
    let active_nodes = lowered.nodes.len();
    let active_cords = lowered.cords.len();
    if !supports(fragment)
        || active_nodes == 0
        || active_nodes > MAX_NODES
        || active_cords == 0
        || active_cords > MAX_CORDS
        || lowered.cord_value_slots as usize > MAX_QUEUE_SLOTS
        || lowered.routes.len() > ROUTE_SLOTS
        || lowered
            .routes
            .iter()
            .map(|route| route.targets.len())
            .sum::<usize>()
            > ROUTE_TARGETS
        || !lowered.remote_endpoints.is_empty()
        || lowered.host_operations.len() > HOST_BINDING_SLOTS
    {
        return Err("fragment exceeds the installed std kernel profile".to_string());
    }

    let mut value_items = 0_u16;
    let mut value_bytes = 0_u32;
    let mut request_capacity = 0_usize;
    let mut maximum_value_bytes = TICK_ENCODED_LEN;
    let mut sign_items = 32_u16;
    for placement in &fragment.placements {
        let budget = preparation::operation_budget(placement)?;
        value_items = value_items
            .checked_add(budget.value_items)
            .ok_or_else(|| "installed value item budget overflow".to_string())?;
        value_bytes = value_bytes
            .checked_add(budget.value_bytes)
            .ok_or_else(|| "installed value byte budget overflow".to_string())?;
        request_capacity = request_capacity
            .checked_add(budget.host_requests)
            .ok_or_else(|| "installed request budget overflow".to_string())?;
        sign_items = sign_items
            .checked_add(budget.sign_items)
            .ok_or_else(|| "installed sign item budget overflow".to_string())?;
        maximum_value_bytes = maximum_value_bytes.max(budget.maximum_value_bytes);
    }
    #[cfg(test)]
    if fragment
        .placements
        .iter()
        .any(|placement| placement.implementation_id.as_str() == TEST_OBSERVER_IMPLEMENTATION)
    {
        request_capacity = request_capacity
            .checked_mul(2)
            .ok_or_else(|| "fixture request budget overflow".to_string())?;
    }

    let mut values =
        HostedValueStore::new(value_items.max(1), maximum_value_bytes, value_bytes.max(1))
            .map_err(|error| format!("installed value store: {error:?}"))?;
    let active_play = bind_active_play(
        &fragment.plan_id,
        &advertisement.host_id,
        &advertisement.boot_id,
        play_sequence,
    );
    let drivers =
        preparation::prepare_operations(fragment, &lowered, &mut values, &active_play, retained)?;
    let driver_capacity_before = drivers
        .iter()
        .map(|driver| driver.operation().allocation_capacity())
        .sum::<usize>();
    let value_allocation_before = values.allocation_capacities();

    let kernel_tables = kernel_preparation::KernelTables::prepare(&[&lowered])?;
    let sign_bytes = u32::from(sign_items)
        .checked_mul(
            u32::try_from(core::mem::size_of::<conduit_kernel::KernelEvent>())
                .map_err(|_| "installed sign charge overflow".to_string())?,
        )
        .ok_or_else(|| "installed sign byte budget overflow".to_string())?;
    let sign = HostedSignLog::new(sign_items, sign_bytes)
        .map_err(|error| format!("installed sign store: {error:?}"))?;
    let mut external_listener = external_websocket_host::prepare(fragment)?;
    let mut http_host = http_host::InstalledHttpHost::prepare(fragment)?;
    let mut calendar_host = calendar_provider_host::CalendarProviderHost::prepare(fragment)?;
    let mut alife_host = alife_host::AlifeHost::prepare(fragment)?;
    let alife_capacity_before = alife_host.allocation_capacity();
    if let Some(listener) = &external_listener {
        writeln!(
            _output,
            "external-websocket-ready address={}",
            listener
                .local_addr()
                .map_err(|error| format!("read external WebSocket address: {error:?}"))?
        )
        .map_err(|error| error.to_string())?;
        _output.flush().map_err(|error| error.to_string())?;
    }
    if let Some(address) = http_host.listener_address()? {
        writeln!(_output, "http-server-ready address={address}")
            .map_err(|error| error.to_string())?;
        _output.flush().map_err(|error| error.to_string())?;
    }
    let mut scheduler = kernel_tables.install(drivers, values, sign)?;

    let presentation_capacity = fragment
        .placements
        .iter()
        .filter(|placement| {
            placement.implementation_id.as_str()
                == conduit_std_offers::STRUCTURED_PRESENTATION_STD_IMPLEMENTATION
        })
        .count();
    let mut execution_identity = KernelExecutionIdentityMap::new(
        &lowered.identity,
        &active_play,
        request_capacity,
        presentation_capacity,
        presentation_capacity.saturating_add(2),
    )
    .map_err(|error| format!("prepare std execution identity: {error:?}"))?;
    let mut requests = Vec::<HostOperationRequest>::with_capacity(request_capacity);
    let wait_contract_id = wait_host_operation_requirement().contract_id;
    let deadline_contract_id = conduit_core::HostOperationContractId::from(
        conduit_core::MONOTONIC_TIMER_HOST_OPERATION_CONTRACT,
    );
    let midi_input_contract_id =
        conduit_core::HostOperationContractId::from(conduit_std_offers::MUSIC_INPUT_MIDI_OPERATION);
    let keyboard_contract_id = conduit_core::HostOperationContractId::from(
        conduit_std_offers::NEXT_KEY_EVENT_HOST_OPERATION_CONTRACT,
    );
    let mut deadlines = deadline_host::InstalledDeadlineHost::<PENDING_REQUESTS>::new();
    let text_target_kind = kind_id("presentation/stdout-text");
    let graphics_presentation_target_kind = kind_id("presentation/graphics-scene");
    let tick_target_kind = kind_id(conduit_std_offers::TICK_PRESENTATION_TARGET);
    let count_target_kind = kind_id(conduit_std_offers::COUNT_PRESENTATION_TARGET);
    let bool_target_kind = kind_id(conduit_std_offers::BOOL_PRESENTATION_TARGET);
    let structured_presentation_target_kind =
        kind_id(conduit_semantic_catalog::STRUCTURED_PRESENTATION_TARGET);
    let upper_contract_id = conduit_core::HostOperationContractId::from(
        conduit_std_offers::TEXT_UPPER_HOST_OPERATION_CONTRACT,
    );
    let upper_target_kind = kind_id(conduit_std_offers::TEXT_UPPER_HOST_OPERATION_TARGET);
    let join_contract_id = conduit_core::HostOperationContractId::from(
        conduit_std_offers::TEXT_JOIN_HOST_OPERATION_CONTRACT,
    );
    let join_target_kind = kind_id(conduit_std_offers::TEXT_JOIN_HOST_OPERATION_TARGET);
    let gate_bool_contract_id = conduit_core::HostOperationContractId::from(
        conduit_std_offers::FLOW_GATE_BOOL_HOST_OPERATION_CONTRACT,
    );
    let gate_bool_target_kind = kind_id(conduit_std_offers::FLOW_GATE_BOOL_HOST_OPERATION_TARGET);
    let keymap_contract_id =
        conduit_core::HostOperationContractId::from(conduit_std_offers::KEYMAP_HOST_OPERATION);
    let keymap_target_kind = kind_id(conduit_std_offers::KEYMAP_HOST_TARGET);
    let chords_contract_id =
        conduit_core::HostOperationContractId::from(conduit_std_offers::CHORDS_HOST_OPERATION);
    let chords_target_kind = kind_id(conduit_std_offers::CHORDS_HOST_TARGET);
    let mut math_host = math_host::MathHost::prepare(fragment)?;
    let presentation_construction =
        presentation_construction_host::PresentationConstructionHost::prepare();
    let mut uppercase_buffer = Vec::with_capacity(contract::MAX_TEXT_BYTES as usize);
    let mut input_keymaps = [conduit_human::ConduitIntlKeymap::new(); MAX_NODES];
    let mut external_output =
        Vec::with_capacity(conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES as usize + 1);
    let mut http_output =
        Vec::with_capacity(conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES as usize);
    let mut json_host = json_operations::JsonHost::prepare(fragment);
    let mut sequence_normalization_host =
        sequence_normalization_operation::SequenceNormalizationHost::prepare();
    let mut timed_pattern_host = timed_pattern_operation::TimedPatternHost::prepare();
    let mut timed_button_attempt_hosts = fragment
        .placements
        .iter()
        .map(|placement| {
            if placement.implementation_id.as_str()
                == conduit_std_offers::TIMED_BUTTON_ATTEMPT_STD_IMPLEMENTATION
            {
                timed_button_attempt_operation::host_maximum(placement).map(|maximum| {
                    Some(timed_button_attempt_host::TimedButtonAttemptHost::prepare(
                        maximum,
                    ))
                })
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut structured_selector_hosts = structured_selector_operation::prepare_hosts(fragment)?;
    let mut image_text_hosts = image_text_operation::prepare_hosts(fragment);
    let mut image_text_record_hosts = image_text_record_operation::prepare_hosts(fragment);
    let mut typed_record_hosts = typed_record_operation::prepare_hosts(fragment);
    let mut structured_presentation_host =
        structured_presentation_host::StructuredPresentationHost::prepare(
            fragment,
            &lowered.identity,
        )?;
    let mut rhythm_compare_hosts = fragment
        .placements
        .iter()
        .map(|placement| {
            if placement.implementation_id.as_str()
                == conduit_std_offers::RHYTHM_COMPARE_STD_IMPLEMENTATION
            {
                rhythm_compare_host::RhythmCompareHost::from_placement(placement).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut pattern_comparison_hosts = fragment
        .placements
        .iter()
        .map(|placement| {
            if placement.implementation_id.as_str()
                == conduit_std_offers::COMPARE_PATTERN_STD_IMPLEMENTATION
            {
                pattern_comparison_operation::PatternComparisonHost::from_placement(placement)
                    .map(Some)
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut template_storage_hosts = fragment
        .placements
        .iter()
        .map(|placement| {
            (placement.implementation_id.as_str()
                == conduit_std_offers::TEMPLATE_STORAGE_STD_IMPLEMENTATION)
                .then(template_storage_host::TemplateStorageHost::prepare)
        })
        .collect::<Vec<_>>();
    let mut generate_text_output =
        Vec::with_capacity(conduit_ai::MAXIMUM_OUTPUT_TOKENS as usize * 4);
    let mut vector_search_output =
        Vec::with_capacity(conduit_ai::MAXIMUM_VECTOR_SEARCH_OUTPUT_BYTES as usize);
    let mut synth_output = Vec::with_capacity(synth_operation::PCM_BLOCK_BYTES as usize);
    let mut synth_states = fragment
        .placements
        .iter()
        .map(|placement| {
            if placement.implementation_id.as_str()
                == conduit_synth::REFERENCE_SYNTH_IMPLEMENTATION_ID
            {
                synth_operation::InstalledSynthState::from_placement(placement).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    if synth_states.iter().any(Option::is_some) {
        let clock_origin_micros = timer.monotonic_now_micros().ok_or_else(|| {
            "installed music/synth requires the admitted Host/Boot monotonic-microsecond basis"
                .to_string()
        })?;
        for state in synth_states.iter_mut().flatten() {
            state.set_clock_origin(clock_origin_micros);
        }
    }
    let mut playback_sessions = fragment
        .placements
        .iter()
        .map(|placement| {
            if placement.implementation_id.as_str()
                == conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION
            {
                audio_play_operation::prepare_session(placement, playback).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut midi_input_sessions = fragment
        .placements
        .iter()
        .map(|placement| {
            if placement.implementation_id.as_str()
                == conduit_std_offers::MUSIC_INPUT_MIDI_IMPLEMENTATION
            {
                midi_input_operation::prepare_session(placement, midi_input).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut midi_input_requests = vec![None; active_nodes];
    let mut keyboard_host = keyboard_input_host::KeyboardInputHost::new(keyboard);
    let mut midi_output_sessions = fragment
        .placements
        .iter()
        .map(|placement| {
            if placement.implementation_id.as_str()
                == conduit_std_offers::MUSIC_PLAY_MIDI_IMPLEMENTATION
            {
                midi_output_operation::prepare_session(placement, midi_output).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut midi_output_adapters = fragment
        .placements
        .iter()
        .map(|placement| {
            if placement.implementation_id.as_str()
                == conduit_std_offers::MUSIC_PLAY_MIDI_IMPLEMENTATION
            {
                midi_output_operation::prepare_adapter().map(Some)
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    #[cfg(test)]
    let mut observed_ticks = Vec::with_capacity(request_capacity / 2);
    #[cfg(test)]
    let observer_contract_id = present_host_operation_requirement(
        kind_id("conduit-test/tick-observation"),
        TICK_ENCODED_LEN,
    )
    .contract_id;
    #[cfg(test)]
    let observer_target_kind = kind_id("conduit-test/tick-observation");
    #[cfg(test)]
    let play_start_probe = crate::allocation_probe::begin();
    let mut accepted_stop = None;
    let terminal_disposition = loop {
        if accepted_stop.is_none() {
            if let Some(request_id) = control.requested_stop() {
                scheduler
                    .cancel()
                    .map_err(|error| format!("cancel installed kernel: {error:?}"))?;
                deadlines.clear();
                accepted_stop = Some(request_id);
            }
        }
        while let Some(cancellation) = scheduler.next_host_cancellation() {
            let cancelled_operation = lowered
                .host_operations
                .iter()
                .find(|operation| {
                    operation.node == cancellation.node
                        && operation.operation == cancellation.operation
                })
                .ok_or_else(|| "cancelled host request has no lowered identity".to_string())?;
            if cancelled_operation.contract_id.as_str() == audio_play_operation::HOST_OPERATION {
                let session = playback_sessions
                    .get_mut(usize::from(cancellation.node.0))
                    .and_then(Option::as_mut)
                    .ok_or_else(|| "cancelled audio/play has no admitted session".to_string())?;
                session
                    .stop()
                    .map_err(|error| format!("stop cancelled audio/play: {error:?}"))?;
            } else if matches!(
                cancelled_operation.contract_id.as_str(),
                http::CLIENT_OPERATION
                    | http::SERVER_ACCEPT_OPERATION
                    | http::SERVER_RESPOND_OPERATION
            ) {
                http_host.cancel();
            } else if matches!(
                cancelled_operation.contract_id.as_str(),
                conduit_std_offers::MUSIC_PLAY_MIDI_NOTE_OPERATION
                    | conduit_std_offers::MUSIC_PLAY_MIDI_CONTROL_OPERATION
            ) {
                let session = midi_output_sessions
                    .get_mut(usize::from(cancellation.node.0))
                    .and_then(Option::as_mut)
                    .ok_or_else(|| "cancelled MIDI output has no admitted session".to_string())?;
                session
                    .stop()
                    .map_err(|error| format!("stop cancelled MIDI output: {error:?}"))?;
            } else if cancelled_operation.contract_id == midi_input_contract_id {
                let node = usize::from(cancellation.node.0);
                let session = midi_input_sessions
                    .get_mut(node)
                    .and_then(Option::as_mut)
                    .ok_or_else(|| "cancelled MIDI input has no admitted session".to_string())?;
                session.cancel();
                midi_input_requests[node] = None;
            } else if cancelled_operation.contract_id == keyboard_contract_id {
                keyboard_host.cancel();
            } else if cancelled_operation.contract_id.as_str()
                == conduit_ai::VECTOR_SEARCH_OPERATION
            {
                if let Some(adapter) = &mut vector_search {
                    adapter.cancel();
                }
            } else {
                deadlines.cancel(cancellation, &mut scheduler)?;
            }
        }
        while let Some(request) = scheduler.next_host_request() {
            let input = scheduler
                .host_value(request.input.value)
                .map_err(|error| format!("read std host input: {error:?}"))?;
            let lowered_operation = lowered
                .host_operations
                .iter()
                .find(|operation| {
                    operation.node == request.node && operation.operation == request.operation
                })
                .ok_or_else(|| "host request has no lowered contract identity".to_string())?;
            let contract = &lowered_operation.contract_id;
            if contract.as_str() == conduit_std_offers::TYPED_RECORD_FRAME_HOST_OPERATION {
                let completion = typed_record_hosts
                    .get_mut(usize::from(request.node.0))
                    .and_then(Option::as_mut)
                    .ok_or_else(|| "typed-record frame request has no admitted host".to_string())?
                    .execute(input);
                let (disposition, output, failure) = match completion {
                    Ok(encoded) => {
                        let value = scheduler
                            .store_host_value(encoded)
                            .map_err(|error| format!("store framed typed record: {error:?}"))?;
                        let output = BoundedValueRef::new(
                            value,
                            lowered_operation.binding.maximum_output_bytes,
                        )
                        .map_err(|error| format!("bound framed typed record: {error:?}"))?;
                        (HostOperationDisposition::Completed, Some(output), None)
                    }
                    Err(refusal) => (
                        HostOperationDisposition::Failed,
                        None,
                        Some(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostOperationFailed,
                            detail: refusal as u16,
                        }),
                    ),
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure,
                        },
                    )
                    .map_err(|error| format!("complete typed-record frame: {error:?}"))?;
                continue;
            }
            if contract.as_str() == conduit_std_offers::IMAGE_TEXT_RECORD_OPERATION {
                let encoded = image_text_record_hosts
                    .get_mut(usize::from(request.node.0))
                    .and_then(Option::as_mut)
                    .ok_or_else(|| "image-text record request has no admitted host".to_string())?
                    .execute(input);
                let (disposition, output, failure) = match encoded {
                    Ok(encoded) => {
                        let value = scheduler
                            .store_host_value(encoded)
                            .map_err(|error| format!("store typed image-text record: {error:?}"))?;
                        let output = BoundedValueRef::new(
                            value,
                            lowered_operation.binding.maximum_output_bytes,
                        )
                        .map_err(|error| format!("bound typed image-text record: {error:?}"))?;
                        (HostOperationDisposition::Completed, Some(output), None)
                    }
                    Err(_) => (
                        HostOperationDisposition::Failed,
                        None,
                        Some(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostOperationFailed,
                            detail: 1,
                        }),
                    ),
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure,
                        },
                    )
                    .map_err(|error| format!("complete image-text record operation: {error:?}"))?;
                continue;
            }
            if matches!(
                contract.as_str(),
                conduit_std_offers::IMAGE_TEXT_IMAGE_OPERATION
                    | conduit_std_offers::IMAGE_TEXT_CAPTION_OPERATION
            ) {
                let completion = image_text_hosts
                    .get_mut(usize::from(request.node.0))
                    .and_then(Option::as_mut)
                    .ok_or_else(|| "image-text request has no admitted host".to_string())?
                    .execute(contract.as_str(), input);
                let (disposition, output, failure) = match completion {
                    Ok(encoded) => {
                        let output = encoded
                            .map(|encoded| scheduler.store_host_value(encoded))
                            .transpose()
                            .map_err(|error| format!("store image-text output: {error:?}"))?
                            .map(|value| {
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                            })
                            .transpose()
                            .map_err(|error| format!("bound image-text output: {error:?}"))?;
                        (HostOperationDisposition::Completed, output, None)
                    }
                    Err(_) => (
                        HostOperationDisposition::Failed,
                        None,
                        Some(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostOperationFailed,
                            detail: 1,
                        }),
                    ),
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure,
                        },
                    )
                    .map_err(|error| format!("complete image-text operation: {error:?}"))?;
                continue;
            }
            if json_operations::matches(contract.as_str()) {
                let completion =
                    json_host.execute(usize::from(request.node.0), contract.as_str(), input);
                let (disposition, output, failure) = match completion {
                    Ok(encoded) => {
                        let value = scheduler
                            .store_host_value(encoded)
                            .map_err(|error| format!("store bounded JSON output: {error:?}"))?;
                        (
                            HostOperationDisposition::Completed,
                            Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| format!("bound JSON output: {error:?}"))?,
                            ),
                            None,
                        )
                    }
                    Err(refusal) => (
                        HostOperationDisposition::Failed,
                        None,
                        Some(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostOperationFailed,
                            detail: refusal,
                        }),
                    ),
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure,
                        },
                    )
                    .map_err(|error| format!("complete bounded JSON operation: {error:?}"))?;
                continue;
            }
            if contract.as_str() == conduit_std_offers::TEMPLATE_STORAGE_HOST_OPERATION {
                let completion = template_storage_hosts
                    .get_mut(usize::from(request.node.0))
                    .and_then(Option::as_mut)
                    .ok_or_else(|| "template request has no admitted storage host".to_string())?
                    .execute(input);
                let (disposition, output, failure) = match completion {
                    Ok(encoded) => {
                        let value = scheduler
                            .store_host_value(encoded)
                            .map_err(|error| format!("store bounded template result: {error:?}"))?;
                        (
                            HostOperationDisposition::Completed,
                            Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| format!("bound template result: {error:?}"))?,
                            ),
                            None,
                        )
                    }
                    Err(refusal) => (
                        HostOperationDisposition::Failed,
                        None,
                        Some(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostOperationFailed,
                            detail: template_storage_operation::refusal_detail(refusal),
                        }),
                    ),
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure,
                        },
                    )
                    .map_err(|error| format!("complete bounded template storage: {error:?}"))?;
                continue;
            }
            if contract.as_str() == conduit_std_offers::TIMED_BUTTON_ATTEMPT_OBSERVE_HOST_OPERATION
            {
                let now_micros = timer.monotonic_now_micros().ok_or_else(|| {
                    "admitted pressed-button monotonic-microsecond Base is unavailable".to_string()
                })?;
                let completion = timed_button_attempt_hosts
                    .get_mut(usize::from(request.node.0))
                    .and_then(Option::as_mut)
                    .ok_or_else(|| {
                        "pressed-button request has no admitted observation host".to_string()
                    })?
                    .observe(input, now_micros);
                let (disposition, output, failure) = match completion {
                    Ok(timed_button_attempt_host::Observation::Released) => {
                        (HostOperationDisposition::Completed, None, None)
                    }
                    Ok(timed_button_attempt_host::Observation::Pressed) => {
                        let value = scheduler.store_host_value(&[0]).map_err(|error| {
                            format!("store bounded pressed-button marker: {error:?}")
                        })?;
                        (
                            HostOperationDisposition::Completed,
                            Some(BoundedValueRef::new(value, 1).map_err(|error| {
                                format!("bound pressed-button marker: {error:?}")
                            })?),
                            None,
                        )
                    }
                    Ok(timed_button_attempt_host::Observation::Complete(encoded)) => {
                        let value = scheduler.store_host_value(encoded).map_err(|error| {
                            format!("store bounded pressed-button attempt: {error:?}")
                        })?;
                        (
                            HostOperationDisposition::Completed,
                            Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| {
                                    format!("bound pressed-button attempt: {error:?}")
                                })?,
                            ),
                            None,
                        )
                    }
                    Err(refusal) => (
                        HostOperationDisposition::Failed,
                        None,
                        Some(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostOperationFailed,
                            detail: timed_button_attempt_operation::refusal_detail(refusal),
                        }),
                    ),
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure,
                        },
                    )
                    .map_err(|error| {
                        format!("complete bounded pressed-button observation: {error:?}")
                    })?;
                continue;
            }
            if contract.as_str() == conduit_std_offers::STRUCTURED_SELECTOR_HOST_OPERATION {
                let completion = structured_selector_hosts
                    .get_mut(usize::from(request.node.0))
                    .and_then(Option::as_mut)
                    .ok_or_else(|| "structured selector request has no admitted host".to_string())?
                    .execute(input);
                let (disposition, output, failure) = match completion {
                    Ok(Some(encoded)) => {
                        let value = scheduler.store_host_value(encoded).map_err(|error| {
                            format!("store bounded structured selector output: {error:?}")
                        })?;
                        (
                            HostOperationDisposition::Completed,
                            Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| {
                                    format!("bound structured selector output: {error:?}")
                                })?,
                            ),
                            None,
                        )
                    }
                    Ok(None) => (HostOperationDisposition::Completed, None, None),
                    Err(refusal) => (
                        HostOperationDisposition::Failed,
                        None,
                        Some(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostOperationFailed,
                            detail: structured_selector_operation::refusal_detail(&refusal),
                        }),
                    ),
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure,
                        },
                    )
                    .map_err(|error| {
                        format!("complete bounded structured selector operation: {error:?}")
                    })?;
                continue;
            }
            if contract.as_str() == conduit_std_offers::ORDERED_EVENT_INTERVALS_HOST_OPERATION {
                let completion = timed_pattern_host.execute(input);
                let (disposition, output, failure) = match completion {
                    Ok(encoded) => {
                        let value = scheduler.store_host_value(encoded).map_err(|error| {
                            format!("store bounded timed-pattern output: {error:?}")
                        })?;
                        (
                            HostOperationDisposition::Completed,
                            Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| {
                                    format!("bound timed-pattern output: {error:?}")
                                })?,
                            ),
                            None,
                        )
                    }
                    Err(refusal) => (
                        HostOperationDisposition::Failed,
                        None,
                        Some(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostOperationFailed,
                            detail: timed_pattern_operation::refusal_detail(&refusal),
                        }),
                    ),
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure,
                        },
                    )
                    .map_err(|error| {
                        format!("complete bounded timed-pattern operation: {error:?}")
                    })?;
                continue;
            }
            if contract.as_str() == conduit_std_offers::NORMALIZE_SEQUENCE_HOST_OPERATION {
                let completion = sequence_normalization_host.execute(input);
                let (disposition, output, failure) = match completion {
                    Ok(encoded) => {
                        let value = scheduler.store_host_value(encoded).map_err(|error| {
                            format!("store bounded normalized sequence: {error:?}")
                        })?;
                        (
                            HostOperationDisposition::Completed,
                            Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| format!("bound normalized sequence: {error:?}"))?,
                            ),
                            None,
                        )
                    }
                    Err(refusal) => (
                        HostOperationDisposition::Failed,
                        None,
                        Some(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostOperationFailed,
                            detail: sequence_normalization_operation::refusal_detail(&refusal),
                        }),
                    ),
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure,
                        },
                    )
                    .map_err(|error| {
                        format!("complete bounded normalization operation: {error:?}")
                    })?;
                continue;
            }
            if let Some(calendar_operation) =
                crate::hosted_calendar::CalendarHostedOperation::from_contract(contract.as_str())
            {
                let completion = calendar_host.execute(
                    usize::from(request.node.0),
                    calendar_operation,
                    input,
                    calendar.as_deref_mut(),
                );
                let (disposition, output, failure) = match completion {
                    Ok(encoded) => {
                        let value = scheduler.store_host_value(&encoded).map_err(|error| {
                            format!("store calendar provider output: {error:?}")
                        })?;
                        (
                            HostOperationDisposition::Completed,
                            Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| {
                                    format!("bound calendar provider output: {error:?}")
                                })?,
                            ),
                            None,
                        )
                    }
                    Err(refusal) => (
                        HostOperationDisposition::Failed,
                        None,
                        Some(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostOperationFailed,
                            detail: calendar_provider_host::refusal_detail(refusal),
                        }),
                    ),
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure,
                        },
                    )
                    .map_err(|error| {
                        format!("complete calendar provider host operation: {error:?}")
                    })?;
                continue;
            }
            if matches!(
                contract.as_str(),
                conduit_std_offers::RHYTHM_PERFORMANCE_HOST_OPERATION
                    | conduit_std_offers::RHYTHM_REFERENCE_HOST_OPERATION
                    | conduit_std_offers::RHYTHM_DRAIN_HOST_OPERATION
            ) {
                let completion = rhythm_compare_hosts
                    .get_mut(usize::from(request.node.0))
                    .and_then(Option::as_mut)
                    .ok_or_else(|| "rhythm request has no admitted comparison host".to_string())?
                    .execute(contract.as_str(), input);
                let (disposition, output, failure) = match completion {
                    Ok(Some(encoded)) => {
                        let value = scheduler
                            .store_host_value(encoded)
                            .map_err(|error| format!("store bounded rhythm feedback: {error:?}"))?;
                        (
                            HostOperationDisposition::Completed,
                            Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| {
                                    format!("bound structured rhythm feedback: {error:?}")
                                })?,
                            ),
                            None,
                        )
                    }
                    Ok(None) => (HostOperationDisposition::Completed, None, None),
                    Err(refusal) => (
                        HostOperationDisposition::Failed,
                        None,
                        Some(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostOperationFailed,
                            detail: refusal as u16,
                        }),
                    ),
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure,
                        },
                    )
                    .map_err(|error| format!("complete bounded rhythm comparison: {error:?}"))?;
                continue;
            }
            if matches!(
                contract.as_str(),
                conduit_std_offers::COMPARE_PATTERN_CANDIDATE_OPERATION
                    | conduit_std_offers::COMPARE_PATTERN_TEMPLATE_OPERATION
            ) {
                let completion = pattern_comparison_hosts
                    .get_mut(usize::from(request.node.0))
                    .and_then(Option::as_mut)
                    .ok_or_else(|| "pattern request has no admitted comparison host".to_string())?
                    .execute(contract.as_str(), input);
                let (disposition, output, failure) = match completion {
                    Ok(Some(encoded)) => {
                        let value = scheduler.store_host_value(encoded).map_err(|error| {
                            format!("store bounded pattern comparison: {error:?}")
                        })?;
                        (
                            HostOperationDisposition::Completed,
                            Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| format!("bound pattern comparison: {error:?}"))?,
                            ),
                            None,
                        )
                    }
                    Ok(None) => (HostOperationDisposition::Completed, None, None),
                    Err(refusal) => (
                        HostOperationDisposition::Failed,
                        None,
                        Some(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostOperationFailed,
                            detail: pattern_comparison_operation::refusal_detail(&refusal),
                        }),
                    ),
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure,
                        },
                    )
                    .map_err(|error| format!("complete bounded pattern comparison: {error:?}"))?;
                continue;
            }
            if matches!(
                contract.as_str(),
                http::CLIENT_OPERATION
                    | http::SERVER_ACCEPT_OPERATION
                    | http::SERVER_RESPOND_OPERATION
            ) {
                let completion = http_host.execute(contract.as_str(), input, &mut http_output);
                let (disposition, output, failure) = match completion {
                    Ok(()) => {
                        let output = if http_output.is_empty() {
                            None
                        } else {
                            let value = scheduler
                                .store_host_value(&http_output)
                                .map_err(|error| format!("store hosted HTTP output: {error:?}"))?;
                            Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| format!("bound hosted HTTP output: {error:?}"))?,
                            )
                        };
                        (HostOperationDisposition::Completed, output, None)
                    }
                    Err(error) => (
                        HostOperationDisposition::Failed,
                        None,
                        Some(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostOperationFailed,
                            detail: error.detail(),
                        }),
                    ),
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure,
                        },
                    )
                    .map_err(|error| format!("complete hosted HTTP operation: {error:?}"))?;
                continue;
            }
            if contract == &midi_input_contract_id {
                if !input.is_empty() {
                    return Err("MIDI input request carries unexpected bytes".into());
                }
                let node = usize::from(request.node.0);
                if midi_input_requests[node].replace(request).is_some() {
                    return Err("MIDI input node has two pending host requests".into());
                }
                requests.push(request);
                continue;
            } else if contract == &keyboard_contract_id {
                keyboard_host.accept(request, input)?;
                requests.push(request);
                continue;
            } else if contract.as_str() == synth_operation::SYNTH_HOST_OPERATION {
                let state = synth_states
                    .get_mut(usize::from(request.node.0))
                    .and_then(Option::as_mut)
                    .ok_or_else(|| "synth request has no exact admitted state".to_string())?;
                let has_output = synth_operation::execute(state, input, &mut synth_output)?;
                let output = if has_output {
                    let value = scheduler
                        .store_host_value(&synth_output)
                        .map_err(|error| format!("store reference synth PCM: {error:?}"))?;
                    Some(
                        BoundedValueRef::new(value, synth_operation::PCM_BLOCK_BYTES)
                            .map_err(|error| format!("bound reference synth PCM: {error:?}"))?,
                    )
                } else {
                    None
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition: HostOperationDisposition::Completed,
                            output,
                            failure: None,
                        },
                    )
                    .map_err(|error| format!("complete reference synth render: {error:?}"))?;
                continue;
            } else if contract.as_str() == audio_play_operation::HOST_OPERATION {
                let session = playback_sessions
                    .get_mut(usize::from(request.node.0))
                    .and_then(Option::as_mut)
                    .ok_or_else(|| {
                        "audio/play request has no exact admitted session".to_string()
                    })?;
                let outcome = audio_play_operation::execute(session, input);
                requests.push(request);
                scheduler
                    .complete_host_operation(request.node, request.request, outcome)
                    .map_err(|error| format!("complete audio/play host operation: {error:?}"))?;
                continue;
            } else if matches!(
                contract.as_str(),
                conduit_std_offers::MUSIC_PLAY_MIDI_NOTE_OPERATION
                    | conduit_std_offers::MUSIC_PLAY_MIDI_CONTROL_OPERATION
            ) {
                let node = usize::from(request.node.0);
                let session = midi_output_sessions
                    .get_mut(node)
                    .and_then(Option::as_mut)
                    .ok_or_else(|| "MIDI request has no exact admitted session".to_string())?;
                let adapter = midi_output_adapters
                    .get_mut(node)
                    .and_then(Option::as_mut)
                    .ok_or_else(|| "MIDI request has no exact admitted adapter".to_string())?;
                let outcome =
                    midi_output_operation::execute(adapter, session, contract.as_str(), input);
                requests.push(request);
                scheduler
                    .complete_host_operation(request.node, request.request, outcome)
                    .map_err(|error| format!("complete MIDI output operation: {error:?}"))?;
                continue;
            } else if contract.as_str() == test_audio_source::YIELD_OPERATION
                && lowered_operation.target_kind.is_none()
            {
                if input != [0] {
                    return Err("proof PCM source yield marker is malformed".to_string());
                }
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition: HostOperationDisposition::Completed,
                            output: None,
                            failure: None,
                        },
                    )
                    .map_err(|error| format!("complete proof PCM source yield: {error:?}"))?;
                continue;
            } else if matches!(
                contract.as_str(),
                conduit_ai::GENERATE_TEXT_HOST_OPERATION | conduit_ai::LOCAL_MODEL_OPERATION
            ) {
                let placement = fragment
                    .placements
                    .get(usize::from(request.node.0))
                    .ok_or_else(|| "model request has no exact placement".to_string())?;
                let completion = model_host::execute(
                    contract.as_str(),
                    placement,
                    input,
                    match &mut local_model {
                        Some(adapter) => Some(&mut **adapter),
                        None => None,
                    },
                    &mut generate_text_output,
                )?;
                let output = if completion.has_output() {
                    let value = scheduler
                        .store_host_value(&generate_text_output)
                        .map_err(|error| format!("store model output: {error:?}"))?;
                    Some(
                        BoundedValueRef::new(value, lowered_operation.binding.maximum_output_bytes)
                            .map_err(|error| format!("bound model output: {error:?}"))?,
                    )
                } else {
                    None
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        completion.outcome(output),
                    )
                    .map_err(|error| format!("complete model operation: {error:?}"))?;
                continue;
            } else if contract.as_str() == conduit_ai::VECTOR_SEARCH_OPERATION {
                let placement = fragment
                    .placements
                    .get(usize::from(request.node.0))
                    .ok_or_else(|| "vector-search request has no exact placement".to_string())?;
                let completion = vector_search_host::execute(
                    placement,
                    input,
                    match &mut vector_search {
                        Some(adapter) => Some(&mut **adapter),
                        None => None,
                    },
                    &mut vector_search_output,
                )?;
                let output = if completion.has_output() {
                    let value = scheduler
                        .store_host_value(&vector_search_output)
                        .map_err(|error| format!("store vector-search output: {error:?}"))?;
                    Some(
                        BoundedValueRef::new(value, lowered_operation.binding.maximum_output_bytes)
                            .map_err(|error| format!("bound vector-search output: {error:?}"))?,
                    )
                } else {
                    None
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        completion.outcome(output),
                    )
                    .map_err(|error| format!("complete vector-search operation: {error:?}"))?;
                continue;
            } else if contract
                .as_str()
                .starts_with("conduit.host/external-websocket-listener-")
            {
                let completion = external_websocket_host::execute(
                    contract.as_str(),
                    input,
                    &mut external_listener,
                    &mut external_output,
                )?;
                let (disposition, output) = match completion {
                    external_websocket_host::ExternalHostCompletion::Output => {
                        let value =
                            scheduler
                                .store_host_value(&external_output)
                                .map_err(|error| {
                                    format!("store external WebSocket output: {error:?}")
                                })?;
                        (
                            HostOperationDisposition::Completed,
                            Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| {
                                    format!("bound external WebSocket output: {error:?}")
                                })?,
                            ),
                        )
                    }
                    external_websocket_host::ExternalHostCompletion::NoOutput => {
                        (HostOperationDisposition::Completed, None)
                    }
                    external_websocket_host::ExternalHostCompletion::ReturnedInput => {
                        (HostOperationDisposition::Completed, Some(request.input))
                    }
                    external_websocket_host::ExternalHostCompletion::Disconnected => {
                        let output = if external_output.is_empty() {
                            None
                        } else {
                            let value =
                                scheduler
                                    .store_host_value(&external_output)
                                    .map_err(|error| {
                                        format!("store external WebSocket disconnect: {error:?}")
                                    })?;
                            Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| {
                                    format!("bound external WebSocket disconnect: {error:?}")
                                })?,
                            )
                        };
                        (HostOperationDisposition::Cancelled, output)
                    }
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition,
                            output,
                            failure: None,
                        },
                    )
                    .map_err(|error| {
                        format!("complete external WebSocket host operation: {error:?}")
                    })?;
                continue;
            } else if let Some(completion) = alife_host.execute(
                contract,
                lowered_operation.target_kind.as_ref(),
                request.node,
                input,
                fragment,
                _output,
            ) {
                let outcome = match completion {
                    alife_host::AlifeCompletion::Completed => HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                    alife_host::AlifeCompletion::Output(encoded) => {
                        let value = scheduler
                            .store_host_value(encoded)
                            .map_err(|error| format!("store Lenia field: {error:?}"))?;
                        HostOperationOutcome {
                            disposition: HostOperationDisposition::Completed,
                            output: Some(
                                BoundedValueRef::new(
                                    value,
                                    conduit_alife::LENIA_MAXIMUM_FIELD_BYTES,
                                )
                                .map_err(|error| format!("bound Lenia field: {error:?}"))?,
                            ),
                            failure: None,
                        }
                    }
                    alife_host::AlifeCompletion::Failed(failure) => HostOperationOutcome {
                        disposition: HostOperationDisposition::Failed,
                        output: None,
                        failure: Some(failure),
                    },
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(request.node, request.request, outcome)
                    .map_err(|error| format!("complete alife host operation: {error:?}"))?;
                continue;
            } else if contract == &wait_contract_id {
                let duration = decode_tick(input).map_err(|error| error.to_string())?;
                if let Some(now_ms) = timer.monotonic_now_ms() {
                    deadlines.arm(request, duration, now_ms)?;
                    requests.push(request);
                    continue;
                }
                timer.wait(Duration::from_millis(duration));
            } else if contract == &deadline_contract_id {
                let duration = conduit_core::decode_monotonic_duration(input)
                    .map_err(|error| format!("decode admitted deadline: {error:?}"))?;
                let now_ms = timer
                    .monotonic_now_ms()
                    .ok_or_else(|| "admitted monotonic deadline Base is unavailable".to_string())?;
                deadlines.arm(request, duration, now_ms)?;
                requests.push(request);
                continue;
            } else if contract == &upper_contract_id
                && lowered_operation.target_kind.as_ref() == Some(&upper_target_kind)
            {
                text_operations::uppercase_utf8(input, &mut uppercase_buffer)?;
                let value = scheduler
                    .store_host_value(&uppercase_buffer)
                    .map_err(|error| format!("store uppercase text output: {error:?}"))?;
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        text_operations::completed_with_output(value),
                    )
                    .map_err(|error| format!("complete text/upper host operation: {error:?}"))?;
                continue;
            } else if contract == &join_contract_id
                && lowered_operation.target_kind.as_ref() == Some(&join_target_kind)
            {
                let placement = fragment
                    .placements
                    .get(usize::from(request.node.0))
                    .ok_or_else(|| "text/join request has no exact placement".to_string())?;
                let prefix = text_operations::join_prefix(placement)?;
                text_operations::prefix_utf8(prefix, input, &mut uppercase_buffer)?;
                let value = scheduler
                    .store_host_value(&uppercase_buffer)
                    .map_err(|error| format!("store joined text output: {error:?}"))?;
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        text_operations::completed_with_output(value),
                    )
                    .map_err(|error| format!("complete text/join host operation: {error:?}"))?;
                continue;
            } else if contract == &gate_bool_contract_id
                && lowered_operation.target_kind.as_ref() == Some(&gate_bool_target_kind)
            {
                let enabled = flow_gate_operation::decode_bool(input)?;
                requests.push(request);
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition: HostOperationDisposition::Completed,
                            output: enabled.then_some(request.input),
                            failure: None,
                        },
                    )
                    .map_err(|error| format!("complete flow/gate bool decode: {error:?}"))?;
                continue;
            } else if (contract == &keymap_contract_id
                && lowered_operation.target_kind.as_ref() == Some(&keymap_target_kind))
                || (contract == &chords_contract_id
                    && lowered_operation.target_kind.as_ref() == Some(&chords_target_kind))
            {
                let node = usize::from(request.node.0);
                let completion = input_semantic_operations::execute_host(
                    contract == &keymap_contract_id,
                    &mut input_keymaps[node],
                    input,
                );
                let outcome = match completion {
                    Ok(Some(encoded)) => {
                        let value = scheduler
                            .store_host_value(encoded.as_slice())
                            .map_err(|error| format!("store input semantic output: {error:?}"))?;
                        HostOperationOutcome {
                            disposition: HostOperationDisposition::Completed,
                            output: Some(
                                BoundedValueRef::new(
                                    value,
                                    lowered_operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| {
                                    format!("bound input semantic output: {error:?}")
                                })?,
                            ),
                            failure: None,
                        }
                    }
                    Ok(None) => HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                    Err(failure) => HostOperationOutcome {
                        disposition: HostOperationDisposition::Failed,
                        output: None,
                        failure: Some(failure),
                    },
                };
                requests.push(request);
                scheduler
                    .complete_host_operation(request.node, request.request, outcome)
                    .map_err(|error| {
                        format!("complete input semantic host operation: {error:?}")
                    })?;
                continue;
            } else if math_host.matches(contract, lowered_operation.target_kind.as_ref()) {
                math_host.complete(fragment, request, &mut scheduler, &mut requests)?;
                continue;
            } else if presentation_construction
                .matches(contract, lowered_operation.target_kind.as_ref())
            {
                presentation_construction.complete(
                    fragment,
                    request,
                    contract,
                    lowered_operation.target_kind.as_ref(),
                    &mut scheduler,
                    &mut requests,
                )?;
                continue;
            } else if lowered_operation.target_kind.as_ref()
                == Some(&graphics_presentation_target_kind)
            {
                let scene =
                    conduit_presentation::GraphicsScene::decode(input).map_err(|error| {
                        format!("graphics presentation input is invalid: {error:?}")
                    })?;
                writeln!(
                    _output,
                    "graphics scene commands={}",
                    scene.commands().len()
                )
                .map_err(|error| error.to_string())?;
            } else if lowered_operation.target_kind.as_ref()
                == Some(&structured_presentation_target_kind)
            {
                structured_presentation_host.capture(request, input)?;
            } else if lowered_operation.target_kind.as_ref() == Some(&text_target_kind) {
                let text = std::str::from_utf8(input)
                    .map_err(|_| "text presentation input is not valid UTF-8".to_string())?;
                write!(_output, "PRESENTATION-TEXT bytes={} hex=", input.len())
                    .map_err(|error| error.to_string())?;
                for byte in input {
                    write!(_output, "{byte:02x}").map_err(|error| error.to_string())?;
                }
                writeln!(_output).map_err(|error| error.to_string())?;
                writeln!(_output, "{text}").map_err(|error| error.to_string())?;
            } else if lowered_operation.target_kind.as_ref() == Some(&tick_target_kind) {
                let tick = decode_tick(input).map_err(|error| error.to_string())?;
                writeln!(_output, "tick sequence={tick}").map_err(|error| error.to_string())?;
            } else if lowered_operation.target_kind.as_ref() == Some(&count_target_kind) {
                let count = count_operations::decode_count(input)?;
                writeln!(_output, "count value={count}").map_err(|error| error.to_string())?;
            } else if lowered_operation.target_kind.as_ref() == Some(&bool_target_kind) {
                let value = conduit_core::InfoBool::decode(input)
                    .map_err(|error| format!("Boolean presentation input is invalid: {error:?}"))?;
                writeln!(_output, "bool value={}", value.get())
                    .map_err(|error| error.to_string())?;
            } else {
                #[cfg(test)]
                {
                    if contract != &observer_contract_id
                        || lowered_operation.target_kind.as_ref() != Some(&observer_target_kind)
                    {
                        return Err("installed host-operation contract is unsupported".to_string());
                    }
                    let tick = decode_tick(input).map_err(|error| error.to_string())?;
                    observed_ticks.push(tick);
                    writeln!(_output, "receipt tick sequence={tick}")
                        .map_err(|error| error.to_string())?;
                }
                #[cfg(not(test))]
                return Err("installed host-operation contract is unsupported".to_string());
            }
            requests.push(request);
            scheduler
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                )
                .map_err(|error| format!("complete std host operation: {error:?}"))?;
        }
        let status = match scheduler.step() {
            Ok(status) => status,
            Err(conduit_kernel::scheduler::SchedulerError::OperationFailed(failure))
                if math_host.accept_failure(
                    scheduler
                        .signs()
                        .events()
                        .filter(|event| event.kind == conduit_kernel::KernelEventKind::Decision)
                        .last()
                        .map(|event| event.node),
                    failure.detail,
                ) =>
            {
                scheduler
                    .cancel()
                    .map_err(|error| format!("clean up failed quantity Play: {error:?}"))?;
                deadlines.clear();
                break TerminalDisposition::Failed {
                    reason: conduit_core::FailureReason::RequiredBranchFailed,
                };
            }
            Err(error) => return Err(format!("installed kernel step: {error:?}")),
        };
        match status {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => break TerminalDisposition::Completed,
            SchedulerStatus::Idle => {
                if keyboard_host.poll(&mut scheduler)? {
                    continue;
                }
                let mut completed_midi_input = false;
                let mut pending_midi_input = false;
                for node in 0..midi_input_requests.len() {
                    let Some(request) = midi_input_requests[node] else {
                        continue;
                    };
                    pending_midi_input = true;
                    let session = midi_input_sessions[node]
                        .as_mut()
                        .ok_or_else(|| "pending MIDI input has no admitted session".to_string())?;
                    let readable = match session.wait_readable(Duration::from_millis(u64::from(
                        crate::hosted_midi::MIDI_READINESS_WAIT_MILLIS,
                    ))) {
                        Ok(readable) => readable,
                        Err(error) => {
                            scheduler
                                .complete_host_operation(
                                    request.node,
                                    request.request,
                                    midi_input_operation::failure_outcome(error),
                                )
                                .map_err(|completion| {
                                    format!("complete MIDI input readiness failure: {completion:?}")
                                })?;
                            midi_input_requests[node] = None;
                            completed_midi_input = true;
                            continue;
                        }
                    };
                    if !readable {
                        continue;
                    }
                    let now_micros = timer.monotonic_now_micros().ok_or_else(|| {
                        "admitted MIDI input monotonic-microsecond Base is unavailable".to_string()
                    })?;
                    let outcome = match session.poll(now_micros) {
                        Ok(crate::hosted_midi::MidiInputPoll::Pending) => continue,
                        Ok(crate::hosted_midi::MidiInputPoll::Observation(observation)) => {
                            let encoded = observation.encode().map_err(|error| {
                                format!("encode exact MIDI input observation: {error:?}")
                            })?;
                            let value = scheduler.store_host_value(&encoded).map_err(|error| {
                                format!("store MIDI input observation: {error:?}")
                            })?;
                            HostOperationOutcome {
                                disposition: HostOperationDisposition::Completed,
                                output: Some(
                                    BoundedValueRef::new(
                                        value,
                                        conduit_midi::MIDI_INPUT_OBSERVATION_ENCODED_LEN as u32,
                                    )
                                    .map_err(|error| {
                                        format!("bound MIDI input observation: {error:?}")
                                    })?,
                                ),
                                failure: None,
                            }
                        }
                        Err(error) => midi_input_operation::failure_outcome(error),
                    };
                    scheduler
                        .complete_host_operation(request.node, request.request, outcome)
                        .map_err(|error| {
                            format!("complete MIDI input host operation: {error:?}")
                        })?;
                    midi_input_requests[node] = None;
                    completed_midi_input = true;
                }
                if completed_midi_input {
                    continue;
                }
                if deadlines.complete_next(&mut scheduler, timer)? {
                    continue;
                }
                if pending_midi_input || keyboard_host.is_pending() {
                    std::thread::park_timeout(Duration::from_millis(1));
                    continue;
                }
                return Err(format!(
                    "installed kernel became idle before completion: {:?}",
                    scheduler.signs().events().collect::<Vec<_>>()
                ));
            }
            SchedulerStatus::Cancelled => {
                break TerminalDisposition::Cancelled {
                    reason: CancellationReason::OperatorRequested,
                }
            }
        }
    };
    for state in synth_states.iter_mut().flatten() {
        state.stop();
    }
    for session in playback_sessions.iter_mut().flatten() {
        if session.lifecycle() != crate::hosted_audio::PlaybackLifecycle::StoppedClosed {
            session
                .stop()
                .map_err(|error| format!("close terminal audio/play: {error:?}"))?;
        }
    }
    if !deadlines.is_empty() {
        return Err("installed deadline effects survived terminal Play state".to_string());
    }
    #[cfg(test)]
    let post_play_start_allocations = play_start_probe.finish();

    for driver in scheduler.drivers() {
        robotics_effect::write_simulated_drive_effect(
            _output,
            driver.operation().simulated_drive_effect(),
        )?;
    }

    #[cfg(test)]
    if fragment
        .placements
        .iter()
        .any(|placement| placement.implementation_id.as_str() == TEST_OBSERVER_IMPLEMENTATION)
    {
        let tick_placement = fragment
            .placements
            .iter()
            .find(|placement| {
                placement.implementation_id.as_str() == TICK_FACTORY.implementation_id
            })
            .ok_or_else(|| "tick observer fixture has no installed tick source".to_string())?;
        let count = parse_tick_configuration(&tick_placement.configuration)
            .map_err(|error| error.to_string())?
            .count;
        if observed_ticks != (0..count).collect::<Vec<_>>() {
            return Err("tick observer received an incomplete or reordered sequence".to_string());
        }
    }
    if scheduler.values().used_items() != 0 {
        return Err(format!(
            "installed kernel retained values after completion: items={} bytes={}",
            scheduler.values().used_items(),
            scheduler.values().used_bytes()
        ));
    }
    let driver_capacity_after = scheduler
        .drivers()
        .iter()
        .map(|driver| driver.operation().allocation_capacity())
        .sum::<usize>();
    let value_allocation_after = scheduler.values().allocation_capacities();
    let alife_capacity_after = alife_host.allocation_capacity();
    if driver_capacity_after != driver_capacity_before
        || value_allocation_after != value_allocation_before
        || alife_capacity_after != alife_capacity_before
    {
        return Err("installed storage grew after Play start".to_string());
    }
    for request in &requests {
        execution_identity
            .bind_request(
                &lowered.identity,
                request.node,
                request.request,
                request.operation,
            )
            .map_err(|error| format!("bind std request identity: {error:?}"))?;
    }
    if !matches!(terminal_disposition, TerminalDisposition::Completed) {
        structured_presentation_host.retain_realized_effects();
    }
    let (mut observations, presentation_ids) = structured_presentation_host.project(
        advertisement,
        fragment,
        &active_play,
        &lowered.identity,
        &mut execution_identity,
        next_sign_sequence,
    )?;
    if let Some(failure) = math_host.failure_observation(
        advertisement,
        fragment,
        &active_play,
        &mut execution_identity,
        next_sign_sequence,
    )? {
        observations.push(failure);
    }
    let terminal_sign = bind_sign(
        &advertisement.host_id,
        &advertisement.boot_id,
        Some(&active_play.active_play_id),
        *next_sign_sequence,
    );
    *next_sign_sequence = next_sign_sequence
        .checked_add(1)
        .ok_or_else(|| "std sign sequence exhausted".to_string())?;
    execution_identity
        .bind_sign(&terminal_sign, None, None, None)
        .map_err(|error| format!("bind std terminal sign: {error:?}"))?;
    observations.push(Observation {
        sign_id: terminal_sign.sign_id,
        active_play_id: Some(active_play.active_play_id.clone()),
        presentation_id: None,
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        plan_id: Some(fragment.plan_id.clone()),
        placement_id: None,
        connection_id: None,
        kind: ObservationKind::PlanTerminal {
            disposition: terminal_disposition,
        },
    });
    let control_receipts = accepted_stop
        .map(|request_id| RunControlReceipt {
            request_id,
            active_play_id: active_play.active_play_id.clone(),
            disposition: RunControlDisposition::Accepted,
        })
        .into_iter()
        .collect();
    let playback = playback_sessions
        .iter()
        .flatten()
        .map(crate::hosted_audio::PlaybackSession::report)
        .collect();
    for session in midi_input_sessions.iter_mut().flatten() {
        if session.report().lifecycle == crate::hosted_midi::MidiInputLifecycle::Open {
            session.cancel();
        }
    }
    let midi_input = midi_input_sessions
        .iter()
        .flatten()
        .map(crate::hosted_midi::MidiInputSession::report)
        .collect();
    for session in midi_output_sessions.iter_mut().flatten() {
        if session.report().lifecycle != crate::hosted_midi::MidiOutputLifecycle::StoppedClosed {
            session
                .stop()
                .map_err(|error| format!("close MIDI output: {error:?}"))?;
        }
    }
    let midi_output = midi_output_sessions
        .iter()
        .flatten()
        .map(crate::hosted_midi::MidiOutputSession::report)
        .collect();
    let report = StdRunReport {
        observations,
        receipts: Vec::new(),
        control_receipts,
        kernel: Some(StdKernelExecutionReport {
            active_play_id: active_play.active_play_id,
            decisions: scheduler.decisions(),
            kernel_events: scheduler.signs().len(),
            kernel_sign: scheduler.signs().events().collect(),
            value_allocation_capacity_before: value_allocation_before,
            value_allocation_capacity_after: value_allocation_after,
            presentation_ids,
            playback,
            midi_input,
            midi_output,
            identity: execution_identity,
            #[cfg(test)]
            post_play_start_allocations,
        }),
    };
    retained_run::finish(report, scheduler, fragment.states.len())
}
