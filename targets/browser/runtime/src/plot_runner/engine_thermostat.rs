//! Exact prepared scan and typed Fore crossing for the browser-owned thermostat.
use super::*;
use conduit_core::{PlacementId, PlannedActivationEntry, PortDirection};
use conduit_plan_lowering::lowering::LoweredForePort;
use conduit_thermostat_plot::{Command, ThermostatState, COMMAND_KIND, STATE_KIND};
use std::collections::BTreeMap;

pub(super) struct ThermostatFore {
    input: LoweredForePort,
    output: LoweredForePort,
    sequence: u64,
    maximum_items: u16,
    committed_outputs: u64,
    state: ThermostatState,
}

pub(super) fn prepare(
    partitions: &[(&PlanFragment, &LoweredPlanFragment)],
    application: Option<&preparation::ApplicationPreparation<'_>>,
) -> Result<
    (
        BTreeMap<PlacementId, conduit_composite::KernelScanBack>,
        Option<ThermostatFore>,
    ),
    String,
> {
    let mut scans = BTreeMap::new();
    let mut fore = None;
    let Some(application) = application else {
        return Ok((scans, fore));
    };
    for (fragment, lowered) in partitions {
        let plan = &application
            .plan
            .plots
            .iter()
            .find(|part| part.plan.plan_id == fragment.plan_id)
            .ok_or("browser partition lacks its whole sealed Plan")?
            .plan;
        if plan.activations.is_empty() {
            continue;
        }
        if plan.activations.len() != 1 || fore.is_some() {
            return Err("browser supports one exact thermostat scan per Body".into());
        }
        let PlannedActivationEntry::Scan(planned) = &plan.activations[0] else {
            return Err("browser activation Kind is unsupported".into());
        };
        let owner = fragment
            .placements
            .iter()
            .find(|placement| placement.placement_id == planned.owner_placement_id)
            .ok_or("thermostat scan owner is absent")?;
        crate::installed_browser::thermostat_scan_offer::validate_planned_thermostat_scan(
            owner, planned,
        )?;
        let maximum_steps =
            crate::installed_browser::thermostat_child_bound::maximum_scan_child_steps(planned)?;
        let child = &planned.selected_plan.fragments[0];
        if child.host_id != fragment.host_id
            || child.boot_id != fragment.boot_id
            || child.offer_generation != fragment.offer_generation
            || !planned.selected_plan.activations.is_empty()
        {
            return Err("thermostat child Plan differs from its parent Host or is nested".into());
        }
        let mut host = crate::flow_activation::BrowserActivationHost::for_plan(
            conduit_core::PreparationHostIdentity {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                offer_generation: fragment.offer_generation,
            },
            plan,
        )?;
        let mut prepared =
            conduit_core::prepare_plan_on_hosts(plan, &mut [&mut host]).map_err(debug_error)?;
        let scan = crate::flow_activation::install_planned_activation(
            plan,
            &mut prepared,
            &planned.activation_id,
            &mut host,
        )
        .map_err(debug_error)?
        .into_scan()
        .map_err(debug_error)?;
        let mut back =
            conduit_composite::KernelScanBack::prepare(scan, planned, 256, maximum_steps)?;
        back.bind_parent_play(application.active_play_id)?;
        let input = lowered
            .fore_ports
            .iter()
            .find(|port| {
                port.direction == PortDirection::Input && port.value_kind.as_str() == COMMAND_KIND
            })
            .ok_or("thermostat command Fore is absent")?
            .clone();
        let output = lowered
            .fore_ports
            .iter()
            .find(|port| {
                port.direction == PortDirection::Output && port.value_kind.as_str() == STATE_KIND
            })
            .ok_or("thermostat state Fore is absent")?
            .clone();
        if input.selected_line.is_some() || output.selected_line.is_some() {
            return Err("browser thermostat requires local typed Fore".into());
        }
        fore = Some(ThermostatFore {
            input,
            output,
            sequence: 0,
            committed_outputs: 0,
            maximum_items: planned.limits.maximum_items,
            state: ThermostatState::decode_info(&planned.initial_accumulator)
                .map_err(debug_error)?,
        });
        scans.insert(owner.placement_id.clone(), back);
    }
    Ok((scans, fore))
}

pub(super) fn capture_output(scheduler: &mut TourScheduler) -> Result<(), String> {
    let Some(fore) = scheduler.thermostat.as_ref() else {
        return Ok(());
    };
    let port = fore.output.clone();
    if let Some(offer) = scheduler
        .kernel
        .remote_egress_offer(port.endpoint, port.cord)
        .map_err(debug_error)?
    {
        let bytes = scheduler
            .kernel
            .host_value(offer.value)
            .map_err(debug_error)?;
        port.validate_value(bytes).map_err(debug_error)?;
        let state = ThermostatState::decode(bytes).map_err(debug_error)?;
        scheduler
            .kernel
            .remote_egress_accept(port.endpoint, port.cord, offer.sequence)
            .map_err(debug_error)?;
        scheduler
            .kernel
            .remote_egress_delivered(port.endpoint, port.cord, offer.sequence)
            .map_err(debug_error)?;
        let fore = scheduler.thermostat.as_mut().expect("prepared Fore");
        fore.state = state;
        fore.committed_outputs = offer.sequence + 1;
    }
    Ok(())
}

