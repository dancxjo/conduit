//! One shared, finite Text-generation residence per exact admitted resource pool.

use super::data_text_back::DataTextOperation;
use conduit_core::{kind_id, ActivePlayIdentity, KindId, PlanFragment, ResourcePoolId};
use conduit_data::{
    DataGenerationNamespace, DataGenerationRefusal, DataLoadTextTerminal, DataReferenceRefusal,
    DataSaveTextTerminal, PreparedDataGenerationStore,
};
use conduit_kernel::NodeId;
use conduit_plan_lowering::lowering::KernelIdentityMap;

struct PoolStore {
    pool: ResourcePoolId,
    store: PreparedDataGenerationStore,
    reference_output: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NodeBinding {
    node: NodeId,
    store: usize,
    operation: DataTextOperation,
}

pub(super) enum DataTextCompletion<'a> {
    Output(&'a [u8]),
    SaveTerminal(DataSaveTextTerminal),
    LoadTerminal(DataLoadTextTerminal),
    Failed(u16),
}

pub(super) struct DataTextGenerationHosts {
    text_kind: KindId,
    stores: Vec<PoolStore>,
    bindings: Vec<NodeBinding>,
}

impl DataTextGenerationHosts {
    pub(super) fn prepare(
        fragment: &PlanFragment,
        identity: &KernelIdentityMap,
        play: &ActivePlayIdentity,
    ) -> Result<Self, String> {
        let mut stores: Vec<PoolStore> = Vec::new();
        let mut bindings = Vec::new();
        let reference_bytes = conduit_data::maximum_data_reference_encoded_bytes("value/text")
            .ok_or("canonical Text reference bound is absent")?;
        for placement in &fragment.placements {
            let operation = match placement.implementation_id.as_str() {
                conduit_std_offers::DATA_SAVE_TEXT_STD_IMPLEMENTATION => DataTextOperation::Save,
                conduit_std_offers::DATA_LOAD_TEXT_STD_IMPLEMENTATION => DataTextOperation::Load,
                _ => continue,
            };
            if placement.resources.len() != 1
                || placement.resources[0].class_id.as_str()
                    != conduit_std_offers::DATA_TEXT_GENERATION_RESOURCE_CLASS
                || placement.resources[0].units != 1
                || placement.resources[0].protected.is_some()
                || placement.resources[0].compute.is_some()
                || placement.resources[0].content.is_some()
            {
                return Err(
                    "data Text placement lacks its exact unprotected resource binding".into(),
                );
            }
            let pool = &placement.resources[0].pool_id;
            let store = if let Some(index) = stores.iter().position(|entry| &entry.pool == pool) {
                index
            } else {
                let ordinal = stores.len();
                let namespace = DataGenerationNamespace::new(&format!(
                    "plan/{}/play/{}/data/text/{ordinal}",
                    play.plan_id.as_str(),
                    play.play_sequence,
                ))
                .map_err(|error| format!("prepare data Text namespace: {error:?}"))?;
                stores.push(PoolStore {
                    pool: pool.clone(),
                    store: PreparedDataGenerationStore::new(
                        namespace,
                        kind_id("value/text"),
                        conduit_std_offers::DATA_TEXT_MAXIMUM_GENERATIONS,
                        conduit_data::MAXIMUM_DATA_TEXT_BYTES as usize,
                        conduit_std_offers::DATA_TEXT_MAXIMUM_RETAINED_BYTES,
                    )
                    .map_err(|error| format!("prepare data Text generation store: {error:?}"))?,
                    reference_output: Vec::with_capacity(reference_bytes),
                });
                ordinal
            };
            let node = identity
                .node_for_placement(&placement.placement_id)
                .ok_or("data Text placement was not lowered")?;
            bindings.push(NodeBinding {
                node,
                store,
                operation,
            });
        }
        Ok(Self {
            text_kind: kind_id("value/text"),
            stores,
            bindings,
        })
    }

    pub(super) fn execute(
        &mut self,
        node: NodeId,
        operation: DataTextOperation,
        input: &[u8],
    ) -> DataTextCompletion<'_> {
        let Some(binding) = self
            .bindings
            .iter()
            .find(|binding| binding.node == node && binding.operation == operation)
            .copied()
        else {
            return DataTextCompletion::Failed(1);
        };
        let text_kind = &self.text_kind;
        let Some(pool) = self.stores.get_mut(binding.store) else {
            return DataTextCompletion::Failed(2);
        };
        match operation {
            DataTextOperation::Save => {
                match pool
                    .store
                    .publish_into(text_kind, input, &mut pool.reference_output)
                {
                    Ok(()) => DataTextCompletion::Output(pool.reference_output.as_slice()),
                    Err(refusal) => save_completion(refusal),
                }
            }
            DataTextOperation::Load => match pool.store.load_encoded(input) {
                Ok(value) => DataTextCompletion::Output(value),
                Err(refusal) => load_completion(refusal),
            },
        }
    }
}

fn save_completion(refusal: DataGenerationRefusal) -> DataTextCompletion<'static> {
    match refusal {
        DataGenerationRefusal::ValueTooLarge => {
            DataTextCompletion::SaveTerminal(DataSaveTextTerminal::ValueTooLarge)
        }
        DataGenerationRefusal::GenerationCapacityExhausted => {
            DataTextCompletion::SaveTerminal(DataSaveTextTerminal::GenerationCapacityExhausted)
        }
        DataGenerationRefusal::ByteCapacityExhausted => {
            DataTextCompletion::SaveTerminal(DataSaveTextTerminal::ByteCapacityExhausted)
        }
        DataGenerationRefusal::WrongContentKind => {
            DataTextCompletion::SaveTerminal(DataSaveTextTerminal::WrongContentKind)
        }
        DataGenerationRefusal::InvalidBounds
        | DataGenerationRefusal::ReferenceOutputCapacity
        | DataGenerationRefusal::Reference(_)
        | DataGenerationRefusal::GenerationNotRetained
        | DataGenerationRefusal::ExtentMismatch => DataTextCompletion::Failed(3),
    }
}

fn load_completion(refusal: DataGenerationRefusal) -> DataTextCompletion<'static> {
    match refusal {
        DataGenerationRefusal::Reference(reference) => {
            let terminal = match reference {
                DataReferenceRefusal::Malformed | DataReferenceRefusal::WrongReferenceKind => {
                    DataLoadTextTerminal::MalformedReference
                }
                DataReferenceRefusal::WrongContentKind => DataLoadTextTerminal::WrongContentKind,
                DataReferenceRefusal::WrongAccessClass => DataLoadTextTerminal::WrongAccessClass,
                DataReferenceRefusal::ExpiringGeneration => {
                    DataLoadTextTerminal::ExpiringGeneration
                }
                DataReferenceRefusal::ItemExtent => DataLoadTextTerminal::ItemExtent,
            };
            DataTextCompletion::LoadTerminal(terminal)
        }
        DataGenerationRefusal::GenerationNotRetained => {
            DataTextCompletion::LoadTerminal(DataLoadTextTerminal::GenerationNotRetained)
        }
        DataGenerationRefusal::ExtentMismatch => {
            DataTextCompletion::LoadTerminal(DataLoadTextTerminal::ExtentMismatch)
        }
        DataGenerationRefusal::InvalidBounds
        | DataGenerationRefusal::ReferenceOutputCapacity
        | DataGenerationRefusal::WrongContentKind
        | DataGenerationRefusal::ValueTooLarge
        | DataGenerationRefusal::GenerationCapacityExhausted
        | DataGenerationRefusal::ByteCapacityExhausted => DataTextCompletion::Failed(4),
    }
}
