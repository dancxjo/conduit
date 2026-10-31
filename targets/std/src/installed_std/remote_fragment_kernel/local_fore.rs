//! Existing local Fore crossings on the same ordinary admitted kernel.
//! No transport, Host identity, provider, or authority is constructed here.
use super::*;
use conduit_core::{ConnectionTrack, PortDirection};
impl InstalledRemoteFragment {
    pub fn admit_local_fore_input(
        &mut self,
        port: &conduit_core::PortId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, String> {
        let selected = self
            .lowered
            .fore_ports
            .iter()
            .filter(|binding| {
                &binding.front_port_id == port
                    && binding.direction == PortDirection::Input
                    && binding.track == ConnectionTrack::Payload
            })
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err("local Fore input absent from the exact admitted Plan".into());
        }
        let mut targets = Vec::with_capacity(selected.len());
        for binding in selected {
            binding
                .validate_value(bytes)
                .map_err(|error| format!("local Fore value refused: {error:?}"))?;
            targets.push((binding.endpoint, binding.cord));
        }
        self.scheduler
            .admit_remote_input_fanout(&targets, sequence, bytes)
            .map_err(|error| format!("admit local Fore value: {error:?}"))
    }
    pub fn close_local_fore_input(&mut self, port: &conduit_core::PortId) -> Result<(), String> {
        let selected = self
            .lowered
            .fore_ports
            .iter()
            .filter(|binding| {
                &binding.front_port_id == port
                    && binding.direction == PortDirection::Input
                    && binding.track == ConnectionTrack::Payload
            })
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err("local Fore input absent from the exact admitted Plan".into());
        }
        for binding in selected {
            self.scheduler
                .close_remote_input(binding.endpoint, binding.cord)
                .map_err(|error| format!("close local Fore: {error:?}"))?;
        }
        Ok(())
    }
    /// Local consumption of the exact typed output, with original kernel delivery
    /// semantics. The bytes are not a signed MCU sample or membership receipt.
    pub fn take_local_fore_output(
        &mut self,
        port: &conduit_core::PortId,
    ) -> Result<Option<Vec<u8>>, String> {
        let selected = self
            .lowered
            .fore_ports
            .iter()
            .filter(|binding| {
                &binding.front_port_id == port
                    && binding.direction == PortDirection::Output
                    && binding.track == ConnectionTrack::Payload
            })
            .collect::<Vec<_>>();
        let [binding] = selected.as_slice() else {
            return Err("local Fore output must select one exact planned edge".into());
        };
        let Some(offer) = self
            .scheduler
            .remote_egress_offer(binding.endpoint, binding.cord)
            .map_err(|error| format!("local Fore offer: {error:?}"))?
        else {
            return Ok(None);
        };
        let bytes = self
            .scheduler
            .host_value(offer.value)
            .map_err(|error| format!("local Fore value: {error:?}"))?
            .to_vec();
        binding
            .validate_value(&bytes)
            .map_err(|error| format!("local Fore output refused: {error:?}"))?;
        self.scheduler
            .remote_egress_accept(binding.endpoint, binding.cord, offer.sequence)
            .map_err(|error| format!("local Fore accept: {error:?}"))?;
        self.scheduler
            .remote_egress_delivered(binding.endpoint, binding.cord, offer.sequence)
            .map_err(|error| format!("local Fore delivery: {error:?}"))?;
        Ok(Some(bytes))
    }
}
