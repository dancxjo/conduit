//! Session source for kernel-merged deliberate input on an exact R1 control Plan.

use conduit_core::{BootId, HostId, Plan, PlanFragment};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, LoweredPlanFragment, RemoteCordDirection,
};
use conduit_signal::{encode_signal_fixed, SIGNAL_ENCODED_LEN};
use conduit_wire::{
    SessionBinding, SessionCheckpointAcceptance, SessionCheckpointOffer, SessionFrame,
    SessionMachine, SessionRole,
};

use crate::r1_control::{R1ControlKernel, R1InputEvent, R1MergedInput};

pub struct PicoControlSource {
    fragment: PlanFragment,
    lowered: LoweredPlanFragment,
    binding: SessionBinding,
    session: SessionMachine,
    kernel: R1ControlKernel,
    in_flight: Option<R1MergedInput>,
    pressure_retries: u32,
}

impl PicoControlSource {
    pub fn prepare_plan(plan: Plan, source_host: &HostId) -> Result<Self, String> {
        let fragment = plan
            .fragments
            .iter()
            .find(|fragment| &fragment.host_id == source_host)
            .cloned()
            .ok_or_else(|| "R1 control source fragment missing".to_string())?;
        let lowered = lower_plan_fragment(&fragment).map_err(debug_error)?;
        let Some(first_remote) = lowered.remote_endpoints.first() else {
            return Err("R1 control source has no remote Cord".into());
        };
        if lowered.nodes.len() != 4
            || lowered.cords.len() != 4
            || lowered.remote_endpoints.len() > 2
            || lowered.remote_endpoints.iter().any(|endpoint| {
                endpoint.direction != RemoteCordDirection::Egress
                    || endpoint.cord != first_remote.cord
                    || endpoint.connection_id != first_remote.connection_id
                    || endpoint.source_fragment_id != first_remote.source_fragment_id
                    || endpoint.sink_fragment_id != first_remote.sink_fragment_id
            })
            || lowered.host_operations.len() != 3
        {
            return Err("R1 control source is not the exact three-input merge fragment".into());
        }
        let remote = &lowered.remote_endpoints[0];
        let connection = fragment
            .connections
            .iter()
            .find(|connection| connection.connection_id == remote.connection_id)
            .ok_or_else(|| "R1 control remote Cord missing".to_string())?;
        let binding = SessionBinding::from_planned_connection(
            fragment.plan_id.clone(),
            remote.source_fragment_id.clone(),
            remote.sink_fragment_id.clone(),
            connection,
        )
        .map_err(debug_error)?;
        let kernel = R1ControlKernel::from_lowered_plan(&fragment, &lowered)
            .map_err(|error| format!("R1 control kernel: {error:?}"))?;
        let session =
            SessionMachine::new(binding.clone(), SessionRole::Source).map_err(debug_error)?;
        Ok(Self {
            fragment,
            lowered,
            binding,
            session,
            kernel,
            in_flight: None,
            pressure_retries: 0,
        })
    }

    pub fn binding(&self) -> &SessionBinding {
        &self.binding
    }

    pub fn fragment(&self) -> &PlanFragment {
        &self.fragment
    }

    pub fn observe_sink_boot(&mut self, boot: BootId) -> Result<(), String> {
        self.binding = self
            .binding
            .clone()
            .with_observed_boots(self.binding.source.boot_id.clone(), boot)
            .map_err(debug_error)?;
        self.session =
            SessionMachine::new(self.binding.clone(), SessionRole::Source).map_err(debug_error)?;
        Ok(())
    }

