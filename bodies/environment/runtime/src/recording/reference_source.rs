//! Finite metadata source for Root-selected immutable recording generations.
//! Portable references are data, never storage authority or a read receipt.
use super::MAXIMUM_CHUNK_BYTES;
use alloc::{vec, vec::Vec};
use conduit_core as c;
use conduit_kernel::{scheduler::*, Failure, FailureCode, PortId};

pub fn generation_source_kind() -> c::Kind {
    c::Kind {
        kind_id: "environment/recording-generations".into(),
        kind_contract_revision: "conduit.environment/recording-generations@1".into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![],
        outputs: vec![c::PortDescriptor {
            port_id: "generation".into(),
            value_kind: c::RESOURCE_REFERENCE_INFO_ID.into(),
            direction: c::PortDirection::Output,
            temporal: c::PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        configuration: vec![],
        semantic_laws: vec![],
        limits: c::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: c::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
        },
    }
}
/// One selected generation is a Value, suitable for an exact snapshot read.
pub fn generation_value_kind() -> c::Kind {
    let mut kind = generation_source_kind();
    kind.kind_id = "environment/recording-generation".into();
    kind.kind_contract_revision = "conduit.environment/recording-generation@1".into();
    kind.outputs[0].temporal = c::PortTemporal::Value;
    kind.semantic_laws = alloc::vec![c::KindSemanticLaw::Terminal(
        c::KindTerminalBehavior::EmitsOnce
    )];
    kind
}
pub fn install_generation_value_kind(
    startup: &mut conduit_plot::StartupCatalog,
    profiles: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind = generation_value_kind();
    startup.insert(conduit_plot::KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: vec![],
    })?;
    startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
    profiles
        .insert_kind(kind)
        .map_err(|error| alloc::format!("{error:?}"))
}
#[derive(Debug, PartialEq, Eq)]
pub enum RecordingReferenceRefusal {
    Capacity,
    Reference(c::ResourceReferenceRefusal),
    Profile,
    Duplicate,
}
pub struct RecordingReferenceSource {
    references: Vec<Vec<u8>>,
    cursor: usize,
    staged: bool,
    cancelled: bool,
}
impl RecordingReferenceSource {
    /// Root supplies its admitted ordered generation list and a finite ceiling.
    /// No reference is minted or looked up, and all encoded output is reserved
    /// here. Empty lists close normally without inventing a generation.
    pub fn prepare(
        references: &[c::BoundedResourceRef],
        maximum_generations: u16,
    ) -> Result<Self, RecordingReferenceRefusal> {
        use RecordingReferenceRefusal as E;
        if maximum_generations == 0 || references.len() > usize::from(maximum_generations) {
            return Err(E::Capacity);
        }
        for (index, reference) in references.iter().enumerate() {
            reference.validate().map_err(E::Reference)?;
            if reference.content_profile.as_str() != "text/json-utf8@1"
                || reference.access_class.as_str() != "resource/snapshot@1"
                || reference.extent.items != Some(1)
                || reference.extent.bytes == 0
                || reference.extent.bytes > MAXIMUM_CHUNK_BYTES as u64
            {
                return Err(E::Profile);
            }
            if references[..index].iter().any(|previous| {
                previous.identity == reference.identity
                    && previous.lifetime.version == reference.lifetime.version
            }) {
                return Err(E::Duplicate);
            }
        }
        let references = references
            .iter()
            .map(|reference| reference.encode().map_err(E::Reference))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            references,
            cursor: 0,
            staged: false,
            cancelled: false,
        })
    }
}
impl<const P: usize> StepBack<P> for RecordingReferenceSource {
    fn step(&mut self, io: &mut StepIo<P>, _: &StepInputBytes<'_, P>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 1,
            });
        }
        let Some(reference) = self.references.get(self.cursor) else {
            return StepOutcome::Complete;
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.send_prepared(PortId(0), reference.len() as u32).is_err() {
            return StepOutcome::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 2,
            });
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0))
            .then(|| self.references.get(self.cursor).map(Vec::as_slice))
            .flatten()
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.cursor += 1;
            self.staged = false;
        }
    }
    fn cancel(&mut self) {
        self.staged = false;
        self.cancelled = true;
    }
}

/// Register only authored meaning; no reference data, provider or access is made.
pub fn install_generation_source_kind(
    startup: &mut conduit_plot::StartupCatalog,
    profiles: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let kind = generation_source_kind();
    startup.insert(conduit_plot::KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: vec![],
    })?;
    startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
    profiles
        .insert_kind(kind)
        .map_err(|error| alloc::format!("{error:?}"))
}
