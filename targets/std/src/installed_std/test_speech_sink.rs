use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, ImplementationId, KindContractRevision, PlannedGear, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_form::{KindDefinition, ProfileCatalog};
use conduit_kernel::{OperationAction, OperationInput, PortId};

const KIND: &str = "conduit-test/speech-pcm-sink";
const REVISION: &str = "conduit-test/speech-pcm-sink@1";
const PROFILE: &str = "conduit-test/speech-pcm-sink-kernel@1";
pub(super) const IMPLEMENTATION: &str = "conduit-test/speech-pcm-sink@1";
const ARTIFACT: &str = "conduit-std-host/test-speech-pcm-sink@1";
const EXPECTED_BLOCKS: u8 = 3;

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TestSpeechSinkOperation {
    blocks: u8,
}

impl TestSpeechSinkOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        let OperationInput::Value {
            port: PortId(0),
            value,
        } = input
        else {
            return InstalledOperation::fail(180);
        };
        if value.byte_len > conduit_std_offers::PIPER_PCM_BLOCK_BYTES {
            return InstalledOperation::fail(181);
        }
        self.blocks = self.blocks.saturating_add(1);
        if self.blocks == EXPECTED_BLOCKS {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }
}

pub(super) fn offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("test-speech-pcm-sink"),
        kind_id: kind_id(KIND),
        kind_contract_revision: KindContractRevision::from(REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: inputs(),
        outputs: Vec::new(),
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: EXPECTED_BLOCKS.into(),
            max_queue_bytes: u32::from(EXPECTED_BLOCKS) * conduit_std_offers::PIPER_PCM_BLOCK_BYTES,
        },
    }
}

pub(super) fn install_catalog(catalog: &mut ProfileCatalog) {
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(KIND),
            kind_contract_revision: KindContractRevision::from(REVISION),
            inputs: inputs(),
            outputs: Vec::new(),
            configuration: Vec::new(),
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
        || !placement.host_operations.is_empty()
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
        maximum_value_bytes: conduit_std_offers::PIPER_PCM_BLOCK_BYTES,
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
