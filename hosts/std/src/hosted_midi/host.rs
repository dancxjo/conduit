use super::{HostedRawMidiSelection, MidiEndpointDirection, MidiOutputSelection};
use crate::{StdHost, StdHostComposition, StdHostConfig};

impl StdHost {
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
            Some(&midi_output),
            false,
        );
        let kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            playback: None,
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
