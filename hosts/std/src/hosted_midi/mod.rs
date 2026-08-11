mod discovery;

pub use discovery::{
    discover_alsa_sequencer_endpoints, MidiDiscoveryError, MidiEndpointDirection,
    MidiEndpointObservation,
};

use conduit_core::{BootId, OfferGeneration, ResourcePoolId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedMidiSelection {
    observation: MidiEndpointObservation,
    boot_id: BootId,
    offer_generation: OfferGeneration,
}

impl HostedMidiSelection {
    pub fn select(
        observations: &[MidiEndpointObservation],
        direction: MidiEndpointDirection,
        client: u16,
        port: u16,
        boot_id: BootId,
        offer_generation: OfferGeneration,
    ) -> Result<Self, String> {
        let observation = observations
            .iter()
            .find(|observation| {
                observation.direction == direction
                    && observation.client == client
                    && observation.port == port
            })
            .cloned()
            .ok_or_else(|| {
                format!(
                    "fresh MIDI {:?} endpoint {client}:{port} is absent",
                    direction
                )
            })?;
        Ok(Self {
            observation,
            boot_id,
            offer_generation,
        })
    }

    pub const fn observation(&self) -> &MidiEndpointObservation {
        &self.observation
    }

    pub fn sequencer_address(&self) -> String {
        format!("{}:{}", self.observation.client, self.observation.port)
    }

    pub fn resource_pool_id(&self) -> ResourcePoolId {
        ResourcePoolId::from(format!(
            "std/midi/alsa-seq/{}/{}/{}/client-{}/port-{}",
            self.boot_id.as_str(),
            self.offer_generation.0,
            self.observation.direction.identity_segment(),
            self.observation.client,
            self.observation.port,
        ))
    }

    pub const fn boot_id(&self) -> &BootId {
        &self.boot_id
    }

    pub const fn offer_generation(&self) -> OfferGeneration {
        self.offer_generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_is_exact_directional_and_boot_scoped() {
        let observations = [
            MidiEndpointObservation {
                client: 20,
                port: 1,
                client_name: "Controller".into(),
                port_name: "Port".into(),
                client_type: "kernel".into(),
                direction: MidiEndpointDirection::ReadableSource,
            },
            MidiEndpointObservation {
                client: 20,
                port: 1,
                client_name: "Controller".into(),
                port_name: "Port".into(),
                client_type: "kernel".into(),
                direction: MidiEndpointDirection::WritableDestination,
            },
        ];
        let selected = HostedMidiSelection::select(
            &observations,
            MidiEndpointDirection::ReadableSource,
            20,
            1,
            BootId::from("boot-a"),
            OfferGeneration(4),
        )
        .unwrap();
        assert_eq!(selected.sequencer_address(), "20:1");
        assert_eq!(
            selected.resource_pool_id().as_str(),
            "std/midi/alsa-seq/boot-a/4/readable-source/client-20/port-1"
        );
        assert!(HostedMidiSelection::select(
            &observations,
            MidiEndpointDirection::WritableDestination,
            20,
            2,
            BootId::from("boot-a"),
            OfferGeneration(4),
        )
        .is_err());
    }
}
