//! Actual completed scheduling custody, separate from the still-selected radio.
use super::*;
pub(super) struct CompletedControl {
    signs: k::HostedSignLog,
    roles: n::NetworkControlRoles,
}
impl CompletedControl {
    fn retire(
        prepared: alloc::boxed::Box<PreparedNetworkKernel>,
    ) -> Result<Self, alloc::boxed::Box<PreparedNetworkKernel>> {
        let roles = prepared.roles;
        prepared
            .try_retire_signs()
            .map(|signs| Self { signs, roles })
    }
}
impl NativePlay {
    /// Scheduling completion alone never returns the selected source/provider,
    /// settles a profile, commits evidence or authorizes replacement execution.
    pub(super) fn retire_completed_kernel(&mut self) -> bool {
        if self.completed.is_some()
            || self.phase != "attached"
            || self.receipt_len == 0
            || self.failure.is_some()
        {
            return false;
        }
        let Some(prepared) = self.prepared.take() else {
            return false;
        };
        match CompletedControl::retire(prepared) {
            Ok(completed) => {
                self.completed = Some(completed);
                true
            }
            Err(prepared) => {
                self.prepared = Some(prepared);
                false
            }
        }
    }
    pub(super) fn execution_is_installed(&self) -> bool {
        matches!(
            (&self.prepared, &self.completed),
            (Some(_), None) | (None, Some(_))
        )
    }
    pub(super) fn execution_can_retire(&self) -> bool {
        match (&self.prepared, &self.completed) {
            (Some(prepared), None) => prepared.kernel.can_retire(),
            (None, Some(_)) => true,
            _ => false,
        }
    }
    pub(super) fn execution_join_role(&self) -> Result<k::NodeId, &'static str> {
        match (&self.prepared, &self.completed) {
            (Some(prepared), None) => Ok(prepared.roles.join),
            (None, Some(completed)) => Ok(completed.roles.join),
            _ => Err("network-retirement-kernel"),
        }
    }
    pub(super) fn take_execution_signs(&mut self) -> Result<k::HostedSignLog, &'static str> {
        if !self.execution_can_retire() {
            return Err("network-retirement-kernel");
        }
        if let Some(completed) = self.completed.take() {
            return Ok(completed.signs);
        }
        self.prepared
            .take()
            .expect("preflighted execution")
            .try_retire_signs()
            .map_err(|prepared| {
                self.prepared = Some(prepared);
                "network-retirement-kernel"
            })
    }
}

#[cfg(test)]
#[path = "../tests/completed_control.rs"]
mod tests;
