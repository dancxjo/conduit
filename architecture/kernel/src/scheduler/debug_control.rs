//! Fixed scheduler-owned breakpoint and suspension state.

use crate::{
    debug_observation::{DebugBreakpoint, DebugControlRefusal, DebugSuspension},
    NodeId,
};

pub(super) struct DebugControlState {
    breakpoint: Option<(DebugBreakpoint, NodeId)>,
    suspension: Option<DebugSuspension>,
}

impl DebugControlState {
    pub(super) const fn new() -> Self {
        Self {
            breakpoint: None,
            suspension: None,
        }
    }

    pub(super) const fn suspension(&self) -> Option<DebugSuspension> {
        self.suspension
    }

    pub(super) fn arm(
        &mut self,
        breakpoint: DebugBreakpoint,
        node: NodeId,
    ) -> Result<(), DebugControlRefusal> {
        if self.breakpoint.is_some() || self.suspension.is_some() {
            return Err(DebugControlRefusal::BreakpointAlreadyArmed);
        }
        self.breakpoint = Some((breakpoint, node));
        Ok(())
    }

    pub(super) fn suspend_before(&mut self, node: NodeId) -> bool {
        let Some((breakpoint, breakpoint_node)) = self.breakpoint else {
            return false;
        };
        if breakpoint_node != node {
            return false;
        }
        self.suspension = Some(DebugSuspension {
            execution: breakpoint.execution,
            subject: breakpoint.subject,
            kind: breakpoint.kind,
            node,
        });
        true
    }

    pub(super) fn resume(
        &mut self,
        suspension: DebugSuspension,
    ) -> Result<NodeId, DebugControlRefusal> {
        let current = self.suspension.ok_or(DebugControlRefusal::NotSuspended)?;
        if current != suspension {
            return Err(DebugControlRefusal::StaleSuspension);
        }
        self.suspension = None;
        self.breakpoint = None;
        Ok(current.node)
    }
}
