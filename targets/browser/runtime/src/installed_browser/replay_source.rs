//! Browser realization of the reusable finite history-to-replay projection.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision, PlannedGear, StructuredInfoType,
};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};

pub(crate) const HOST_OPERATION: &str = "conduit.host/browser-replay-source@1";
const IMPLEMENTATION: &str = "browser/bounded-replay-source@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

pub(crate) struct PreparedReplaySource {
    timeline_type: Vec<u8>,
    replay_type: Vec<u8>,
    replay: Vec<u8>,
    gap: Vec<u8>,
    canonical: Vec<u8>,
}

impl PreparedReplaySource {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        if placement.implementation_id.as_str() != IMPLEMENTATION {
            return Ok(None);
        }
        validate_placement(placement, &offer())?;
        Ok(Some(Self {
            timeline_type: leaf_type("history/typed-timeline@1")?,
            replay_type: leaf_type("history/replay-timeline@1")?,
            replay: vec![0; conduit_time::MAXIMUM_REPLAY_TIMELINE_BYTES],
            gap: vec![0; conduit_time::HISTORICAL_RETENTION_GAP_BYTES],
            canonical: Vec::with_capacity(super::MAXIMUM_BROWSER_VALUE_BYTES),
        }))
    }

    pub(crate) fn execute<'a>(&'a mut self, canonical: &[u8]) -> Result<&'a [u8], Failure> {
        let timeline = exact_leaf(canonical, &self.timeline_type)
            .ok_or_else(|| failure(FailureCode::InvalidInput, 1))?;
        let timeline = conduit_time::decode_historical_timeline(timeline)
            .map_err(|_| failure(FailureCode::InvalidInput, 2))?;
        let result = conduit_time::BoundedReplaySourceOperation::new(&timeline)
            .project_into(&mut self.replay, &mut self.gap)
            .map_err(|_| failure(FailureCode::InvalidInput, 3))?;
        if result.gap_bytes.is_some() {
            return Err(failure(FailureCode::InvalidInput, 4));
        }
        wrap_leaf(
            &self.replay_type,
            &self.replay[..result.replay_bytes],
            &mut self.canonical,
        )?;
        Ok(&self.canonical)
    }
}

fn offer() -> CapabilityOffer {
    let definition = conduit_time::replay_source_kind_definition();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(IMPLEMENTATION),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(
            conduit_time::REPLAY_SOURCE_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-time/bounded-replay-source@1"),
        },
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(HOST_OPERATION),
            target_kind: Some(definition.kind_id),
            maximum_in_flight: 1,
            maximum_input_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
            maximum_output_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: (super::MAXIMUM_BROWSER_VALUE_BYTES * 2) as u32,
        },
    }
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    PreparedReplaySource::for_placement(placement)?
        .ok_or_else(|| "replay source placement selected another implementation".to_string())?;
    Ok(BrowserOperation::unary(
        super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        1,
    ))
}

fn leaf_type(identity: &str) -> Result<Vec<u8>, String> {
    StructuredInfoType::leaf(conduit_core::kind_id(identity))
        .map_err(|error| format!("replay source type: {error:?}"))?
        .canonical_bytes()
        .map_err(|error| format!("replay source type bytes: {error:?}"))
}

fn exact_leaf<'a>(canonical: &'a [u8], value_type: &[u8]) -> Option<&'a [u8]> {
    let node = canonical.strip_prefix(value_type)?;
    if node.first() != Some(&0) || node.len() < 5 {
        return None;
    }
    let length = usize::try_from(u32::from_le_bytes(node[1..5].try_into().ok()?)).ok()?;
    (node.len() == 5 + length).then_some(&node[5..])
}

fn wrap_leaf(value_type: &[u8], payload: &[u8], output: &mut Vec<u8>) -> Result<(), Failure> {
    let length =
        u32::try_from(payload.len()).map_err(|_| failure(FailureCode::StorageExhausted, 5))?;
    let total = value_type
        .len()
        .checked_add(5)
        .and_then(|value| value.checked_add(payload.len()))
        .ok_or_else(|| failure(FailureCode::StorageExhausted, 6))?;
    if total > super::MAXIMUM_BROWSER_VALUE_BYTES {
        return Err(failure(FailureCode::StorageExhausted, 7));
    }
    output.clear();
    output.extend_from_slice(value_type);
    output.push(0);
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(payload);
    Ok(())
}

fn failure(code: FailureCode, detail: u16) -> Failure {
    Failure { code, detail }
}
