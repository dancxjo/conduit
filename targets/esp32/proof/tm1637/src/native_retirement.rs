//! Native custody retirement below Root policy and execution orchestration.
//! No claim, grant or permission for a later Play survives the consuming transfer.
use conduit_core as c;
use conduit_monotonic_wait::{MonotonicWaitHostCall, MonotonicWaitProvider};
use conduit_pulse_capture::owner::{PulseHostCall, PulseReadProvider};
/// Physical providers returned without any old capability table, handle or claim.
pub struct RetiredProviders<P, W> {
    pulse: conduit_pulse_capture::installation::RetiredPulseBase<P>,
    wait: conduit_monotonic_wait::RetiredWaitBase<W>,
    play_scope: [u8; 32],
}
impl<P, W> RetiredProviders<P, W> {
    pub fn matches_active(&self, active: &conduit_core::ActivePlayIdentity) -> bool {
        matches_retired_scope(&self.play_scope, active)
    }
    pub(crate) fn scope_fingerprint(&self) -> [u8; 32] {
        self.play_scope
    }
    pub fn attachment_generations(&self) -> [u64; 2] {
        [
            self.pulse.attachment().generation,
            self.wait.attachment().generation,
        ]
    }
    pub fn provider_generations(&self) -> [u64; 2] {
        [
            self.pulse.provider_generation(),
            self.wait.provider_generation(),
        ]
    }
}
pub(crate) fn matches_retired_scope(scope: &[u8; 32], active: &c::ActivePlayIdentity) -> bool {
    *scope
        == play_scope([
            active.host_id.as_str(),
            active.boot_id.as_str(),
            active.plan_id.as_str(),
            active.active_play_id.as_str(),
        ])
}
fn play_scope(ids: [&str; 4]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/retired-native-play@1");
    for id in ids {
        hash.update((id.len() as u64).to_le_bytes());
        hash.update(id.as_bytes());
    }
    hash.finalize().into()
}

