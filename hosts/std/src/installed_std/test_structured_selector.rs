use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationEntry,
    ConfigurationValue, ExecutionProfileId, ImplementationId, ImplementationOffer,
    KindContractRevision, KindId, PlannedGear, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType, StructuredInfoValue,
};
use conduit_kernel::{
    HostedValueStore, OperationAction, OperationInput, PortId, ValueRef, ValueStorage,
};

pub(crate) const SOURCE_KIND: &str = "conduit-test/structured-source";
pub(crate) const SINK_KIND: &str = "conduit-test/structured-sink";
const SOURCE_IMPLEMENTATION: &str = "conduit.test/structured-source@1";
const SINK_IMPLEMENTATION: &str = "conduit.test/structured-sink@1";

pub(super) static SOURCE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SOURCE_IMPLEMENTATION,
    budget,
    prepare: prepare_source,
};
pub(super) static SINK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SINK_IMPLEMENTATION,
    budget,
    prepare: prepare_sink,
};

pub(super) struct SourceOperation {
    value: Option<ValueRef>,
}

impl SourceOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        self.value
            .take()
            .map(|value| OperationAction::Emit {
                port: PortId(0),
                value,
            })
            .unwrap_or(OperationAction::Complete)
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Complete
    }
}

pub(super) struct SinkOperation {
    expected: Vec<u8>,
    received: bool,
}

impl SinkOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume_value(&mut self, port: PortId, canonical: &[u8]) -> OperationAction {
        if port != PortId(0) || self.received || canonical != self.expected {
            return InstalledOperation::fail(150);
        }
        self.received = true;
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Closed { port: PortId(0) } if self.received => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(151),
        }
    }
}

pub(crate) fn offer(value_type: &StructuredInfoType, direction: PortDirection) -> CapabilityOffer {
    let profile = value_type.profile().unwrap();
    let source = direction == PortDirection::Output;
    let port = PortDescriptor {
        port_id: port_id(if source { "output" } else { "input" }),
        value_kind: profile.value_kind().clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    };
    CapabilityOffer {
        startup_parameters: vec![conduit_core::FaceStartupParameter {
            name: "value".into(),
            value_type: "Text".into(),
            has_default: true,
        }],
        shorthand: None,
        capability_id: CapabilityId::from(if source {
            "test-structured-source"
        } else {
            "test-structured-sink"
        }),
        kind_id: KindId::from(if source { SOURCE_KIND } else { SINK_KIND }),
        kind_contract_revision: KindContractRevision::from(if source {
            "conduit.test/structured-source@1"
        } else {
            "conduit.test/structured-sink@1"
        }),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("conduit.test/structured-kernel@1"),
            implementation_id: ImplementationId::from(if source {
                SOURCE_IMPLEMENTATION
            } else {
                SINK_IMPLEMENTATION
            }),
            artifact_id: ArtifactId::from("conduit-std-host/test-structured@1"),
        },
        inputs: if source {
            Vec::new()
        } else {
            vec![port.clone()]
        },
        outputs: if source { vec![port] } else { Vec::new() },
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
        },
    }
}

pub(crate) fn configuration(value: &StructuredInfoValue) -> Vec<ConfigurationEntry> {
    vec![ConfigurationEntry {
        key: "value".into(),
        value: ConfigurationValue::Text(hex(&value.canonical_bytes().unwrap())),
    }]
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let value = configured_value(placement)?;
    let maximum = value
        .canonical_bytes()
        .map_err(|error| format!("value: {error:?}"))?
        .len() as u32;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: maximum.saturating_mul(2),
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: maximum,
    })
}

fn prepare_source(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    let value = configured_value(placement)?
        .canonical_bytes()
        .map_err(|error| format!("value: {error:?}"))?;
    let value = values
        .store(&value)
        .map_err(|error| format!("store fixture: {error:?}"))?;
    Ok(InstalledOperation::TestStructuredSource(SourceOperation {
        value: Some(value),
    }))
}

fn prepare_sink(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    let expected = configured_value(placement)?
        .canonical_bytes()
        .map_err(|error| format!("value: {error:?}"))?;
    Ok(InstalledOperation::TestStructuredSink(SinkOperation {
        expected,
        received: false,
    }))
}

fn configured_value(placement: &PlannedGear) -> Result<StructuredInfoValue, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("structured fixture requires one value".into());
    };
    let ("value", ConfigurationValue::Text(encoded)) = (entry.key.as_str(), &entry.value) else {
        return Err("structured fixture value is malformed".into());
    };
    StructuredInfoValue::from_canonical_bytes(&unhex(encoded)?)
        .map_err(|error| format!("structured fixture refusal: {error:?}"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(encoded: &str) -> Result<Vec<u8>, String> {
    if !encoded.len().is_multiple_of(2) {
        return Err("odd structured fixture hex".into());
    }
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = core::str::from_utf8(pair).map_err(|_| "invalid fixture hex")?;
            u8::from_str_radix(text, 16).map_err(|_| "invalid fixture hex")
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(String::from)
}
