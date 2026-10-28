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

pub(super) struct DataTextGenerationHosts<'a> {
    #[cfg(target_os = "linux")]
    file: Option<(
        NodeId,
        DataTextOperation,
        conduit_core::AuthorityBinding,
        crate::text_file::PreparedTextFile<'a>,
    )>,
    _lifetime: core::marker::PhantomData<&'a ()>,
    text_kind: KindId,
    stores: Vec<PoolStore>,
    bindings: Vec<NodeBinding>,
}

impl<'a> DataTextGenerationHosts<'a> {
    pub(super) fn contains_node(&self, node: NodeId) -> bool {
        #[cfg(target_os = "linux")]
        if self.file.as_ref().is_some_and(|entry| entry.0 == node) {
            return true;
        }
        self.bindings.iter().any(|binding| binding.node == node)
    }

    pub(super) fn prepare_for_body(
        fragment: &PlanFragment,
        identity: &KernelIdentityMap,
        #[cfg(target_os = "linux")] selected_file: Option<
            &'a crate::text_file::SelectedTextFileResidence,
        >,
    ) -> Result<Self, String> {
        if fragment.placements.iter().any(|placement| {
            [
                conduit_std_offers::DATA_SAVE_TEXT_STD_IMPLEMENTATION,
                conduit_std_offers::DATA_LOAD_TEXT_STD_IMPLEMENTATION,
            ]
            .contains(&placement.implementation_id.as_str())
        }) {
            return Err("Body Text data requires an explicit durable file selection".into());
        }
        #[cfg(not(target_os = "linux"))]
        if fragment
            .placements
            .iter()
            .any(|p| is_file_implementation(p.implementation_id.as_str()))
        {
            return Err("selected file Text is unavailable on this Host".into());
        }
        Ok(Self {
            #[cfg(target_os = "linux")]
            file: prepare_file_binding(fragment, identity, selected_file)?,
            _lifetime: core::marker::PhantomData,
            text_kind: kind_id("value/text"),
            stores: Vec::new(),
            bindings: Vec::new(),
        })
    }

    pub(super) fn prepare(
        fragment: &PlanFragment,
        identity: &KernelIdentityMap,
        play: &ActivePlayIdentity,
        #[cfg(target_os = "linux")] selected_file: Option<
            &'a crate::text_file::SelectedTextFileResidence,
        >,
    ) -> Result<Self, String> {
        #[cfg(target_os = "linux")]
        let file = prepare_file_binding(fragment, identity, selected_file)?;
        #[cfg(not(target_os = "linux"))]
        if fragment
            .placements
            .iter()
            .any(|p| is_file_implementation(p.implementation_id.as_str()))
        {
            return Err("selected file Text is unavailable on this Host".into());
        }
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
            #[cfg(target_os = "linux")]
            file,
            _lifetime: core::marker::PhantomData,
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
        #[cfg(target_os = "linux")]
        if let Some((expected_node, expected_operation, authority, file)) = self.file.as_mut() {
            if node == *expected_node {
                if operation != *expected_operation {
                    return DataTextCompletion::Failed(10);
                }
                let completion = match operation {
                    DataTextOperation::Save => file.publish(authority, input),
                    DataTextOperation::Load => file.load(authority, input),
                };
                return match completion {
                    Ok(encoded) => DataTextCompletion::Output(encoded),
                    Err(refusal) => DataTextCompletion::Failed(refusal.detail()),
                };
            }
        }
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

/// One exact completion path for memory and selected-file Text data Backs.
pub(super) fn complete(
    scheduler: &mut super::InstalledScheduler,
    lowered_operation: &conduit_plan_lowering::lowering::LoweredHostCall,
    request: conduit_kernel::scheduler::HostCallRequest,
    completion: DataTextCompletion<'_>,
) -> Result<(), String> {
    use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome};
    let (disposition, output, failure) = match completion {
        DataTextCompletion::Output(encoded) => {
            let value = scheduler
                .store_host_value(encoded)
                .map_err(|error| format!("store bounded data Text output: {error:?}"))?;
            (
                HostCallDisposition::Completed,
                Some(
                    BoundedValueRef::new(value, lowered_operation.binding.maximum_output_bytes)
                        .map_err(|error| format!("bound data Text output: {error:?}"))?,
                ),
                None,
            )
        }
        DataTextCompletion::SaveTerminal(terminal) => (
            HostCallDisposition::Denied,
            None,
            Some(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::HostCallDenied,
                detail: u16::from(terminal.encode()[0]),
            }),
        ),
        DataTextCompletion::LoadTerminal(terminal) => (
            HostCallDisposition::Denied,
            None,
            Some(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::HostCallDenied,
                detail: u16::from(terminal.encode()[0]),
            }),
        ),
        DataTextCompletion::Failed(detail) => (
            HostCallDisposition::Failed,
            None,
            Some(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::HostCallFailed,
                detail,
            }),
        ),
    };
    scheduler
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition,
                output,
                failure,
            },
        )
        .map_err(|error| format!("complete data Text operation: {error:?}"))?;
    Ok(())
}

pub(super) fn operation_for_contract(
    contract: &conduit_core::HostCallContractId,
) -> Option<DataTextOperation> {
    match contract.as_str() {
        conduit_std_offers::DATA_SAVE_TEXT_HOST_CALL
        | conduit_std_offers::DURABLE_TEXT_PUBLISH_CALL => Some(DataTextOperation::Save),
        conduit_std_offers::DATA_LOAD_TEXT_HOST_CALL
        | conduit_std_offers::DURABLE_TEXT_READ_CALL => Some(DataTextOperation::Load),
        _ => None,
    }
}

fn is_file_implementation(id: &str) -> bool {
    [
        conduit_std_offers::DURABLE_TEXT_SAVE_IMPLEMENTATION,
        conduit_std_offers::DURABLE_TEXT_LOAD_IMPLEMENTATION,
    ]
    .contains(&id)
}
#[cfg(target_os = "linux")]
fn prepare_file_binding<'a>(
    fragment: &PlanFragment,
    identity: &KernelIdentityMap,
    selected_file: Option<&'a crate::text_file::SelectedTextFileResidence>,
) -> Result<
    Option<(
        NodeId,
        DataTextOperation,
        conduit_core::AuthorityBinding,
        crate::text_file::PreparedTextFile<'a>,
    )>,
    String,
> {
    let mut file = None;
    for placement in &fragment.placements {
        if !is_file_implementation(placement.implementation_id.as_str()) {
            continue;
        }
        #[cfg(not(target_os = "linux"))]
        return Err("selected file Text is unavailable on this Host".into());
        #[cfg(target_os = "linux")]
        {
            if file.is_some() {
                return Err("selected file Text admits one operation per Play".into());
            }
            let selected = selected_file.ok_or("durable Text has no selected file residence")?;
            let operation = if placement.kind_id.as_str() == conduit_data::DATA_SAVE_TEXT_KIND {
                DataTextOperation::Save
            } else {
                DataTextOperation::Load
            };
            let prepared = crate::text_file::PreparedTextFile::prepare(
                &selected.directory,
                placement,
                &selected.reference,
            )
            .map_err(|error| format!("prepare file Text: {error:?}"))?;
            let node = identity
                .node_for_placement(&placement.placement_id)
                .ok_or("file Text placement was not lowered")?;
            file = Some((node, operation, placement.authority[0].clone(), prepared));
        }
    }
    Ok(file)
}
