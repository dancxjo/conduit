//! Bind the existing remote kernel to exact, already-retained Body startup.
//! This witness grants neither resources, sound authority, nor transport rights.
use super::InstalledRemoteFragment;
use crate::body_execution::CommittedBodyStartup;
use conduit_body::{PreparedBodyFragment, StartupScope};

impl InstalledRemoteFragment {
    /// Root must supply the original prepared containment and acknowledged
    /// biography. Public fragment IDs or a transport acknowledgment are not
    /// substitutes. Calling this method itself performs no kernel step/effect.
    pub fn bind_committed_body_startup(
        &mut self,
        scope: PreparedBodyFragment<'_>,
        committed: &CommittedBodyStartup,
    ) -> Result<(), String> {
        if self.startup_bound {
            return Err("remote startup is absent or already bound".into());
        }
        let original = scope.fragment();
        if original.fragment_id != self.lowered.identity.fragment_id
            || original.plan_id != self.lowered.identity.plan_id
            || original.placements != self.placements
        {
            return Err("retained Body startup does not contain this exact remote fragment".into());
        }
        committed.for_play(scope.plan(), scope.play())?;
        // The kernel is still unsteppable until every original source is bound.
        // A failed binding cannot make partial source state executable.
        for driver in self.scheduler.drivers() {
            if let super::super::InstalledBack::StartupSource(source) = driver {
                source.gate.bind(committed.eligible(StartupScope::Wake))?;
            }
        }
        self.startup_bound = true;
        Ok(())
    }
}
