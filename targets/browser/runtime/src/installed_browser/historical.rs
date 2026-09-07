//! Browser realization of the reusable bounded typed-history operation.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    FaceStartupParameter, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, PlannedGear, StructuredInfoType,
};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};

pub(crate) const HOST_OPERATION: &str = "conduit.host/browser-bounded-typed-history@1";
const IMPLEMENTATION: &str = "browser/bounded-typed-history@1";
const MAXIMUM_COMMANDS: u32 = 16;
const MAXIMUM_BROWSER_HISTORY_ENTRIES: usize = 4;

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

pub(crate) struct PreparedHistory {
    operation: conduit_time::BoundedHistoricalOperation,
    command_type: Vec<u8>,
    timeline_type: Vec<u8>,
    timeline: Vec<u8>,
    output: Vec<u8>,
}

impl PreparedHistory {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        if placement.implementation_id.as_str() != IMPLEMENTATION {
            return Ok(None);
        }
        validate_placement(placement, &offer())?;
        let maximum_entries = placement
            .configuration
            .iter()
            .find(|entry| entry.key == "maximum-entries")
            .and_then(|entry| match entry.value {
                conduit_core::ConfigurationValue::U64(value) => usize::try_from(value).ok(),
                _ => None,
            })
            .ok_or("browser bounded history has no exact entry bound")?;
        if maximum_entries > MAXIMUM_BROWSER_HISTORY_ENTRIES {
            return Err("browser bounded history exceeds the four-entry profile".into());
        }
        let timeline =
            conduit_time::historical_timeline_from_configuration(&placement.configuration)
                .map_err(|error| format!("prepare browser bounded history: {error:?}"))?;
        Ok(Some(Self {
            operation: conduit_time::BoundedHistoricalOperation::new(timeline),
            command_type: StructuredInfoType::leaf(conduit_core::kind_id(
                conduit_time::HISTORICAL_TIMELINE_COMMAND_INFO_ID,
            ))
            .map_err(|error| format!("history command type: {error:?}"))?
            .canonical_bytes()
            .map_err(|error| format!("history command type bytes: {error:?}"))?,
            timeline_type: StructuredInfoType::leaf(conduit_core::kind_id(
                "history/typed-timeline@1",
            ))
            .map_err(|error| format!("history timeline type: {error:?}"))?
            .canonical_bytes()
            .map_err(|error| format!("history timeline type bytes: {error:?}"))?,
            timeline: vec![0; conduit_time::MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES],
            output: Vec::with_capacity(super::MAXIMUM_BROWSER_VALUE_BYTES),
        }))
    }

    pub(crate) fn execute<'a>(&'a mut self, canonical: &[u8]) -> Result<&'a [u8], Failure> {
        let command = exact_leaf(canonical, &self.command_type)
            .ok_or_else(|| failure(FailureCode::InvalidInput, 1))?;
        let result = self
            .operation
            .apply_command(command, &mut self.timeline)
            .map_err(|_| failure(FailureCode::InvalidInput, 2))?;
        let output_length = self
            .timeline_type
            .len()
            .checked_add(5)
            .and_then(|length| length.checked_add(result.timeline_bytes))
            .ok_or_else(|| failure(FailureCode::StorageExhausted, 3))?;
        if output_length > super::MAXIMUM_BROWSER_VALUE_BYTES {
            return Err(failure(FailureCode::StorageExhausted, 5));
        }
        let timeline_length = u32::try_from(result.timeline_bytes)
            .map_err(|_| failure(FailureCode::StorageExhausted, 4))?;
        self.output.clear();
        self.output.extend_from_slice(&self.timeline_type);
        self.output.push(0);
        self.output
            .extend_from_slice(&timeline_length.to_le_bytes());
        self.output
            .extend_from_slice(&self.timeline[..result.timeline_bytes]);
        Ok(&self.output)
    }
}

fn offer() -> CapabilityOffer {
    let definition = conduit_time::historical_timeline_kind_definition();
    CapabilityOffer {
        startup_parameters: [
            ("value-profile", "Text"),
            ("clock-basis", "Text"),
            ("time-scale", "Text"),
            ("maximum-entries", "Count"),
            ("maximum-referenced-bytes", "Count"),
            ("overflow-policy", "Text"),
            ("first-sequence", "Count"),
        ]
        .map(|(name, value_type)| FaceStartupParameter {
            name: name.into(),
            value_type: value_type.into(),
            has_default: true,
        })
        .into(),
        shorthand: None,
        capability_id: CapabilityId::from(IMPLEMENTATION),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(
            conduit_time::HISTORICAL_TIMELINE_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-time/bounded-typed-history@1"),
        },
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(HOST_OPERATION),
            target_kind: Some(definition.kind_id),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_time::MAXIMUM_HISTORICAL_TIMELINE_COMMAND_BYTES as u32,
            maximum_output_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        },
    }
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    PreparedHistory::for_placement(placement)?
        .ok_or_else(|| "bounded history placement selected another implementation".to_string())?;
    Ok(BrowserOperation::unary(
        conduit_time::MAXIMUM_HISTORICAL_TIMELINE_COMMAND_BYTES as u32,
        MAXIMUM_COMMANDS,
    ))
}

