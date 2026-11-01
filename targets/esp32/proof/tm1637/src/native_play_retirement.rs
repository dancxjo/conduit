use super::*;

/// Opaque returned native custody and original mandatory control evidence.
/// A claimed Play ID cannot construct this retirement proof.
#[must_use]
pub struct RetiredNetworkControl {
    pub(super) active: c::ActivePlayIdentity,
    provider: Option<Provider>,
    source: Option<Source>,
    evidence: k::HostedSignLog,
    pub(super) provider_generation: u64,
    pub(super) configuration_revision: u64,
    committed: bool,
    station_outcome: station_outcome::StationOutcome,
    profile_settled: bool,
}
impl RetiredNetworkControl {
    pub fn owners_recoverable(&self) -> bool {
        self.provider.is_some()
            && self
                .source
                .as_ref()
                .is_some_and(|source| source.generation() < u64::MAX)
    }
    pub fn matches_active(&self, active: &c::ActivePlayIdentity) -> bool {
        &self.active == active
    }
    pub fn active(&self) -> &c::ActivePlayIdentity {
        &self.active
    }
    pub fn evidence_is_committed(&self) -> bool {
        self.committed
    }
    #[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
    pub fn commit_evidence(&mut self) -> Result<(), &'static str> {
        if !self.committed {
            journal::append(self)?;
            self.committed = true;
        }
        Ok(())
    }
    /// Only actual committed, returned native custody can settle its selection.
    pub fn settle_profile_outcome(&mut self, now: u64) -> Result<(), &'static str> {
        if !self.committed {
            return Err("network-profile-evidence-pending");
        }
        if !self.profile_settled {
            self.source
                .as_mut()
                .ok_or("network-profile-source")?
                .record_retired_outcome(
                    self.configuration_revision,
                    self.station_outcome.policy_outcome(),
                    now,
                )?;
            self.profile_settled = true;
        }
        Ok(())
    }
    pub fn evidence(&self) -> &k::HostedSignLog {
        &self.evidence
    }
    /// One actual transfer. Caller keeps this object's mandatory evidence until
    /// its admitted persistence/export consumer has accepted it.
    pub fn take_owners(&mut self) -> Option<(Provider, Source)> {
        if !self.committed || self.provider.is_none() || self.source.is_none() {
            return None;
        }
        Some((
            self.provider.take().expect("checked provider"),
            self.source.take().expect("checked source"),
        ))
    }
}
impl NativePlay {
    /// Explicitly revoke the old selected scope, quiesce its kernel, and restore
    /// AP services through the provider's original absolute cleanup deadline.
    /// Failure keeps all actual owners and cleanup obligation in this Play.
    pub async fn restore_for_retirement(&mut self) -> Result<(), &'static str> {
        let join_role = self.execution_join_role()?;
        let source = self.source.as_mut().ok_or("network-retirement-source")?;
        let join = self.join.as_mut().ok_or("network-retirement-provider")?;
        self.receipt_observed = false;
        // An already-retired completed scheduler cannot be cancelled again.
        // Its original Signs stay owned while actual native cleanup proceeds.
        if let Some(prepared) = self.prepared.as_mut() {
            prepared
                .kernel
                .cancel()
                .map_err(|_| "network-retirement-cancel")?;
        }
        source
            .revoke()
            .map_err(|_| "network-retirement-source-revoke")?;
        join.revoke(join_role, k::HostCallId(0))
            .map_err(|_| "network-retirement-join-revoke")?;
        self.phase = "restoring-access-point";
        match join.cleanup().await {
            Ok(()) => {
                self.phase = "access-point-restored";
                Ok(())
            }
            Err(error) => {
                self.cleanup_failure = Some(error);
                self.phase = "cleanup-failed";
                Err("network-retirement-cleanup")
            }
        }
    }
    /// All preflights precede any custody transfer. Fresh Play storage exists
    /// only after both spent owners have returned and evidence has a new owner.
    pub fn take_retired(&mut self) -> Result<RetiredNetworkControl, &'static str> {
        if self.phase != "access-point-restored"
            || self.active.is_none()
            || !self.execution_can_retire()
            || !self
                .source
                .as_ref()
                .is_some_and(|owner| owner.retirement_ready())
            || !self
                .join
                .as_ref()
                .is_some_and(|owner| owner.retirement_ready())
        {
            return Err("network-not-retirable");
        }
        let provider_generation = self
            .join
            .as_ref()
            .expect("preflighted provider")
            .inspection()
            .scope
            .base_provider_generation;
        let configuration_revision = self.configuration_revision;
        let station_outcome = self.station_outcome;
        let provider = n::NetworkJoinHostCall::retire_retained(&mut self.join)
            .map_err(|_| "network-retirement-provider")?;
        let source = n::NetworkCredentialsHostCall::retire_retained(&mut self.source)
            .expect("preflighted source retirement");
        let evidence = self
            .take_execution_signs()
            .unwrap_or_else(|_| unreachable!("exclusive terminal kernel preflight"));
        let active = self.active.take().expect("preflighted scope");
        *self = Self::new();
        Ok(RetiredNetworkControl {
            active,
            provider: Some(provider),
            source: Some(source),
            evidence,
            provider_generation,
            configuration_revision,
            committed: false,
            station_outcome,
            profile_settled: false,
        })
    }
}
