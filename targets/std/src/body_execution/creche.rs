//! Scoped Crèche attachment to the ordinary Body lifecycle and typed Fore.
use super::*;
use crate::hosted_creche::{CrecheHost, HostedCrecheAdapter};
use conduit_body_make::deployment_runtime::{CRECHE_REQUEST_BYTES, CRECHE_REQUEST_KIND};
use conduit_core::{kind_id, ConnectionTrack, PortDirection};

impl StdHost {
    /// Execute the selected ordinary Crèche Plot with its explicit encounter
    /// owner. Preparation binds the prospective execution; effects begin only
    /// after the actual Body Play has started. The output adapter acknowledges
    /// the exact typed outcome, independently of receipt ownership.
    pub fn run_body_plan_with_creche_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        adapter: &mut dyn HostedCrecheAdapter,
        fore_output: &mut dyn BodyForeOutputAdapter,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        request
            .plan
            .validate_for(request.wake)
            .map_err(|error| format!("Crèche Body Plan validation: {error:?}"))?;
        let prospective = BodyPlayIdentity::bind(request.plan, self.next_kernel_play_sequence);
        let mut owner =
            CrecheHost::prepare(adapter, &self.advertisement, request.plan, &prospective)?;
        let result = (|| {
            let context = owner.context();
            let mut ports = context
                .plan()
                .plots
                .iter()
                .filter(|plot| &plot.plot == context.plot())
                .flat_map(|plot| &plot.plan.fragments)
                .filter(|fragment| &fragment.fragment_id == context.fragment_id())
                .flat_map(|fragment| &fragment.fore_ports)
                .filter(|port| {
                    port.direction == PortDirection::Input
                        && port.placement_id == context.placement().placement_id
                        && port.value_kind == kind_id(CRECHE_REQUEST_KIND)
                        && port.track == ConnectionTrack::Payload
                });
            let port = ports
                .next()
                .ok_or("selected Crèche has no exact external request Fore")?;
            if ports.next().is_some()
                || port.gear_port_id != conduit_core::port_id("request")
                || port.byte_capacity < CRECHE_REQUEST_BYTES as u32
            {
                return Err("selected Crèche request Fore is ambiguous or insufficient".into());
            }
            let inputs = [ExternalForeInput {
                front_port_id: port.front_port_id.clone(),
                track: ConnectionTrack::Payload,
                bytes: owner.request().0.to_vec(),
            }];
            self.run_body_plan_to_with_start_and_clock_and_creche(
                request,
                output,
                timer,
                None,
                None,
                Some((&inputs, false, fore_output)),
                None,
                None,
                None,
                Some(&mut owner),
                started,
            )
        })();
        // Also drain preparation/start refusals, before any borrowed owner is
        // dropped. A cleanup failure never erases the actual kernel report.
        let drained = owner.retire();
        match (result, drained) {
            (Ok(mut report), Err(error)) => {
                let message = format!("Crèche retirement: {error}");
                report.cleanup_failure = Some(match report.cleanup_failure {
                    Some(previous) => format!("{previous}; {message}"),
                    None => message,
                });
                Ok(report)
            }
            (Err(error), Err(drain)) => Err(format!("{error}; Crèche retirement: {drain}")),
            (result, Ok(())) => result,
        }
    }
}
