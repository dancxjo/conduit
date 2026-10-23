//! Joint pulse/timer admission before either retained provider is transferred.
//! This owns native possession only; it neither starts a Body nor steps a kernel.
use crate::native_authority::RootIssuer;
use conduit_core as c;
use conduit_environment_runtime::appliance::assigned::{
    ResidentAppliance, ValidatedAssignedAppliance,
};
use conduit_monotonic_wait::{
    MonotonicWaitHostCall, MonotonicWaitProvider, ReadyMonotonicWaitBase,
};
use conduit_plan_lowering::selected_assigned_operation::{
    AssignedOperationInput, SelectedAssignedOperationPlan,
};
use conduit_pulse_capture::{
    installation::ReadyPulseBase,
    owner::{PulseHostCall, PulseReadProvider},
    PulseReadContract,
};

pub struct NativeBindings<P, W> {
    pub pulse: PulseHostCall<P>,
    pub wait: MonotonicWaitHostCall<W>,
}
pub struct AssignedNativeInput<'a> {
    pub assignment: &'a ValidatedAssignedAppliance,
    pub resident: &'a ResidentAppliance<'a>,
    pub input: AssignedOperationInput<'a>,
    pub maxima: c::AssignedPlanMaxima,
    pub requirements: c::AssignedPlanRequirements<'a>,
    pub fragment: &'a c::FragmentId,
    pub active: &'a c::ActivePlayIdentity,
    pub contract: &'a PulseReadContract,
}
impl RootIssuer {
    pub fn bind_assigned_native<P: PulseReadProvider, W: MonotonicWaitProvider>(
        &mut self,
        pulse: &mut Option<ReadyPulseBase<P>>,
        wait: &mut Option<ReadyMonotonicWaitBase<W>>,
        source: AssignedNativeInput<'_>,
    ) -> Result<NativeBindings<P, W>, &'static str> {
        let identity = source.assignment.identity();
        let id = c::AssignedIdentity::from_text;
        if source.assignment.cadence() != source.resident.cadence
            || identity.host != id(self.host.as_str())
            || identity.boot != id(self.boot.as_str())
            || identity.plan != id(source.active.plan_id.as_str())
            || identity.fragment != id(source.fragment.as_str())
        {
            return Err("native-scope");
        }
        let pulse_owner = pulse.as_ref().ok_or("pulse-unavailable")?;
        let wait_owner = wait.as_ref().ok_or("wait-unavailable")?;
        let pulse_possession = self
            .prepare_pulse_possession(
                pulse_owner.native_identity(),
                &source.active.plan_id,
                source.active,
                &source.resident.cadence,
            )
            .map_err(|_| "pulse-possession")?;
        let wait_possession = self
            .prepare_wait_possession(
                wait_owner.native_identity(),
                &source.active.plan_id,
                source.active,
                &source.resident.cadence,
            )
            .map_err(|_| "wait-possession")?;
        let selected = |node| SelectedAssignedOperationPlan {
            input: source.input,
            maxima: source.maxima,
            requirements: source.requirements,
            computations: source.resident.computations,
            fragment_id: source.fragment,
            active: source.active,
            node,
        };
        let configuration = [
            c::ConfigurationEntry {
                key: "count".into(),
                value: c::ConfigurationValue::U64(source.resident.cadence.count),
            },
            c::ConfigurationEntry {
                key: "period-ms".into(),
                value: c::ConfigurationValue::U64(source.resident.cadence.period_ms),
            },
        ];
        pulse_owner
            .check_assigned(
                &pulse_possession.0,
                &pulse_possession.2,
                source.contract,
                selected(source.resident.roles.pulse),
            )
            .map_err(|_| "pulse-selection")?;
        wait_owner
            .check_assigned(
                &wait_possession.0,
                &wait_possession.2,
                &configuration,
                selected(source.resident.roles.tick),
            )
            .map_err(|_| "wait-selection")?;
        // Both exact owners and their selections passed before either issuance
        // budget is spent. Later transfer failure stays fenced; it cannot renew
        // possession or claim rollback of an already moved provider.
        self.pulse_generation = pulse_owner.native_identity().provider_generation;
        self.wait_generation = wait_owner.native_identity().provider_generation;
        self.native_issued_play = Some(source.active.clone());
        self.pulse_issued = true;
        self.wait_issued = true;
        let pulse = ReadyPulseBase::bind_assigned_retained(
            pulse,
            pulse_possession.0,
            pulse_possession.1,
            pulse_possession.2,
            source.contract,
            selected(source.resident.roles.pulse),
        )
        .map_err(|_| "pulse-transfer")?;
        let wait = ReadyMonotonicWaitBase::bind_assigned_retained(
            wait,
            wait_possession.0,
            wait_possession.1,
            wait_possession.2,
            &configuration,
            selected(source.resident.roles.tick),
        )
        .map_err(|_| "wait-transfer")?;
        Ok(NativeBindings { pulse, wait })
    }
}
