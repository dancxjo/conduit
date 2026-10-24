//! Native custody retirement below Root policy and execution orchestration.
//! No claim, grant or permission for a later Play survives the consuming transfer.
use conduit_core as c;
use conduit_monotonic_wait::{MonotonicWaitHostCall, MonotonicWaitProvider};
use conduit_pulse_capture::owner::{PulseHostCall, PulseReadProvider};
/// Physical providers returned without any old capability table, handle or claim.
pub struct RetiredProviders<P, W> {
    pub pulse: (P, conduit_pulse_capture::owner::PulseAttachment),
    pub wait: (W, conduit_monotonic_wait::WaitAttachment),
    play_scope: [u8; 32],
    provider_generations: [u64; 2],
}
impl<P, W> RetiredProviders<P, W> {
    pub fn matches_active(&self, active: &conduit_core::ActivePlayIdentity) -> bool {
        self.play_scope
            == play_scope([
                active.host_id.as_str(),
                active.boot_id.as_str(),
                active.plan_id.as_str(),
                active.active_play_id.as_str(),
            ])
    }
    pub fn provider_generations(&self) -> [u64; 2] {
        self.provider_generations
    }
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
        let provider_generations = [
            p.inspection().scope.base_provider_generation,
            w.inspection().scope.base_provider_generation,
        ];
        Ok(Self {
            play_scope,
            provider_generations,
            pulse: PulseHostCall::retire_retained(pulse).expect("exclusively preflighted pulse"),
            wait: MonotonicWaitHostCall::retire_retained(wait)
                .expect("exclusively preflighted wait"),
        })
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
