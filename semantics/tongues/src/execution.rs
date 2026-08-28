use crate::{plan_speech, OutputCondition, SPECIMEN_TEXT};
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver, SchedulerError, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, FixedHostOperationBindings, FixedRoutes,
    HostOperationDisposition, HostOperationOutcome, HostedSignLog, HostedValueStore, KernelEvent,
    NodeId, Operation, OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use serde::{Deserialize, Serialize};

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const MAX_SIGNS: u16 = 256;
type SpeechScheduler = FixedScheduler<
    OperationDriver<SpeechOperation, PORTS>,
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
        input: Option<ValueRef>,
        operation: conduit_kernel::HostOperationId,
        maximum_input_bytes: u32,
    },
    Present {
        stage: u8,
        input: Option<ValueRef>,
        operation: conduit_kernel::HostOperationId,
        maximum_input_bytes: u32,
    },
}

impl Operation for SpeechOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source { value, emitted } => {
                *emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: *value,
                }
            }
            _ => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Synthesize {
                    stage,
                    input,
                    operation,
                    maximum_input_bytes,
                },
                OperationInput::Value { value, .. },
            )
            | (
                Self::Present {
                    stage,
                    input,
                    operation,
                    maximum_input_bytes,
                },
                OperationInput::Value { value, .. },
            ) => {
                *stage = 1;
                *input = Some(value);
                OperationAction::RequestHostOperation {
                    request: RequestId(1),
                    operation: *operation,
                    input: BoundedValueRef::new(value, *maximum_input_bytes).unwrap(),
                }
            }
            (
                Self::Synthesize { stage, .. },
                OperationInput::HostOperationCompleted { outcome, .. },
            ) => {
                *stage = 2;
                match (outcome.disposition, outcome.output) {
                    (HostOperationDisposition::Completed, Some(output)) => OperationAction::Emit {
                        port: PortId(0),
                        value: output.value,
                    },
                    (HostOperationDisposition::Cancelled, _) => OperationAction::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 1,
                    }),
                    _ => OperationAction::Fail(Failure {
                        code: FailureCode::HostOperationFailed,
                        detail: 2,
                    }),
                }
            }
            (
                Self::Present { stage, .. },
                OperationInput::HostOperationCompleted { outcome, .. },
            ) => {
                *stage = 2;
                match outcome.disposition {
                    HostOperationDisposition::Completed => OperationAction::Complete,
                    HostOperationDisposition::Cancelled => OperationAction::Fail(Failure {
                        code: FailureCode::Cancelled,
                        detail: 3,
                    }),
                    _ => OperationAction::Fail(Failure {
                        code: FailureCode::HostOperationFailed,
                        detail: 4,
                    }),
                }
            }
            (_, OperationInput::Closed { .. }) => OperationAction::Complete,
            _ => OperationAction::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 5,
            }),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source { emitted: true, .. } | Self::Synthesize { stage: 2, .. } => {
                OperationAction::Complete
            }
            _ => OperationAction::Await,
        }
    }
}