impl TourScheduler {
    pub(in crate::plot_runner) fn thermostat_commands_available(&self) -> bool {
        self.thermostat.as_ref().is_some_and(|fore| {
            fore.sequence < u64::from(fore.maximum_items) && fore.sequence == fore.committed_outputs
        })
    }
    pub(in crate::plot_runner) fn thermostat_state(&self) -> Option<ThermostatState> {
        self.thermostat.as_ref().map(|fore| fore.state)
    }
    pub(in crate::plot_runner) fn validate_thermostat_command(
        &self,
        command: Command,
    ) -> Result<(), String> {
        let fore = self
            .thermostat
            .as_ref()
            .ok_or("thermostat is not playing")?;
        if fore.sequence >= u64::from(fore.maximum_items) {
            return Err("thermostat command bound exhausted".into());
        }
        if fore.sequence != fore.committed_outputs {
            return Err("thermostat command is awaiting its output".into());
        }
        fore.state.apply(command).map_err(debug_error)?;
        fore.input
            .validate_value(&command.encode())
            .map_err(debug_error)
    }
    pub(in crate::plot_runner) fn admit_thermostat_command(
        &mut self,
        command: Command,
    ) -> Result<(), String> {
        self.validate_thermostat_command(command)?;
        let fore = self.thermostat.as_ref().expect("validated Fore");
        let bytes = command.encode();
        Command::decode(&bytes).map_err(debug_error)?;
        fore.input.validate_value(&bytes).map_err(debug_error)?;
        match self
            .kernel
            .admit_remote_input(fore.input.endpoint, fore.input.cord, fore.sequence, &bytes)
            .map_err(debug_error)?
        {
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. } => {
                self.thermostat.as_mut().expect("prepared Fore").sequence += 1;
                Ok(())
            }
            conduit_kernel::scheduler::RemoteIngressOutcome::Full { .. } => {
                Err("thermostat command Fore is full".into())
            }
        }
    }
}

impl super::super::TourSession {
    pub(in crate::plot_runner) fn submit_thermostat_command(
        &mut self,
        command: Command,
    ) -> Result<super::super::TourProgress, String> {
        self.scheduler.admit_thermostat_command(command)?;
        self.poll_effect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "creche-surface")]
    #[test]
    fn normal_reviewed_bundle_plans_and_plays_exact_browser_thermostat() {
        let source = include_str!("../../../../../plots/thermostat/main.conduit");
        let mut request = crate::plot_runner::body_start::tests::request_from_sources(&[source]);
        let bundle = serde_json::json!({"schema":"conduit.creche/reviewed-plot-bundle@2","plots":[{"slug":"thermostat","entry":"thermostat/main","source":source}]}).to_string();
        let host = crate::installed_browser::advertisement(
            request.local_host_id.clone(),
            request.local_boot_id.clone(),
        );
        let plans = crate::creche::plan_workspace_plots(
            request.body_evidence.as_ref().unwrap(),
            &bundle,
            &[host],
            &request.local_host_id,
            &request.local_boot_id,
            &[],
            crate::creche::PlanningAuthority::default(),
        )
        .unwrap();
        assert_eq!(plans[0].plan.activations.len(), 1);
        request.plan = conduit_body::BodyPlan::seal(&request.wake, plans).unwrap();
        let (mut session, _) = crate::plot_runner::body_start::prepare(request).unwrap();
        session
            .submit_thermostat_command(Command::SetMode(conduit_thermostat_plot::Mode::Cool))
            .unwrap();
        assert_eq!(
            session.scheduler.thermostat_state().unwrap().mode,
            conduit_thermostat_plot::Mode::Cool
        );
        session.cancel().unwrap();
    }
    #[test]
    fn declared_command_bound_executes_without_sign_exhaustion_and_refuses_next() {
        let request = crate::plot_runner::body_start::tests::request_from_sources(&[include_str!(
            "../../../../../plots/thermostat/main.conduit"
        )]);
        let (mut session, _) = crate::plot_runner::body_start::prepare(request).unwrap();
        let command = Command::SetMode(conduit_thermostat_plot::Mode::Heat);
        for revision in 1..=256 {
            session
                .submit_thermostat_command(Command::SetMode(if revision % 2 == 1 {
                    conduit_thermostat_plot::Mode::Heat
                } else {
                    conduit_thermostat_plot::Mode::Cool
                }))
                .unwrap();
            assert_eq!(
                session.scheduler.thermostat_state().unwrap().revision,
                revision
            );
        }
        assert!(session
            .scheduler
            .validate_thermostat_command(command)
            .is_err());
        assert!(session.submit_thermostat_command(command).is_err());
        assert_eq!(session.scheduler.thermostat_state().unwrap().revision, 256);
        let receipt = session.cancel().unwrap();
        let bytes = serde_json::to_vec(&receipt).unwrap();
        assert!(
            bytes.len() <= crate::plot_runner::abi::OUTPUT_BYTES,
            "terminal receipt {} bytes exceeds admitted ABI output {}",
            bytes.len(),
            crate::plot_runner::abi::OUTPUT_BYTES
        );
    }
}