fn exact_leaf<'a>(canonical: &'a [u8], value_type: &[u8]) -> Option<&'a [u8]> {
    let node = canonical.strip_prefix(value_type)?;
    if node.first() != Some(&0) || node.len() < 5 {
        return None;
    }
    let length = usize::try_from(u32::from_le_bytes(node[1..5].try_into().ok()?)).ok()?;
    (node.len() == 5 + length).then_some(&node[5..])
}

fn failure(code: FailureCode, detail: u16) -> Failure {
    Failure { code, detail }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{ConfigurationEntry, ConfigurationValue, OfferGeneration};

    fn placement(maximum_entries: u64) -> PlannedGear {
        let offer = offer();
        PlannedGear {
            placement_id: "history-placement".into(),
            gear_id: "history".into(),
            kind_id: offer.kind_id,
            kind_contract_revision: offer.kind_contract_revision,
            execution_profile_id: offer.implementation.execution_profile_id,
            configuration: vec![
                entry(
                    "value-profile",
                    ConfigurationValue::Text("value/text@1".into()),
                ),
                entry(
                    "clock-basis",
                    ConfigurationValue::Text("memory/event-clock".into()),
                ),
                entry(
                    "time-scale",
                    ConfigurationValue::Text("milliseconds".into()),
                ),
                entry("maximum-entries", ConfigurationValue::U64(maximum_entries)),
                entry("maximum-referenced-bytes", ConfigurationValue::U64(4096)),
                entry("overflow-policy", ConfigurationValue::Text("refuse".into())),
                entry("first-sequence", ConfigurationValue::U64(0)),
            ],
            host_id: "browser/history".into(),
            boot_id: "browser-boot/history".into(),
            offer_generation: OfferGeneration(1),
            capability_id: offer.capability_id,
            implementation_id: offer.implementation.implementation_id,
            artifact_id: offer.implementation.artifact_id,
            realization_characteristics: Vec::new(),
            limits: offer.limits,
            inputs: offer.inputs,
            outputs: offer.outputs,
            host_operations: offer.host_operations,
            resources: Vec::new(),
            authority: Vec::new(),
            pool_references: Vec::new(),
        }
    }

    fn entry(key: &str, value: ConfigurationValue) -> ConfigurationEntry {
        ConfigurationEntry {
            key: key.into(),
            value,
        }
    }

    fn clear_command() -> Vec<u8> {
        let mut wire = [0; conduit_time::MAXIMUM_HISTORICAL_TIMELINE_COMMAND_BYTES];
        let length = conduit_time::encode_historical_timeline_command_into(
            &conduit_time::HistoricalTimelineCommand::Clear,
            &mut wire,
        )
        .unwrap();
        conduit_core::StructuredInfoValue::leaf(
            StructuredInfoType::leaf(conduit_core::kind_id(
                conduit_time::HISTORICAL_TIMELINE_COMMAND_INFO_ID,
            ))
            .unwrap(),
            wire[..length].to_vec(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    }

    #[test]
    fn exact_commands_mutate_one_prepared_history_and_emit_canonical_snapshots() {
        let placement = placement(4);
        let mut prepared = PreparedHistory::for_placement(&placement).unwrap().unwrap();
        let timeline_type =
            StructuredInfoType::leaf(conduit_core::kind_id("history/typed-timeline@1"))
                .unwrap()
                .canonical_bytes()
                .unwrap();
        let first_revision = {
            let first = prepared.execute(&clear_command()).unwrap();
            conduit_time::decode_historical_timeline(exact_leaf(first, &timeline_type).unwrap())
                .unwrap()
                .clear_revision()
        };
        let second_revision = {
            let second = prepared.execute(&clear_command()).unwrap();
            conduit_time::decode_historical_timeline(exact_leaf(second, &timeline_type).unwrap())
                .unwrap()
                .clear_revision()
        };
        assert_eq!(first_revision, 1);
        assert_eq!(second_revision, 2);
    }

    #[test]
    fn malformed_commands_and_overlarge_browser_profiles_refuse_before_output() {
        let mut prepared = PreparedHistory::for_placement(&placement(4))
            .unwrap()
            .unwrap();
        assert_eq!(
            prepared
                .execute(b"not a canonical command")
                .unwrap_err()
                .code,
            FailureCode::InvalidInput
        );
        assert!(PreparedHistory::for_placement(&placement(5)).is_err());
    }
}
