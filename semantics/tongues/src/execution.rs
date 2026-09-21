use crate::{plan_speech_text, OutputCondition, SPECIMEN_TEXT};
use conduit_kernel::scheduler::{
    FixedScheduler, SchedulerError, SchedulerStatus, StepBack, StepInputBytes, StepIo, StepOutcome,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, FixedHostCallBindings, FixedRoutes, HostCallDisposition,
    HostCallOutcome, HostedSignLog, HostedValueStore, KernelEvent, NodeId, PortId, RequestId,
    ValueRef, ValueStorage,
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use serde::{Deserialize, Serialize};

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const MAX_SIGNS: u16 = 256;
type SpeechScheduler = FixedScheduler<
    SpeechOperation,
    HostedValueStore,
    HostedSignLog,
    3,
    2,
    PORTS,
    2,
    { 3 * PORTS },
    2,
    6,
    2,
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeechFault {
    None,
    FormatMismatch,
    Pressure,
    Cancelled,
    Underrun,
    ImplementationUnavailable,
    BaseDenied,
    BaseLost,
    DeviceFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeechOutcome {
    Played {
        pcm_sha256: String,
    },
    WavArtifact {
        wav_bytes: u32,
        wav_sha256: String,
        pcm_sha256: String,
    },
    FormatMismatch,
    Pressure,
    Cancelled,
    Underrun,
    ImplementationUnavailable,
    BaseDenied,
    BaseLost,
    DeviceFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeechRunReceipt {
    pub plan_id: String,
    pub text_sha256: String,
    pub condition: OutputCondition,
    pub outcome: SpeechOutcome,
    pub signs: Vec<crate::SpeechSign>,
    pub sign_count: usize,
    pub kernel_event_count: usize,
    pub sign_digest: String,
}

#[derive(Clone, Copy)]
enum SpeechOperation {
    Source {
        value: ValueRef,
        emitted: bool,
    },
    Synthesize {
        stage: u8,
        operation: conduit_kernel::HostCallId,
        maximum_input_bytes: u32,
    },
    Present {
        stage: u8,
        operation: conduit_kernel::HostCallId,
        maximum_input_bytes: u32,
    },
}

impl<const PORTS: usize> StepBack<PORTS> for SpeechOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source { value, emitted } => {
                if *emitted {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), *value)
                    .expect("ready Tongues source output");
                *emitted = true;
                StepOutcome::Progress
            }
            Self::Synthesize {
                stage,
                operation,
                maximum_input_bytes,
            }
            | Self::Present {
                stage,
                operation,
                maximum_input_bytes,
            } if *stage == 0 => {
                if let Some(value) = io.input(PortId(0)) {
                    let Ok(input) = BoundedValueRef::new(value, *maximum_input_bytes) else {
                        return failure(FailureCode::InvalidInput, 5);
                    };
                    io.consume(PortId(0)).expect("present Tongues input");
                    io.request_host_call(RequestId(1), *operation, input)
                        .expect("Tongues Host Call request");
                    *stage = 1;
                    return StepOutcome::Progress;
                }
                if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0))
                        .expect("observed Tongues input closure");
                    return StepOutcome::Complete;
                }
                StepOutcome::Await
            }
            Self::Synthesize { stage, .. } if *stage == 1 => {
                let Some((request, outcome)) = io.host_completion() else {
                    return StepOutcome::Await;
                };
                if request != RequestId(1) {
                    return failure(FailureCode::InvalidInput, 5);
                }
                if outcome.disposition == HostCallDisposition::Completed
                    && outcome.output.is_some()
                    && !io.output_ready(PortId(0))
                {
                    return StepOutcome::Await;
                }
                io.consume_host_completion()
                    .expect("observed Tongues synthesis completion");
                *stage = 2;
                match (outcome.disposition, outcome.output) {
                    (HostCallDisposition::Completed, Some(output)) => {
                        io.send(PortId(0), output.value)
                            .expect("ready Tongues synthesis output");
                        StepOutcome::Progress
                    }
                    (HostCallDisposition::Cancelled, _) => failure(FailureCode::Cancelled, 1),
                    _ => failure(FailureCode::HostCallFailed, 2),
                }
            }
            Self::Present { stage, .. } if *stage == 1 => {
                let Some((request, outcome)) = io.host_completion() else {
                    return StepOutcome::Await;
                };
                if request != RequestId(1) {
                    return failure(FailureCode::InvalidInput, 5);
                }
                io.consume_host_completion()
                    .expect("observed Tongues presentation completion");
                *stage = 2;
                match outcome.disposition {
                    HostCallDisposition::Completed => StepOutcome::Complete,
                    HostCallDisposition::Cancelled => failure(FailureCode::Cancelled, 3),
                    _ => failure(FailureCode::HostCallFailed, 4),
                }
            }
            Self::Synthesize { stage: 2, .. } | Self::Present { stage: 2, .. } => {
                StepOutcome::Complete
            }
            _ => failure(FailureCode::InvalidInput, 5),
        }
    }
}