fn scope_identity(scope: &c::BaseCapabilityScope) -> [u8; 32] {
    play_scope([
        scope.host_id.as_str(),
        scope.boot_id.as_str(),
        scope.plan_id.as_str(),
        scope.active_play_id.as_str(),
    ])
}
pub fn check_pair<P: PulseReadProvider, W: MonotonicWaitProvider>(
    pulse: &PulseHostCall<P>,
    wait: &MonotonicWaitHostCall<W>,
) -> Result<(), &'static str> {
    for inspection in [pulse.inspection(), wait.inspection()] {
        if inspection.lifecycle != c::CapabilityLifecycle::Revoked
            || inspection.in_flight_operations != 0
        {
            return Err("native-retirement-unavailable");
        }
    }
    if scope_identity(&pulse.inspection().scope) != scope_identity(&wait.inspection().scope) {
        return Err("native-retirement-scope");
    }
    Ok(())
}
impl<P: PulseReadProvider, W: MonotonicWaitProvider> RetiredProviders<P, W> {
    /// Both retained owners are checked before either is consumed. The private
    /// scope can only be constructed from these actual revoked, idle owners.
    pub fn retire_pair(
        pulse: &mut Option<PulseHostCall<P>>,
        wait: &mut Option<MonotonicWaitHostCall<W>>,
    ) -> Result<Self, &'static str> {
        let p = pulse.as_ref().ok_or("pulse-unavailable")?;
        let w = wait.as_ref().ok_or("wait-unavailable")?;
        check_pair(p, w)?;
        let play_scope = scope_identity(&p.inspection().scope);
        Ok(Self {
            play_scope,
            pulse: PulseHostCall::retire_base_retained(pulse)
                .expect("exclusively preflighted pulse"),
            wait: MonotonicWaitHostCall::retire_base_retained(wait)
                .expect("exclusively preflighted wait"),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairReattachRefusal {
    Pulse(conduit_pulse_capture::installation::ReattachRefusal),
    Wait(conduit_monotonic_wait::ReattachRefusal),
}
impl<P: PulseReadProvider, W: MonotonicWaitProvider> RetiredProviders<P, W> {
    /// Both actual native identity seals are checked before moving either owner.
    /// This returns readiness only; new Body/Plan/Play admission remains required.
    pub fn check_reattach(
        &self,
        pulse: &conduit_pulse_capture::installation::PulseNativeIdentity,
        wait: &conduit_monotonic_wait::WaitNativeIdentity,
    ) -> Result<(), PairReattachRefusal> {
        self.pulse
            .check_reattach(pulse)
            .map_err(PairReattachRefusal::Pulse)?;
        self.wait
            .check_reattach(wait)
            .map_err(PairReattachRefusal::Wait)
    }
    #[allow(clippy::result_large_err, clippy::type_complexity)] // Cold affine all-owner refusal without allocating.
    pub fn reattach(
        self,
        pulse: conduit_pulse_capture::installation::PulseNativeIdentity,
        wait: conduit_monotonic_wait::WaitNativeIdentity,
    ) -> Result<
        (
            conduit_pulse_capture::installation::ReadyPulseBase<P>,
            conduit_monotonic_wait::ReadyMonotonicWaitBase<W>,
        ),
        (
            Self,
            conduit_pulse_capture::installation::PulseNativeIdentity,
            conduit_monotonic_wait::WaitNativeIdentity,
            PairReattachRefusal,
        ),
    > {
        if let Err(error) = self.check_reattach(&pulse, &wait) {
            return Err((self, pulse, wait, error));
        }
        // Private tokens and owned candidates cannot change after the shared preflight.
        let pulse = match self.pulse.reattach(pulse) {
            Ok(ready) => ready,
            Err(_) => unreachable!("preflighted immutable pulse continuation"),
        };
        let wait = match self.wait.reattach(wait) {
            Ok(ready) => ready,
            Err(_) => unreachable!("preflighted immutable wait continuation"),
        };
        Ok((pulse, wait))
    }
}

/// Returned display custody with no old bearer, claim or table. Its scope can
/// only be captured from an actual revoked idle owner, never received metadata.
pub struct RetiredDisplay<P> {
    pub provider: P,
    pub attachment: conduit_tm1637_wire::owner::WriteAttachment,
    play_scope: [u8; 32],
    provider_generation: u64,
}
impl<P> RetiredDisplay<P> {
    pub fn matches_active(&self, active: &c::ActivePlayIdentity) -> bool {
        self.play_scope
            == play_scope([
                active.host_id.as_str(),
                active.boot_id.as_str(),
                active.plan_id.as_str(),
                active.active_play_id.as_str(),
            ])
    }
    pub fn provider_generation(&self) -> u64 {
        self.provider_generation
    }
}
impl<P: conduit_tm1637_wire::OpenDrain> RetiredDisplay<P> {
    pub fn retire_boxed(
        owner: &mut Option<alloc::boxed::Box<conduit_tm1637_wire::owner::WriteHostCall<P>>>,
    ) -> Result<Self, &'static str> {
        let current = owner.as_ref().ok_or("display-unavailable")?.inspection();
        if current.lifecycle != c::CapabilityLifecycle::Revoked || current.in_flight_operations != 0
        {
            return Err("display-retirement-unavailable");
        }
        let mut returned = Some(*owner.take().expect("preflighted boxed display"));
        Self::retire(&mut returned)
    }

    pub fn retire(
        owner: &mut Option<conduit_tm1637_wire::owner::WriteHostCall<P>>,
    ) -> Result<Self, &'static str> {
        use conduit_tm1637_wire::owner::WriteHostCall;
        let current = owner.as_ref().ok_or("display-unavailable")?;
        let inspection = current.inspection();
        if inspection.lifecycle != c::CapabilityLifecycle::Revoked
            || inspection.in_flight_operations != 0
        {
            return Err("display-retirement-unavailable");
        }
        let play_scope = scope_identity(&inspection.scope);
        let provider_generation = inspection.scope.base_provider_generation;
        let (provider, attachment) = WriteHostCall::retire_retained(owner)
            .expect("exclusively preflighted idle revoked display");
        Ok(Self {
            provider,
            attachment,
            play_scope,
            provider_generation,
        })
    }
}
