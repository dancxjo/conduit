use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, ImplementationId, KindIdentity, PlannedGear, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_form::{KindProjection, ProfileCatalog};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    Failure, FailureCode, PortId,
};

pub(crate) const KIND: &str = "conduit-proof/speech-pcm-sink";
const REVISION: &str = "conduit-proof/speech-pcm-sink@1";
const PROFILE: &str = "conduit-proof/speech-pcm-sink-kernel@1";
pub(super) const IMPLEMENTATION: &str = "conduit-proof/speech-pcm-sink@1";
const ARTIFACT: &str = "conduit-std-host/proof-speech-pcm-sink@1";

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TestSpeechSinkOperation {
    blocks: u16,
}

impl<const PORTS: usize> StepOperation<PORTS> for TestSpeechSinkOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            if value.byte_len > conduit_std_offers::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BYTES
                || self.blocks >= conduit_std_offers::PIPER_MAXIMUM_BLOCKS
            {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidLifecycle,
                    detail: 180,
                });
            }
            io.consume(PortId(0)).expect("present test speech block");
            self.blocks += 1;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.blocks != 0 {
            io.consume_closed(PortId(0))
                .expect("observed test speech closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

impl TestSpeechSinkOperation {}

pub(crate) fn offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("proof-speech-pcm-sink"),
        kind_id: kind_id(KIND),
        kind_contract_revision: KindIdentity::from(REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: inputs(),
        outputs: Vec::new(),
        host_calls: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: conduit_std_offers::PIPER_MAXIMUM_BLOCKS,
            max_queue_bytes: u32::from(conduit_std_offers::PIPER_MAXIMUM_BLOCKS)
                * conduit_std_offers::PIPER_PCM_BLOCK_BYTES,
        },
    }
}

pub(crate) fn install_catalog(catalog: &mut ProfileCatalog) {
    catalog
        .insert(KindProjection {
            kind_id: kind_id(KIND),
            kind_contract_revision: KindIdentity::from(REVISION),
            inputs: inputs(),
            outputs: Vec::new(),
            configuration: Default::default(),
        })
        .expect("test speech sink is unique");
}

fn inputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("audio"),
        value_kind: kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
    }]
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id.as_str() != PROFILE
        || placement.implementation_id.as_str() != IMPLEMENTATION
        || placement.artifact_id.as_str() != ARTIFACT
        || placement.inputs != offer.inputs
        || !placement.outputs.is_empty()
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.configuration.is_empty()
    {
        return Err("planned test speech sink differs from installation".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 16,
        maximum_value_bytes: conduit_std_offers::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BYTES,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::TestSpeechSink(
        TestSpeechSinkOperation { blocks: 0 },
    ))
}
