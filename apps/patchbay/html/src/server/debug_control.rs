//! Host bridge from Patchbay actions to real bounded kernel debugger control.

use super::{PatchbayHtmlServer, ServerError};
use conduit_kernel::debug_observation::{
    DebugBreakpoint, DebugBreakpointKind, DebugExecutionIdentity, DebugNodeBinding, DebugSubject,
    ObservedSignSink, DEBUG_CONTROL_SCHEMA_VERSION,
};
use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerError, StepInputBytes, StepIo,
    StepOperation, StepOutcome,
};
use conduit_kernel::{
    CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId, PortId, RouteRange,
    RouteTarget,
};
use serde::Deserialize;

const PORTS: usize = 1;

#[derive(Clone, Copy)]
struct DocumentaryDriver;

impl StepOperation<PORTS> for DocumentaryDriver {
    fn step(
        &mut self,
        _io: &mut StepIo<PORTS>,
        _inputs: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        StepOutcome::Complete
    }
}

type DocumentaryScheduler = FixedScheduler<
    DocumentaryDriver,
    FixedValueStore<1, 1>,
    ObservedSignSink<FixedSignLog<8>, 1, PORTS, 8>,
    1,
    1,
    PORTS,
    1,
    1,
    1,
>;

pub(super) struct DocumentaryDebuggerRuntime {
    scheduler: DocumentaryScheduler,
    execution: DebugExecutionIdentity,
    visible_subject: String,
}

impl DocumentaryDebuggerRuntime {
    pub(super) fn from_snapshot(
        snapshot: &crate::RendererSnapshot,
    ) -> Result<Option<Self>, ServerError> {
        let Some(control) = &snapshot.debugger_control else {
            return Ok(None);
        };
        let visible_subject = control
            .eligible_subjects
            .first()
            .cloned()
            .ok_or(ServerError::InvalidRequest)?;
        let execution = DebugExecutionIdentity {
            body: control.execution.body,
            plan: control.execution.plan,
            play: control.execution.play,
        };
        let mut routes = FixedRoutes::<1, 1>::new(PORTS as u16);
        routes
            .install(
                NodeId(0),
                PortId(0),
                RouteRange { start: 0, len: 1 },
                &[RouteTarget {
                    cord: CordId(0),
                    sink: conduit_kernel::CordEndpoint::local(NodeId(0), PortId(0)),
                }],
            )
            .map_err(|error| ServerError::Interaction(format!("debugger route: {error:?}")))?;
        routes
            .seal()
            .map_err(|error| ServerError::Interaction(format!("debugger route: {error:?}")))?;
        let signs = FixedSignLog::<8>::new(
            u32::try_from(8 * core::mem::size_of::<KernelEvent>())
                .map_err(|_| ServerError::InvalidRequest)?,
        )
        .map_err(|error| ServerError::Interaction(format!("debugger signs: {error:?}")))?;
        let observed = ObservedSignSink::<_, 1, PORTS, 8>::detached(
            signs,
            execution,
            [DebugNodeBinding { form: 1, host: 1 }],
            [[None]],
        );
        let scheduler = FixedScheduler::new(
            [NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_work: 1,
            }],
            [CordSpec::local(
                CordId(0),
                (NodeId(0), PortId(0)),
                (NodeId(0), PortId(0)),
                CordCapacity {
                    slot_start: 0,
                    item_capacity: 1,
                    byte_capacity: 1,
                },
            )],
            routes,
            [DocumentaryDriver],
            FixedValueStore::<1, 1>::new(1)
                .map_err(|error| ServerError::Interaction(format!("debugger values: {error:?}")))?,
            observed,
        )
        .map_err(|error| ServerError::Interaction(format!("debugger scheduler: {error:?}")))?;
        Ok(Some(Self {
            scheduler,
            execution,
            visible_subject,
        }))
    }

    fn suspend(&mut self, subject: &str) -> Result<(), ServerError> {
        if subject != self.visible_subject {
            return Err(ServerError::InvalidRequest);
        }
        self.scheduler
            .request_debug_breakpoint(DebugBreakpoint {
                schema_version: DEBUG_CONTROL_SCHEMA_VERSION,
                execution: self.execution,
                subject: DebugSubject::Gear(NodeId(0)),
                kind: DebugBreakpointKind::BeforeGearStart,
            })
            .map_err(|error| ServerError::Interaction(format!("breakpoint refused: {error:?}")))?;
        if self.scheduler.step() != Err(SchedulerError::DebugSuspended) {
            return Err(ServerError::Interaction(
                "runtime did not enter the admitted suspension".into(),
            ));
        }
        Ok(())
    }

    fn resume(&mut self) -> Result<(), ServerError> {
        let suspension = self
            .scheduler
            .debug_suspension()
            .ok_or_else(|| ServerError::Interaction("runtime is not suspended".into()))?;
        self.scheduler
            .resume_debug_suspension(suspension)
            .map_err(|error| ServerError::Interaction(format!("resume refused: {error:?}")))?;
        let _ = self
            .scheduler
            .step()
            .map_err(|error| ServerError::Interaction(format!("resume step: {error:?}")))?;
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DebuggerControlRequest {
    presentation_id: String,
    presentation_revision: u64,
    control_revision: u64,
    action: DebuggerControlAction,
    #[serde(default)]
    subject: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum DebuggerControlAction {
    BreakHere,
    Resume,
}

impl PatchbayHtmlServer {
    pub(super) fn apply_debugger_control(&mut self, body: &[u8]) -> Result<Vec<u8>, ServerError> {
        let request: DebuggerControlRequest =
            serde_json::from_slice(body).map_err(|_| ServerError::InvalidRequest)?;
        let control = self
            .snapshot
            .debugger_control
            .as_mut()
            .ok_or(ServerError::InvalidRequest)?;
        if request.presentation_id != self.snapshot.presentation.identity.as_str()
            || request.presentation_revision != self.snapshot.presentation.revision
            || request.control_revision != control.revision
        {
            return Err(ServerError::InvalidRequest);
        }
        let runtime = self
            .debug_runtime
            .as_mut()
            .ok_or_else(|| ServerError::Interaction("runtime control unsupported".into()))?;
        match request.action {
            DebuggerControlAction::BreakHere => {
                let subject = request.subject.ok_or(ServerError::InvalidRequest)?;
                runtime.suspend(&subject)?;
                control.suspended(&subject);
            }
            DebuggerControlAction::Resume => {
                runtime.resume()?;
                control.resumed();
            }
        }
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(self.encoded_snapshot.clone())
    }
}