const fn failure(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

pub fn run_speech(
    condition: OutputCondition,
    fault: SpeechFault,
) -> Result<SpeechRunReceipt, String> {
    run_speech_text(SPECIMEN_TEXT, condition, fault)
}

pub fn run_speech_text(
    text: &str,
    condition: OutputCondition,
    fault: SpeechFault,
) -> Result<SpeechRunReceipt, String> {
    let planned = plan_speech_text(text, condition)?;
    let plan_id = planned.plan.plan_id.as_str().to_owned();
    let text_sha256 = sha256(text.as_bytes());
    let terminal = match fault {
        SpeechFault::FormatMismatch => Some(SpeechOutcome::FormatMismatch),
        SpeechFault::Pressure => Some(SpeechOutcome::Pressure),
        SpeechFault::ImplementationUnavailable => Some(SpeechOutcome::ImplementationUnavailable),
        SpeechFault::BaseDenied => Some(SpeechOutcome::BaseDenied),
        _ => None,
    };
    if let Some(outcome) = terminal {
        return Ok(receipt(plan_id, text_sha256, condition, outcome, &[], None));
    }

    let pcm = deterministic_pcm(text);
    if pcm.len() > crate::MAXIMUM_PCM_BYTES as usize {
        return Ok(receipt(
            plan_id,
            text_sha256,
            condition,
            SpeechOutcome::Pressure,
            &[],
            None,
        ));
    }
    let pcm_bytes = u32::try_from(pcm.len()).map_err(debug)?;
    let (mut scheduler, pcm_ref) = scheduler(&planned, text, &pcm)?;
    let mut outcome = None;
    loop {
        while let Some(request) = scheduler.next_host_request() {
            let node_kind = kind_for_node(&planned, request.node)?;
            if fault == SpeechFault::Cancelled {
                scheduler.cancel().map_err(debug)?;
                outcome = Some(SpeechOutcome::Cancelled);
                break;
            }
            if node_kind == crate::SPEECH_SYNTHESIZE_KIND {
                let host_outcome = if fault == SpeechFault::Underrun {
                    outcome = Some(SpeechOutcome::Underrun);
                    HostCallOutcome {
                        disposition: HostCallDisposition::Failed,
                        output: None,
                        failure: Some(Failure {
                            code: FailureCode::HostCallFailed,
                            detail: 6,
                        }),
                    }
                } else if fault == SpeechFault::BaseLost {
                    outcome = Some(SpeechOutcome::BaseLost);
                    HostCallOutcome {
                        disposition: HostCallDisposition::Denied,
                        output: None,
                        failure: Some(Failure {
                            code: FailureCode::HostCallDenied,
                            detail: 7,
                        }),
                    }
                } else {
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(
                            BoundedValueRef::new(pcm_ref, crate::MAXIMUM_PCM_BYTES).unwrap(),
                        ),
                        failure: None,
                    }
                };
                scheduler
                    .complete_host_call(request.node, request.request, host_outcome)
                    .map_err(debug)?;
            } else {
                let failed = fault == SpeechFault::DeviceFailure;
                if failed {
                    outcome = Some(SpeechOutcome::DeviceFailure);
                }
                scheduler
                    .complete_host_call(
                        request.node,
                        request.request,
                        HostCallOutcome {
                            disposition: if failed {
                                HostCallDisposition::Failed
                            } else {
                                HostCallDisposition::Completed
                            },
                            output: None,
                            failure: failed.then_some(Failure {
                                code: FailureCode::HostCallFailed,
                                detail: 8,
                            }),
                        },
                    )
                    .map_err(debug)?;
                if !failed {
                    let digest = sha256(&pcm);
                    outcome = Some(match condition {
                        OutputCondition::PrimaryPlayback => {
                            SpeechOutcome::Played { pcm_sha256: digest }
                        }
                        OutputCondition::DegradedWavArtifact => {
                            let bytes = wav(&pcm);
                            debug_assert!(bytes.starts_with(b"RIFF"));
                            SpeechOutcome::WavArtifact {
                                wav_bytes: u32::try_from(bytes.len()).map_err(debug)?,
                                wav_sha256: sha256(&bytes),
                                pcm_sha256: digest,
                            }
                        }
                    });
                }
            }
        }
        let status = scheduler.step();
        match status {
            Ok(SchedulerStatus::Drained | SchedulerStatus::Cancelled) => break,
            Ok(SchedulerStatus::Progress { .. }) => {}
            Ok(SchedulerStatus::Idle) => return Err("speech kernel became idle".into()),
            Err(SchedulerError::BackFailed(_)) if outcome.is_some() => break,
            Err(error) => return Err(debug(error)),
        }
    }
    let events = scheduler.signs().events().collect::<Vec<_>>();
    Ok(receipt(
        plan_id,
        text_sha256,
        condition,
        outcome.ok_or("kernel ended without outcome")?,
        &events,
        Some(pcm_bytes),
    ))
}