    pub fn checkpoint_offer(&self) -> SessionCheckpointOffer<'_> {
        self.session.checkpoint_offer()
    }

    pub fn resume_with_line(
        &mut self,
        line: &conduit_core::AdmittedLine,
        peer: SessionCheckpointOffer<'_>,
    ) -> Result<SessionCheckpointAcceptance, String> {
        let remote = &self.lowered.remote_endpoints[0];
        let connection = self
            .fragment
            .connections
            .iter()
            .find(|connection| connection.connection_id == remote.connection_id)
            .ok_or_else(|| "R1 control remote Cord missing".to_string())?;
        let replacement = SessionBinding::from_planned_connection_with_line(
            self.fragment.plan_id.clone(),
            remote.source_fragment_id.clone(),
            remote.sink_fragment_id.clone(),
            connection,
            line,
        )
        .and_then(|binding| {
            binding.with_observed_boots(
                self.binding.source.boot_id.clone(),
                self.binding.sink.boot_id.clone(),
            )
        })
        .map_err(debug_error)?;
        let acceptance = self
            .session
            .resume_with_attachment(replacement.clone(), peer)
            .map_err(debug_error)?;
        self.binding = replacement;
        Ok(acceptance)
    }

    pub fn offer_input(
        &mut self,
        input: R1InputEvent,
    ) -> Result<(u64, [u8; SIGNAL_ENCODED_LEN as usize]), String> {
        if !self.session.is_active() || self.in_flight.is_some() {
            return Err("R1 control source cannot admit another input now".into());
        }
        self.kernel
            .offer(input)
            .map_err(|error| format!("R1 control input: {error:?}"))?;
        let merged = self
            .kernel
            .pop()
            .ok_or_else(|| "R1 control kernel lost admitted input".to_string())?;
        if merged.signal.sequence != self.session.next_sequence() {
            return Err("R1 control merge sequence disagrees with Session".into());
        }
        let sequence = merged.signal.sequence;
        let payload = encode_signal_fixed(&merged.signal);
        self.in_flight = Some(merged);
        Ok((sequence, payload))
    }

    pub fn pressure(&mut self, sequence: u64) -> Result<(), String> {
        if self.in_flight_sequence() != Some(sequence) {
            return Err("R1 control pressure has the wrong in-flight sequence".into());
        }
        self.pressure_retries = self.pressure_retries.saturating_add(1);
        Ok(())
    }

    pub fn delivered(&mut self, sequence: u64) -> Result<R1MergedInput, String> {
        if self.in_flight_sequence() != Some(sequence) {
            return Err("R1 control delivery has the wrong in-flight sequence".into());
        }
        self.in_flight
            .take()
            .ok_or_else(|| "R1 control delivery lost its input".to_string())
    }

    pub fn final_sequence(&self) -> Result<u64, String> {
        if self.in_flight.is_some() || self.kernel.pending() != 0 {
            return Err("R1 control source retained input at terminal".into());
        }
        Ok(self.session.next_sequence())
    }

    pub fn admit_outbound(&mut self, frame: SessionFrame<'_>) -> Result<(), String> {
        self.session.admit_outbound(frame).map_err(debug_error)
    }

    pub fn admit_inbound(&mut self, frame: SessionFrame<'_>) -> Result<(), String> {
        self.session.admit_inbound(frame).map_err(debug_error)
    }

    pub fn is_active(&self) -> bool {
        self.session.is_active()
    }

    pub fn is_terminal(&self) -> bool {
        self.session.is_terminal()
    }

    pub fn pressure_retries(&self) -> u32 {
        self.pressure_retries
    }

    fn in_flight_sequence(&self) -> Option<u64> {
        self.in_flight.as_ref().map(|merged| merged.signal.sequence)
    }
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use conduit_core::{BootId, HostId};
    use conduit_wire::{SessionMessage, SessionTerminalDisposition};

    use super::*;
    use crate::r1_control::R1ControlPeer;

    fn exchange(
        source: &mut PicoControlSource,
        sink: &mut SessionMachine,
        outbound: SessionFrame<'_>,
        response: SessionFrame<'_>,
    ) {
        source.admit_outbound(outbound).unwrap();
        sink.admit_inbound(outbound).unwrap();
        sink.admit_outbound(response).unwrap();
        source.admit_inbound(response).unwrap();
    }

    #[test]
    fn six_deliberate_inputs_cross_the_exact_control_session_with_pressure() {
        let exact = conduit_system_continuity::exact_r1_control_plan(
            BootId::from(conduit_r1_network_conformance::R1_PICO_BOOT_ID),
            conduit_system_continuity::R1SignalRouteSet::UsbOnly,
        )
        .unwrap();
        let mut source = PicoControlSource::prepare_plan(
            exact.plan,
            &HostId::from(conduit_r1_network_conformance::R1_STD_HOST_ID),
        )
        .unwrap();
        let binding = source.binding().clone();
        let mut sink = SessionMachine::new(binding.clone(), SessionRole::Sink).unwrap();
        exchange(
            &mut source,
            &mut sink,
            binding.hello_frame(),
            binding.hello_frame(),
        );
        exchange(
            &mut source,
            &mut sink,
            binding.frame(SessionMessage::Ready),
            binding.frame(SessionMessage::Ready),
        );

        let events = [
            R1ControlPeer::Terminal,
            R1ControlPeer::BrowserA,
            R1ControlPeer::BrowserB,
        ]
        .into_iter()
        .flat_map(|peer| [(peer, 0, true), (peer, 1, false)]);
        for (expected_sequence, (peer, peer_sequence, level)) in events.enumerate() {
            let (sequence, payload) = source
                .offer_input(R1InputEvent {
                    peer,
                    peer_sequence,
                    level,
                })
                .unwrap();
            assert_eq!(sequence, expected_sequence as u64);
            let offered = binding.frame(SessionMessage::Offered {
                sequence,
                payload: &payload,
            });
            source.admit_outbound(offered).unwrap();
            sink.admit_inbound(offered).unwrap();
            if sequence == 0 {
                let pressure = binding.frame(SessionMessage::Pressure { sequence });
                sink.admit_outbound(pressure).unwrap();
                source.admit_inbound(pressure).unwrap();
                source.pressure(sequence).unwrap();
                source.admit_outbound(offered).unwrap();
                sink.admit_inbound(offered).unwrap();
            }
            let accepted = binding.frame(SessionMessage::Accepted { sequence });
            sink.admit_outbound(accepted).unwrap();
            source.admit_inbound(accepted).unwrap();
            let delivered = binding.frame(SessionMessage::Delivered { sequence });
            sink.admit_outbound(delivered).unwrap();
            source.admit_inbound(delivered).unwrap();
            let merged = source.delivered(sequence).unwrap();
            assert_eq!(merged.input.peer, peer);
            assert_eq!(merged.signal.level, level);
        }
        let final_sequence = source.final_sequence().unwrap();
        assert_eq!(final_sequence, 6);
        exchange(
            &mut source,
            &mut sink,
            binding.frame(SessionMessage::InputClosed { final_sequence }),
            binding.frame(SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence,
            }),
        );
        let terminal = binding.frame(SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence,
        });
        source.admit_outbound(terminal).unwrap();
        sink.admit_inbound(terminal).unwrap();
        assert!(source.is_terminal());
        assert!(sink.is_terminal());
        assert_eq!(source.pressure_retries(), 1);
    }

    #[test]
    fn pulse_source_and_control_source_cannot_masquerade_as_each_other() {
        let exact = conduit_system_continuity::exact_r1_control_plan(
            BootId::from(conduit_r1_network_conformance::R1_PICO_BOOT_ID),
            conduit_system_continuity::R1SignalRouteSet::UsbOnly,
        )
        .unwrap();
        assert!(crate::pico_usb_source::PicoUsbSource::prepare_plan(
            exact.plan.clone(),
            &HostId::from(conduit_r1_network_conformance::R1_STD_HOST_ID),
        )
        .is_err());
        assert!(PicoControlSource::prepare_plan(
            exact.plan,
            &HostId::from(conduit_r1_network_conformance::R1_STD_HOST_ID),
        )
        .is_ok());
    }
}