pub fn run_speech(
    condition: OutputCondition,
    fault: SpeechFault,
) -> Result<SpeechRunReceipt, String> {
    let planned = plan_speech(condition)?;
    let plan_id = planned.plan.plan_id.as_str().to_owned();
    let terminal = match fault {
        SpeechFault::FormatMismatch => Some(SpeechOutcome::FormatMismatch),
        SpeechFault::Pressure => Some(SpeechOutcome::Pressure),
        SpeechFault::ImplementationUnavailable => Some(SpeechOutcome::ImplementationUnavailable),
        SpeechFault::BaseDenied => Some(SpeechOutcome::BaseDenied),
        _ => None,
    };
    if let Some(outcome) = terminal {
        return Ok(receipt(plan_id, condition, outcome, &[]));
    }

    let pcm = deterministic_pcm(SPECIMEN_TEXT);
    if pcm.len() > crate::MAXIMUM_PCM_BYTES as usize {
        return Ok(receipt(plan_id, condition, SpeechOutcome::Pressure, &[]));
    }
    let (mut scheduler, pcm_ref) = scheduler(&planned, &pcm)?;
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
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Failed,
                        output: None,
                        failure: Some(Failure {
                            code: FailureCode::HostOperationFailed,
                            detail: 6,
                        }),
                    }
                } else if fault == SpeechFault::BaseLost {
                    outcome = Some(SpeechOutcome::BaseLost);
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Denied,
                        output: None,
                        failure: Some(Failure {
                            code: FailureCode::HostOperationDenied,
                            detail: 7,
                        }),
                    }
                } else {
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: Some(
                            BoundedValueRef::new(pcm_ref, crate::MAXIMUM_PCM_BYTES).unwrap(),
                        ),
                        failure: None,
                    }
                };
                scheduler
                    .complete_host_operation(request.node, request.request, host_outcome)
                    .map_err(debug)?;
            } else {
                let failed = fault == SpeechFault::DeviceFailure;
                if failed {
                    outcome = Some(SpeechOutcome::DeviceFailure);
                }
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition: if failed {
                                HostOperationDisposition::Failed
                            } else {
                                HostOperationDisposition::Completed
                            },
                            output: None,
                            failure: failed.then_some(Failure {
                                code: FailureCode::HostOperationFailed,
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
            Ok(SchedulerStatus::Complete | SchedulerStatus::Cancelled) => break,
            Ok(SchedulerStatus::Progress { .. }) => {}
            Ok(SchedulerStatus::Idle) => return Err("speech kernel became idle".into()),
            Err(SchedulerError::OperationFailed(_)) if outcome.is_some() => break,
            Err(error) => return Err(debug(error)),
        }
    }
    let events = scheduler.signs().events().collect::<Vec<_>>();
    Ok(receipt(
        plan_id,
        condition,
        outcome.ok_or("kernel ended without outcome")?,
        &events,
    ))
}

fn scheduler(
    planned: &crate::PlannedSpeech,
    pcm: &[u8],
) -> Result<(SpeechScheduler, ValueRef), String> {
    let lowered = &planned.lowered;
    let mut values =
        HostedValueStore::new(4, crate::MAXIMUM_PCM_BYTES, crate::MAXIMUM_PCM_BYTES * 2)
            .map_err(debug)?;
    let text = values.store(SPECIMEN_TEXT.as_bytes()).map_err(debug)?;
    let pcm_ref = values.store(pcm).map_err(debug)?;
    let mut operations = Vec::new();
    for node in &lowered.nodes {
        operations.push(
            OperationDriver::new(match kind_for_node(planned, node.node)? {
                "text/literal" => SpeechOperation::Source {
                    value: text,
                    emitted: false,
                },
                crate::SPEECH_SYNTHESIZE_KIND => SpeechOperation::Synthesize {
                    stage: 0,
                    input: None,
                    operation: lowered
                        .host_operations
                        .iter()
                        .find(|op| op.node == node.node)
                        .ok_or("synthesis operation missing")?
                        .operation,
                    maximum_input_bytes: lowered
                        .host_operations
                        .iter()
                        .find(|op| op.node == node.node)
                        .unwrap()
                        .binding
                        .maximum_input_bytes,
                },
                crate::AUDIO_PLAY_KIND => SpeechOperation::Present {
                    stage: 0,
                    input: None,
                    operation: lowered
                        .host_operations
                        .iter()
                        .find(|op| op.node == node.node)
                        .ok_or("presentation operation missing")?
                        .operation,
                    maximum_input_bytes: lowered
                        .host_operations
                        .iter()
                        .find(|op| op.node == node.node)
                        .unwrap()
                        .binding
                        .maximum_input_bytes,
                },
                other => return Err(format!("unexpected planned kind {other}")),
            })
            .map_err(debug)?,
        );
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
    let mut bindings = FixedHostOperationBindings::<6>::new(2);
    for operation in &lowered.host_operations {
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
    let scheduler = SpeechScheduler::new_with_active_counts_and_host_operations(
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
    condition: OutputCondition,
    outcome: SpeechOutcome,
    events: &[KernelEvent],
) -> SpeechRunReceipt {
    let signs = crate::signs::outcome_signs(&outcome);
    debug_assert!(signs.len() <= 4);
    let sign_digest = sha256(&serde_json::to_vec(&signs).expect("speech Signs serialize"));
    SpeechRunReceipt {
        plan_id,
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
}