fn scheduler(
    planned: &crate::PlannedSpeech,
    text_value: &str,
    pcm: &[u8],
) -> Result<(SpeechScheduler, ValueRef), String> {
    let lowered = &planned.lowered;
    let mut values =
        HostedValueStore::new(4, crate::MAXIMUM_PCM_BYTES, crate::MAXIMUM_PCM_BYTES * 2)
            .map_err(debug)?;
    let text = values.store(text_value.as_bytes()).map_err(debug)?;
    let pcm_ref = values.store(pcm).map_err(debug)?;
    let mut operations = Vec::new();
    for node in &lowered.nodes {
        operations.push(match kind_for_node(planned, node.node)? {
            "text/literal" => SpeechOperation::Source {
                value: text,
                emitted: false,
            },
            crate::SPEECH_SYNTHESIZE_KIND => SpeechOperation::Synthesize {
                stage: 0,
                operation: lowered
                    .host_calls
                    .iter()
                    .find(|op| op.node == node.node)
                    .ok_or("synthesis operation missing")?
                    .call,
                maximum_input_bytes: lowered
                    .host_calls
                    .iter()
                    .find(|op| op.node == node.node)
                    .unwrap()
                    .binding
                    .maximum_input_bytes,
            },
            crate::AUDIO_PLAY_KIND => SpeechOperation::Present {
                stage: 0,
                operation: lowered
                    .host_calls
                    .iter()
                    .find(|op| op.node == node.node)
                    .ok_or("presentation operation missing")?
                    .call,
                maximum_input_bytes: lowered
                    .host_calls
                    .iter()
                    .find(|op| op.node == node.node)
                    .unwrap()
                    .binding
                    .maximum_input_bytes,
            },
            other => return Err(format!("unexpected planned kind {other}")),
        });
    }
    let mut routes = FixedRoutes::<{ 3 * PORTS }, 2>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(debug)?;
    }
    routes.seal().map_err(debug)?;
    let mut bindings = FixedHostCallBindings::<6>::new(2);
    for operation in &lowered.host_calls {
        bindings
            .install(operation.node, operation.binding)
            .map_err(debug)?;
    }
    bindings.seal().map_err(debug)?;
    let signs = HostedSignLog::new(
        MAX_SIGNS,
        u32::from(MAX_SIGNS) * core::mem::size_of::<KernelEvent>() as u32,
    )
    .map_err(debug)?;
    let scheduler = SpeechScheduler::new_with_active_counts_and_host_calls(
        3,
        2,
        lowered
            .node_specs
            .clone()
            .try_into()
            .map_err(|_| "node shape")?,
        lowered
            .cords
            .iter()
            .map(|cord| cord.spec)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "cord shape")?,
        routes,
        bindings,
        operations.try_into().map_err(|_| "operation shape")?,
        values,
        signs,
    )
    .map_err(debug)?;
    Ok((scheduler, pcm_ref))
}

