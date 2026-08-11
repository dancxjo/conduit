use super::{HostedRawMidiSelection, MidiEndpointDirection, MidiOutputSelection};
use crate::{StdHost, StdHostComposition, StdHostConfig};

impl StdHost {
    pub fn new_with_raw_midi_input(
        config: StdHostConfig,
        composition: StdHostComposition,
        midi_input: HostedRawMidiSelection,
    ) -> Result<Self, String> {
        if midi_input.boot_id() != &config.boot_id
            || midi_input.offer_generation() != config.offer_generation
            || midi_input.observation().direction != MidiEndpointDirection::ReadableSource
            || midi_input.observation().direct_device_path().is_none()
        {
            return Err(
                "raw MIDI input observation does not match direction, Boot, generation, or direct node"
                    .into(),
            );
        }
        let advertisement = crate::composition::build_advertisement(
            config,
            composition,
            None,
            Some(&midi_input),
            None,
            false,
        );
        let kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            playback: None,
            midi_input: Some(midi_input),
            midi_output: None,
            kernel_resources,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
        })
    }

    pub fn raw_midi_input_selection(&self) -> Option<&HostedRawMidiSelection> {
        self.midi_input.as_ref()
    }

    /// Constructs the typed grant for a caller that has independently
    /// authorized this exact selected RawMIDI input. Discovery and selection
    /// never imply this authority.
    pub fn midi_input_authority_grant(
        &self,
        grant_id: &str,
    ) -> Result<conduit_core::AuthorityGrant, String> {
        let selected = self
            .midi_input
            .as_ref()
            .ok_or_else(|| "std Host has no selected MIDI input resource".to_string())?;
        if selected.boot_id() != &self.advertisement.boot_id
            || selected.offer_generation() != self.advertisement.offer_generation
        {
            return Err("selected MIDI input observation is stale for this Host".into());
        }
        let capability = self
            .advertisement
            .capabilities
            .iter()
            .find(|offer| {
                offer.implementation.implementation_id.as_str()
                    == conduit_std_catalog::MUSIC_INPUT_MIDI_IMPLEMENTATION
            })
            .ok_or_else(|| "selected MIDI input capability is not advertised".to_string())?;
        let requirement = capability
            .authority_requirements
            .first()
            .ok_or_else(|| "MIDI input capability has no authority contract".to_string())?;
        if capability.authority_requirements.len() != 1 {
            return Err("MIDI input capability authority shape changed".into());
        }
        Ok(conduit_core::AuthorityGrant {
            grant_id: conduit_core::AuthorityGrantId::from(grant_id),
            contract_id: requirement.contract_id.clone(),
            host_operation_contract_id: requirement.host_operation_contract_id.clone(),
            subject_kind: requirement.subject_kind.clone(),
            host_id: self.advertisement.host_id.clone(),
            boot_id: self.advertisement.boot_id.clone(),
            capability_id: capability.capability_id.clone(),
        })
    }

    pub fn new_with_raw_midi_output(
        config: StdHostConfig,
        composition: StdHostComposition,
        midi_output: HostedRawMidiSelection,
    ) -> Result<Self, String> {
        if midi_output.boot_id() != &config.boot_id
            || midi_output.offer_generation() != config.offer_generation
            || midi_output.observation().direction != MidiEndpointDirection::WritableDestination
            || midi_output.observation().direct_device_path().is_none()
        {
            return Err(
                "raw MIDI output observation does not match direction, Boot, generation, or direct node"
                    .into(),
            );
        }
        let midi_output = MidiOutputSelection::raw(midi_output);
        let advertisement = crate::composition::build_advertisement(
            config,
            composition,
            None,
            None,
            Some(&midi_output),
            false,
        );
        let kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            playback: None,
            midi_input: None,
            midi_output: Some(midi_output),
            kernel_resources,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
        })
    }

    pub fn raw_midi_output_selection(&self) -> Option<&HostedRawMidiSelection> {
        self.midi_output
            .as_ref()
            .and_then(MidiOutputSelection::as_raw)
    }
}
