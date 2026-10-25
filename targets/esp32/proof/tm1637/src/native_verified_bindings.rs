//! Local verified Plan custody. Body admission and kernel startup remain separate.
use super::*;
use conduit_monotonic_wait::{MonotonicWaitProvider, ReadyMonotonicWaitBase};
use conduit_plan_lowering::{
    lowering::LoweredPlanFragment, selected_operation::SelectedVerifiedOperationPlan,
};
use conduit_pulse_capture::{
    installation::ReadyPulseBase, owner::PulseReadProvider, PulseReadContract,
};

pub struct VerifiedNativeInput<'a> {
    pub plan: &'a c::VerifiedPlanMetadata,
    pub fragment: &'a c::FragmentId,
    pub lowered: &'a LoweredPlanFragment,
    pub active: &'a c::ActivePlayIdentity,
    pub cadence: &'a conduit_time::TickConfiguration,
    pub pulse: &'a c::PlacementId,
    pub wait: &'a c::PlacementId,
    pub contract: &'a PulseReadContract,
}
impl RootIssuer {
    /// Reserve current Root identity from Core-verified scope, without restoring
    /// retired configuration or authorizing Body start or native effects.
    pub fn reserve_verified_play(
        &mut self,
        plan: &c::VerifiedPlanMetadata,
        fragment: &c::FragmentId,
    ) -> Result<c::ActivePlayIdentity, ()> {
        let view = plan.fragment(fragment).ok_or(())?;
        let metadata = view.lowering_metadata();
        if metadata.host_id != self.host || metadata.boot_id != self.boot {
            return Err(());
        }
        self.reserve_plan(plan.plan_id())
    }
    /// Root supplies cadence and placement identities from independently checked
    /// resident Source. Both exact native selections pass before issuance state
    /// is spent or either initialized owner leaves its slot.
    pub fn bind_verified_native<P: PulseReadProvider, W: MonotonicWaitProvider>(
        &mut self,
        pulse: &mut Option<ReadyPulseBase<P>>,
        wait: &mut Option<ReadyMonotonicWaitBase<W>>,
        source: VerifiedNativeInput<'_>,
    ) -> Result<NativeBindings<P, W>, &'static str> {
        let fragment = source
            .plan
            .fragment(source.fragment)
            .ok_or("native-fragment")?;
        let metadata = fragment.lowering_metadata();
        if metadata.host_id != self.host
            || metadata.boot_id != self.boot
            || source.plan.plan_id() != &source.active.plan_id
            || source.pulse == source.wait
        {
            return Err("native-scope");
        }
        let pulse_owner = pulse.as_ref().ok_or("pulse-unavailable")?;
        let wait_owner = wait.as_ref().ok_or("wait-unavailable")?;
        let pulse_possession = self
            .prepare_pulse_possession(
                pulse_owner.native_identity(),
                source.plan.plan_id(),
                source.active,
                source.cadence,
            )
            .map_err(|_| "pulse-possession")?;
        let wait_possession = self
            .prepare_wait_possession(
                wait_owner.native_identity(),
                source.plan.plan_id(),
                source.active,
                source.cadence,
            )
            .map_err(|_| "wait-possession")?;
        let selected = |placement_id| SelectedVerifiedOperationPlan {
            plan: source.plan,
            fragment_id: source.fragment,
            lowered: source.lowered,
            active: source.active,
            placement_id,
        };
        let configuration = [
            c::ConfigurationEntry {
                key: "count".into(),
                value: c::ConfigurationValue::U64(source.cadence.count),
            },
            c::ConfigurationEntry {
                key: "period-ms".into(),
                value: c::ConfigurationValue::U64(source.cadence.period_ms),
            },
        ];
        pulse_owner
            .check_verified(
                &pulse_possession.0,
                &pulse_possession.2,
                source.contract,
                selected(source.pulse),
            )
            .map_err(|_| "pulse-selection")?;
        wait_owner
            .check_verified(
                &wait_possession.0,
                &wait_possession.2,
                &configuration,
                selected(source.wait),
            )
            .map_err(|_| "wait-selection")?;
        self.pulse_generation = pulse_owner.native_identity().provider_generation;
        self.wait_generation = wait_owner.native_identity().provider_generation;
        self.native_issued_play = Some(alloc::boxed::Box::new(source.active.clone()));
        self.pulse_issued = true;
        self.wait_issued = true;
        // As in assigned preparation, any subsequent transfer failure remains
        // fenced. It cannot renew authority or pretend an earlier move rolled back.
        let pulse = ReadyPulseBase::bind_verified_retained(
            pulse,
            pulse_possession.0,
            pulse_possession.1,
            pulse_possession.2,
            source.contract,
            selected(source.pulse),
        )
        .map_err(|_| "pulse-transfer")?;
        let wait = ReadyMonotonicWaitBase::bind_verified_retained(
            wait,
            wait_possession.0,
            wait_possession.1,
            wait_possession.2,
            &configuration,
            selected(source.wait),
        )
        .map_err(|_| "wait-transfer")?;
        Ok(NativeBindings { pulse, wait })
    }
}