fn kind_for_node(planned: &crate::PlannedSpeech, node: NodeId) -> Result<&str, String> {
    let placement_id = &planned.lowered.nodes[usize::from(node.0)].placement_id;
    planned.plan.fragments[0]
        .placements
        .iter()
        .find(|p| &p.placement_id == placement_id)
        .map(|p| p.kind_id.as_str())
        .ok_or_else(|| "lowered node lost placement identity".into())
}

use crate::pcm::{deterministic_pcm, sha256, wav};
fn debug(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
fn receipt(
    plan_id: String,
    text_sha256: String,
    condition: OutputCondition,
    outcome: SpeechOutcome,
    events: &[KernelEvent],
    pcm_bytes: Option<u32>,
) -> SpeechRunReceipt {
    let signs = crate::signs::outcome_signs(&outcome, pcm_bytes);
    debug_assert!(signs.len() <= 4);
    let sign_digest = sha256(&serde_json::to_vec(&signs).expect("speech Signs serialize"));
    SpeechRunReceipt {
        plan_id,
        text_sha256,
        condition,
        outcome,
        sign_count: signs.len(),
        kernel_event_count: events.len(),
        signs,
        sign_digest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn production_kernel_runs_primary_and_degraded_conditions() {
        let primary = run_speech(OutputCondition::PrimaryPlayback, SpeechFault::None).unwrap();
        assert!(matches!(primary.outcome, SpeechOutcome::Played { .. }));
        assert!(primary.sign_count > 0);
        assert!(primary.kernel_event_count > 0);
        let degraded = run_speech(OutputCondition::DegradedWavArtifact, SpeechFault::None).unwrap();
        match degraded.outcome {
            SpeechOutcome::WavArtifact { wav_bytes, .. } => assert_eq!(wav_bytes, 1_260),
            _ => panic!("wrong outcome"),
        }
    }

    #[test]
    fn failures_remain_distinct_and_machine_readable() {
        let cases = [
            (SpeechFault::FormatMismatch, SpeechOutcome::FormatMismatch),
            (SpeechFault::Pressure, SpeechOutcome::Pressure),
            (SpeechFault::Cancelled, SpeechOutcome::Cancelled),
            (SpeechFault::Underrun, SpeechOutcome::Underrun),
            (
                SpeechFault::ImplementationUnavailable,
                SpeechOutcome::ImplementationUnavailable,
            ),
            (SpeechFault::BaseDenied, SpeechOutcome::BaseDenied),
            (SpeechFault::BaseLost, SpeechOutcome::BaseLost),
            (SpeechFault::DeviceFailure, SpeechOutcome::DeviceFailure),
        ];
        for (fault, expected) in cases {
            let receipt = run_speech(OutputCondition::PrimaryPlayback, fault).unwrap();
            assert_eq!(receipt.outcome, expected);
            assert!(!receipt.signs.is_empty());
            let encoded = serde_json::to_string(&receipt.signs).unwrap();
            assert!(!encoded.contains(SPECIMEN_TEXT));
        }
    }

    #[test]
    fn caller_supplied_bounded_text_changes_exact_plan_and_synthesis_identity() {
        let first = run_speech_text(
            "The upstairs temperature is 21 C.",
            OutputCondition::DegradedWavArtifact,
            SpeechFault::None,
        )
        .unwrap();
        let second = run_speech_text(
            "The upstairs temperature is 22 C.",
            OutputCondition::DegradedWavArtifact,
            SpeechFault::None,
        )
        .unwrap();
        assert_ne!(first.plan_id, second.plan_id);
        assert_ne!(first.text_sha256, second.text_sha256);
        assert_ne!(first.outcome, second.outcome);
        let encoded = serde_json::to_string(&first).unwrap();
        assert!(!encoded.contains("upstairs temperature"));
    }

    #[test]
    fn maximum_admitted_text_stays_within_one_pcm_frame_block() {
        let text = "x".repeat(crate::MAXIMUM_TEXT_BYTES as usize);
        let receipt = run_speech_text(
            &text,
            OutputCondition::DegradedWavArtifact,
            SpeechFault::None,
        )
        .unwrap();
        let SpeechOutcome::WavArtifact { wav_bytes, .. } = receipt.outcome else {
            panic!("maximum admitted text must produce the degraded artifact");
        };
        assert!(wav_bytes <= crate::MAXIMUM_PCM_BYTES);
    }
}
