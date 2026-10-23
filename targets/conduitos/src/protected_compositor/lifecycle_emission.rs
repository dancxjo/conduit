//! Four finite first-occurrence product witnesses, outside measured intervals.
//! No rendering, requests, retry, admission or independent lifecycle truth.
use super::DisplayOwner;
use crate::protected_compositor::lifecycle::sign::*;

impl DisplayOwner {
    /// Bind actual selected immutable Make metadata once. This does not renew
    /// the provider scope or claim that the binding is the final ISO digest.
    pub(crate) fn bind_lifecycle_make(&mut self, make: crate::make::MakeRecord) {
        if self.observation_make.is_none() {
            self.observation_make = Some(make);
        }
    }

    pub(crate) fn select_lifecycle_checkpoint(&mut self, checkpoint: Checkpoint) -> bool {
        let bit = 1u8 << checkpoint.ordinal();
        if self.observation_seen & bit != 0 {
            return false;
        }
        self.observation_seen |= bit;
        true
    }

    pub(crate) fn emit_prepared_lifecycle(&mut self) {
        if self.select_lifecycle_checkpoint(Checkpoint::Prepared) {
            self.emit_selected_lifecycle(Checkpoint::Prepared, None);
        }
    }

    pub(crate) fn emit_missing_lifecycle_ack(&self, checkpoint: Checkpoint) {
        let mut buffer = [0; CAPACITY];
        let encoded =
            encode_missing_ack(self.lifecycle_position(checkpoint, None, 0, 6), &mut buffer);
        crate::arch::early_write(&buffer[..encoded.length]);
    }

    fn lifecycle_position(
        &self,
        checkpoint: Checkpoint,
        ack: Option<AckRef>,
        part: u8,
        parts: u8,
    ) -> Position {
        let instance = self.sessions.service_instance();
        Position {
            preparation: instance,
            checkpoint: checkpoint.ordinal(),
            part,
            parts,
            instance,
            namespace: ack.map(|ack| ack.namespace),
        }
    }

    /// Capture the already-completed operations before serialization/serial I/O.
    /// Warm Before/After pairs retain independent activation and request samples.
    pub(crate) fn emit_selected_lifecycle(&self, checkpoint: Checkpoint, ack: Option<AckRef>) {
        let Some(make) = self.observation_make.as_ref() else {
            self.emit_missing_lifecycle_ack(checkpoint);
            return;
        };
        let observed = self.lifecycle_observation();
        let prepared = matches!(checkpoint, Checkpoint::Prepared);
        if !prepared && ack.is_none() {
            self.emit_missing_lifecycle_ack(checkpoint);
            return;
        }
        let identity = Identity::borrowed(self.display_identity(), make, self.guest_image_sha256());
        let initialization = observed.service.initialization;
        let activation = observed.activations.attempts;
        let requests = observed.service.requests;
        let parts = if prepared { 5 } else { 6 };
        let mut buffer = [0; CAPACITY];
        for part in 0..parts {
            let (stage, kind, phase, sample, cost) = if prepared {
                match part {
                    0 => (Stage::Preparation, Kind::Context, None, None, None),
                    1 => (
                        Stage::Preparation,
                        Kind::Installed,
                        None,
                        None,
                        Some(observed.service.installed),
                    ),
                    2 => (
                        Stage::Preparation,
                        Kind::Before,
                        None,
                        initialization,
                        initialization.and_then(|sample| sample.before),
                    ),
                    3 => (
                        Stage::Preparation,
                        Kind::After,
                        None,
                        initialization,
                        initialization.and_then(|sample| sample.after),
                    ),
                    _ => (
                        Stage::Preparation,
                        Kind::Current,
                        None,
                        None,
                        Some(observed.current_cost),
                    ),
                }
            } else {
                match part {
                    0 => (Stage::Activation, Kind::Context, None, None, None),
                    1 => (
                        Stage::Activation,
                        Kind::Before,
                        Some(activation),
                        activation.last,
                        activation.last.and_then(|sample| sample.before),
                    ),
                    2 => (
                        Stage::Activation,
                        Kind::After,
                        Some(activation),
                        activation.last,
                        activation.last.and_then(|sample| sample.after),
                    ),
                    3 => (
                        Stage::Request,
                        Kind::Before,
                        Some(requests),
                        requests.last,
                        requests.last.and_then(|sample| sample.before),
                    ),
                    4 => (
                        Stage::Request,
                        Kind::After,
                        Some(requests),
                        requests.last,
                        requests.last.and_then(|sample| sample.after),
                    ),
                    _ => (
                        Stage::Request,
                        Kind::Current,
                        None,
                        None,
                        Some(observed.current_cost),
                    ),
                }
            };
            let record = Snapshot {
                checkpoint,
                ack,
                position: self.lifecycle_position(checkpoint, ack, part, parts),
                stage,
                kind,
                identity,
                phase: phase.map(PhaseMetadata::from),
                sample: sample.map(SampleMetadata::from),
                cost: cost.as_ref().map(Cost),
                staging_prepared_bytes: observed.staging_prepared_bytes as u64,
                staging_live_bytes: observed.staging_live_bytes as u64,
                installation_raw_ticks: observed.service.installation_raw_ticks,
                staging_preparation_raw_ticks: observed.staging_preparation_raw_ticks,
                preparation_raw_ticks: observed.preparation_raw_ticks,
                disposal_raw_ticks: observed.disposal_raw_ticks,
            };
            let encoded = encode(&record, &mut buffer);
            crate::arch::early_write(&buffer[..encoded.length]);
            if !encoded.complete {
                break;
            }
        }
    }
}
