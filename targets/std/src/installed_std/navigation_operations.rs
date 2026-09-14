//! Installed bounded navigation operations and their per-Play host state.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId, ValueRef,
};

pub(super) static ROUTE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::NAVIGATION_ROUTE_GRID4_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static TIME_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::NAVIGATION_TIME_PARAMETERIZE_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static CONTROL_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::NAVIGATION_LOCAL_CONTROL_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct NavigationOperation {
    input_count: u16,
    seen: [bool; 4],
    deferred: [Option<ValueRef>; 4],
    pending: Option<RequestId>,
    next_request: u32,
    emitted: bool,
    operation_by_port: [HostOperationId; 4],
}

impl NavigationOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(port),
                value,
            } if port < self.input_count && !self.seen[usize::from(port)] => {
                self.seen[usize::from(port)] = true;
                if self.pending.is_some() {
                    self.deferred[usize::from(port)] = Some(value);
                    OperationAction::Await
                } else {
                    self.request(port, value)
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Completed, None, None) => self
                        .deferred
                        .iter_mut()
                        .enumerate()
                        .find_map(|(port, value)| value.take().map(|value| (port, value)))
                        .map_or(OperationAction::Await, |(port, value)| {
                            self.request(port as u16, value)
                        }),
                    (HostOperationDisposition::Denied, _, _) => {
                        fail(FailureCode::HostOperationDenied, 1)
                    }
                    _ => fail(FailureCode::HostOperationFailed, 2),
                }
            }
            OperationInput::Closed { port: PortId(port) }
                if port < self.input_count && self.seen[usize::from(port)] =>
            {
                OperationAction::Await
            }
            _ => fail(FailureCode::InvalidLifecycle, 3),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.deferred = [None; 4];
    }

    fn request(&mut self, port: u16, value: ValueRef) -> OperationAction {
        let request = RequestId(self.next_request);
        self.next_request = self.next_request.saturating_add(1);
        self.pending = Some(request);
        let Ok(input) = BoundedValueRef::new(value, MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32)
        else {
            return fail(FailureCode::InvalidInput, 4);
        };
        OperationAction::RequestHostOperation {
            request,
            operation: self.operation_by_port[usize::from(port)],
            input,
        }
    }
}

pub(super) struct NavigationHost {
    kind: NavigationHostKind,
    output: Vec<u8>,
}

enum NavigationHostKind {
    Route {
        pose: Option<conduit_semantic_catalog::NavigationPose>,
        goal: Option<conduit_semantic_catalog::NavigationGoal>,
        traversability: Option<conduit_semantic_catalog::Traversability4x4>,
        time: Option<conduit_semantic_catalog::NavigationTime>,
    },
    Time {
        route: Option<conduit_semantic_catalog::NavigationRoute>,
        time: Option<conduit_semantic_catalog::NavigationTime>,
    },
    Control {
        pose: Option<conduit_semantic_catalog::NavigationPose>,
        trajectory: Option<conduit_semantic_catalog::NavigationTrajectory>,
        time: Option<conduit_semantic_catalog::NavigationTime>,
    },
}

impl NavigationHost {
    pub(super) fn execute(
        &mut self,
        contract: &str,
        input: &[u8],
    ) -> Result<Option<&[u8]>, String> {
        let output = match &mut self.kind {
            NavigationHostKind::Route {
                pose,
                goal,
                traversability,
                time,
            } => {
                match contract {
                    conduit_std_offers::NAVIGATION_ROUTE_GRID4_POSE_OPERATION => set(
                        pose,
                        conduit_semantic_catalog::decode_navigation_pose(input).map_err(codec)?,
                    )?,
                    conduit_std_offers::NAVIGATION_ROUTE_GRID4_GOAL_OPERATION => set(
                        goal,
                        conduit_semantic_catalog::decode_navigation_goal(input).map_err(codec)?,
                    )?,
                    conduit_std_offers::NAVIGATION_ROUTE_GRID4_TRAVERSABILITY_OPERATION => set(
                        traversability,
                        conduit_semantic_catalog::decode_navigation_traversability(input)
                            .map_err(codec)?,
                    )?,
                    conduit_std_offers::NAVIGATION_ROUTE_GRID4_TIME_OPERATION => set(
                        time,
                        conduit_semantic_catalog::decode_navigation_time(input).map_err(codec)?,
                    )?,
                    _ => return Err("unknown route host operation".into()),
                }
                let (Some(pose), Some(goal), Some(traversability), Some(time)) = (
                    pose.as_ref(),
                    goal.as_ref(),
                    traversability.as_ref(),
                    time.as_ref(),
                ) else {
                    return Ok(None);
                };
                conduit_semantic_catalog::encode_route_decision(
                    &conduit_semantic_catalog::route_grid4(pose, goal, traversability, time)
                        .map_err(|error| format!("route navigation: {error:?}"))?,
                )
                .map_err(|error| format!("encode route decision: {error:?}"))?
            }
            NavigationHostKind::Time { route, time } => {
                match contract {
                    conduit_std_offers::NAVIGATION_TIME_PARAMETERIZE_ROUTE_OPERATION => set(
                        route,
                        conduit_semantic_catalog::decode_navigation_route(input).map_err(codec)?,
                    )?,
                    conduit_std_offers::NAVIGATION_TIME_PARAMETERIZE_TIME_OPERATION => set(
                        time,
                        conduit_semantic_catalog::decode_navigation_time(input).map_err(codec)?,
                    )?,
                    _ => return Err("unknown trajectory host operation".into()),
                }
                let (Some(route), Some(time)) = (route.as_ref(), time.as_ref()) else {
                    return Ok(None);
                };
                conduit_semantic_catalog::encode_trajectory(
                    &conduit_semantic_catalog::time_parameterize(route, time, 100, 50, 30_000_000)
                        .map_err(|error| format!("time-parameterize navigation: {error:?}"))?,
                )
                .map_err(|error| format!("encode navigation trajectory: {error:?}"))?
            }
            NavigationHostKind::Control {
                pose,
                trajectory,
                time,
            } => {
                match contract {
                    conduit_std_offers::NAVIGATION_LOCAL_CONTROL_POSE_OPERATION => set(
                        pose,
                        conduit_semantic_catalog::decode_navigation_pose(input).map_err(codec)?,
                    )?,
                    conduit_std_offers::NAVIGATION_LOCAL_CONTROL_TRAJECTORY_OPERATION => set(
                        trajectory,
                        conduit_semantic_catalog::decode_navigation_trajectory(input)
                            .map_err(codec)?,
                    )?,
                    conduit_std_offers::NAVIGATION_LOCAL_CONTROL_TIME_OPERATION => set(
                        time,
                        conduit_semantic_catalog::decode_navigation_time(input).map_err(codec)?,
                    )?,
                    _ => return Err("unknown local-control host operation".into()),
                }
                let (Some(pose), Some(trajectory), Some(time)) =
                    (pose.as_ref(), trajectory.as_ref(), time.as_ref())
                else {
                    return Ok(None);
                };
                conduit_semantic_catalog::encode_control(
                    &conduit_semantic_catalog::local_control(pose, trajectory, time)
                        .map_err(|error| format!("local navigation control: {error:?}"))?,
                )
                .map_err(|error| format!("encode navigation control: {error:?}"))?
            }
        };
        self.output.clear();
        self.output.extend_from_slice(&output);
        Ok(Some(&self.output))
    }
}

