use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, ImplementationId, KindContractRevision, PcmChannelLayout, PcmFrameHeader,
    PcmSampleRepresentation, PlannedGear, PortDescriptor, PortDirection, PortTemporal,
    AUDIO_PCM_INFO_ID,
};
use conduit_form::{KindDefinition, ProfileCatalog};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationInput, RequestId,
};
use conduit_kernel::{OperationAction, PortId, ValueRef, ValueStorage};

pub(super) const KIND: &str = "conduit-proof/pcm-specimen-source";
const REVISION: &str = "conduit-proof/pcm-specimen-source@1";
const PROFILE: &str = "conduit-proof/pcm-specimen-source-kernel@1";
pub(super) const IMPLEMENTATION: &str = "conduit-proof/pcm-specimen-source-kernel@1";
const ARTIFACT: &str = "conduit-std-host/proof-pcm-specimen-source@1";
const BLOCKS: u16 = 96;
const YIELDS: usize = BLOCKS as usize - 1;
pub(super) const YIELD_OPERATION: &str = "conduit-proof/audio-source-yield@1";

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TestPcmSourceOperation {
    pub(super) values: [ValueRef; BLOCKS as usize],
    yield_markers: [ValueRef; YIELDS],
    pub(super) next: usize,
    pending: Option<RequestId>,
}

impl TestPcmSourceOperation {
    pub(super) fn emit_or_complete(&self) -> OperationAction {
        self.values
            .get(self.next)
            .copied()
            .map_or(OperationAction::Complete, |value| OperationAction::Emit {
                port: PortId(0),
                value,
            })
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.emit_or_complete()
            }
            _ => InstalledOperation::fail(64),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        self.next += 1;
        if self.next >= self.values.len() {
            return OperationAction::Complete;
        }
        let request = RequestId(self.next as u32);
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(self.yield_markers[self.next - 1], 1)
                .expect("yield marker is one byte"),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

pub(super) fn offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("test-pcm-source"),
        kind_id: kind_id(KIND),
        kind_contract_revision: KindContractRevision::from(REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: outputs(),
        host_operations: vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(YIELD_OPERATION),
            target_kind: None,
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: 0,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: conduit_std_catalog::MAXIMUM_AUDIO_QUEUE_ITEMS,
            max_queue_bytes: conduit_std_catalog::MAXIMUM_AUDIO_QUEUE_BYTES,
        },
    }
}

pub(super) fn install_catalog(catalog: &mut ProfileCatalog) {
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(KIND),
            kind_contract_revision: KindContractRevision::from(REVISION),
            inputs: Vec::new(),
            outputs: outputs(),
            configuration: Vec::new(),
        })
        .expect("test PCM source kind is unique");
}

fn outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id("audio"),
        value_kind: kind_id(AUDIO_PCM_INFO_ID),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
    }]
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != KIND
        || placement.kind_contract_revision.as_str() != REVISION
        || placement.execution_profile_id.as_str() != PROFILE
        || placement.implementation_id.as_str() != IMPLEMENTATION
        || placement.artifact_id.as_str() != ARTIFACT
        || !placement.inputs.is_empty()
        || placement.outputs != outputs()
        || placement.host_operations != offer().host_operations
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.configuration.is_empty()
    {
        return Err("planned test PCM source does not match its fixture".to_string());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: BLOCKS + YIELDS as u16,
        value_bytes: BLOCKS as u32 * conduit_std_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES
            + YIELDS as u32,
        host_requests: BLOCKS as usize - 1,
        sign_items: 1_024,
        maximum_value_bytes: conduit_std_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let stored: [Result<ValueRef, String>; BLOCKS as usize] = core::array::from_fn(|index| {
        let frame_count = conduit_std_catalog::AUDIO_PLAY_ALSA_PERIOD_FRAMES;
        let start_frame = index as u64 * u64::from(frame_count);
        let header = PcmFrameHeader::new(
            PcmSampleRepresentation::Signed16LittleEndian,
            crate::hosted_audio::SAMPLE_RATE_HZ,
            PcmChannelLayout::StereoLeftRight,
            frame_count,
            crate::hosted_audio::SOURCE_CLOCK_ID,
            start_frame,
            false,
        )
        .expect("fixture PCM header is exact");
        let mut payload = vec![0_u8; usize::try_from(header.payload_bytes).unwrap()];
        for (sample_index, encoded) in payload.as_chunks_mut::<2>().0.iter_mut().enumerate() {
            let phase = (sample_index / 2 + index * usize::from(frame_count)) % 48;
            let sample = if phase < 24 { 2_000_i16 } else { -2_000_i16 };
            encoded.copy_from_slice(&sample.to_le_bytes());
        }
        let encoded = header
            .encode_frame(&payload)
            .expect("fixture PCM is canonical");
        values
            .store(&encoded)
            .map_err(|error| format!("store test PCM block: {error:?}"))
    });
    let stored = stored.into_iter().collect::<Result<Vec<_>, _>>()?;
    let yield_markers: [Result<ValueRef, String>; YIELDS] = core::array::from_fn(|_| {
        values
            .store(&[0])
            .map_err(|error| format!("store test PCM yield marker: {error:?}"))
    });
    let yield_markers = yield_markers
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "test PCM yield count changed")?;
    Ok(InstalledOperation::TestPcmSource(Box::new(
        TestPcmSourceOperation {
            values: stored
                .try_into()
                .map_err(|_| "test PCM block count changed")?,
            yield_markers,
            next: 0,
            pending: None,
        },
    )))
}