fn set<T>(slot: &mut Option<T>, value: T) -> Result<(), String> {
    if slot.replace(value).is_some() {
        Err("duplicate navigation input".into())
    } else {
        Ok(())
    }
}

fn codec(error: conduit_semantic_catalog::NavigationCodecError) -> String {
    format!("decode navigation input: {error:?}")
}

pub(super) fn prepare_hosts(
    fragment: &conduit_core::PlanFragment,
) -> [Option<NavigationHost>; super::MAX_NODES] {
    core::array::from_fn(|index| {
        fragment.placements.get(index).and_then(|placement| {
            let kind = match placement.implementation_id.as_str() {
                conduit_std_offers::NAVIGATION_ROUTE_GRID4_IMPLEMENTATION => {
                    NavigationHostKind::Route {
                        pose: None,
                        goal: None,
                        traversability: None,
                        time: None,
                    }
                }
                conduit_std_offers::NAVIGATION_TIME_PARAMETERIZE_IMPLEMENTATION => {
                    NavigationHostKind::Time {
                        route: None,
                        time: None,
                    }
                }
                conduit_std_offers::NAVIGATION_LOCAL_CONTROL_IMPLEMENTATION => {
                    NavigationHostKind::Control {
                        pose: None,
                        trajectory: None,
                        time: None,
                    }
                }
                _ => return None,
            };
            Some(NavigationHost {
                kind,
                output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
            })
        })
    })
}

pub(super) fn is_host_operation(contract: &str) -> bool {
    matches!(
        contract,
        conduit_std_offers::NAVIGATION_ROUTE_GRID4_GOAL_OPERATION
            | conduit_std_offers::NAVIGATION_ROUTE_GRID4_POSE_OPERATION
            | conduit_std_offers::NAVIGATION_ROUTE_GRID4_TIME_OPERATION
            | conduit_std_offers::NAVIGATION_ROUTE_GRID4_TRAVERSABILITY_OPERATION
            | conduit_std_offers::NAVIGATION_TIME_PARAMETERIZE_ROUTE_OPERATION
            | conduit_std_offers::NAVIGATION_TIME_PARAMETERIZE_TIME_OPERATION
            | conduit_std_offers::NAVIGATION_LOCAL_CONTROL_POSE_OPERATION
            | conduit_std_offers::NAVIGATION_LOCAL_CONTROL_TIME_OPERATION
            | conduit_std_offers::NAVIGATION_LOCAL_CONTROL_TRAJECTORY_OPERATION
    )
}

fn offer(placement: &PlannedGear) -> Result<conduit_core::CapabilityOffer, String> {
    conduit_std_offers::navigation_std_offers()
        .into_iter()
        .find(|offer| offer.implementation.implementation_id == placement.implementation_id)
        .ok_or_else(|| "unknown installed navigation implementation".to_string())
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = offer(placement)?;
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || !placement.configuration.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err("planned navigation identity does not match installation".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        host_requests: placement.inputs.len(),
        sign_items: 32,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::Navigation(NavigationOperation {
        input_count: placement.inputs.len() as u16,
        seen: [false; 4],
        deferred: [None; 4],
        pending: None,
        next_request: 0,
        emitted: false,
        operation_by_port: match placement.implementation_id.as_str() {
            conduit_std_offers::NAVIGATION_ROUTE_GRID4_IMPLEMENTATION => [
                HostOperationId(1),
                HostOperationId(0),
                HostOperationId(3),
                HostOperationId(2),
            ],
            conduit_std_offers::NAVIGATION_TIME_PARAMETERIZE_IMPLEMENTATION => [
                HostOperationId(0),
                HostOperationId(1),
                HostOperationId(0),
                HostOperationId(0),
            ],
            conduit_std_offers::NAVIGATION_LOCAL_CONTROL_IMPLEMENTATION => [
                HostOperationId(0),
                HostOperationId(2),
                HostOperationId(1),
                HostOperationId(0),
            ],
            _ => return Err("unknown installed navigation implementation".into()),
        },
    }))
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
