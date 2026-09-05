use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationEntry,
    ConfigurationValue, ExecutionProfileId, ImplementationId, ImplementationOffer,
    KindContractRevision, KindId, PlannedGear, PortDescriptor, PortDirection, PortTemporal,
    StructuredInfoType, StructuredInfoValue,
};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, HostedValueStore, OperationAction,
    OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

pub(crate) const SOURCE_KIND: &str = "conduit-test/structured-source";
pub(crate) const SINK_KIND: &str = "conduit-test/structured-sink";
const SOURCE_IMPLEMENTATION: &str = "conduit-test/structured-source@1";
const SINK_IMPLEMENTATION: &str = "conduit-test/structured-sink@1";

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
    pub(super) values: Vec<ValueRef>,
    pub(super) waits: Vec<ValueRef>,
    next: usize,
    pending: Option<RequestId>,
}

impl SourceOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        if self.waits.is_empty() {
            self.emit_or_complete()
        } else {
            self.request_wait()
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        self.next += 1;
        if self.next >= self.values.len() {
            OperationAction::Complete
        } else {
            self.request_wait()
        }
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
            _ => InstalledOperation::fail(154),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }

    fn emit_or_complete(&self) -> OperationAction {
        self.values
            .get(self.next)
            .copied()
            .map_or(OperationAction::Complete, |value| OperationAction::Emit {
                port: PortId(0),
                value,
            })
    }

    fn request_wait(&mut self) -> OperationAction {
        let request = RequestId(u32::try_from(self.next).expect("bounded fixture request"));
        let Some(value) = self.waits.get(self.next).copied() else {
            return InstalledOperation::fail(155);
        };
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(value, 8).expect("fixture wait is exactly eight bytes"),
        }
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
    offer_named(value_type, direction, SOURCE_KIND, SINK_KIND)
}

pub(crate) fn offer_named(
    value_type: &StructuredInfoType,
    direction: PortDirection,
    source_kind: &str,
    sink_kind: &str,
) -> CapabilityOffer {
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
        capability_id: CapabilityId::from(if source { source_kind } else { sink_kind }),
        kind_id: KindId::from(if source { source_kind } else { sink_kind }),
        kind_contract_revision: KindContractRevision::from(if source {
            "conduit-test/structured-source@1"
        } else {
            "conduit-test/structured-sink@1"
        }),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("conduit-test/structured-kernel@1"),
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
    raw_configuration(&value.canonical_bytes().unwrap())
}

pub(crate) fn raw_source_offer(kind: &str, value_kind: &str) -> CapabilityOffer {
    let mut offer = offer_named(
        &StructuredInfoType::leaf(KindId::from("conduit-test/raw-placeholder@1")).unwrap(),
        PortDirection::Output,
        kind,
        SINK_KIND,
    );
    offer.outputs[0].value_kind = KindId::from(value_kind);
    offer
}

pub(crate) fn raw_configuration(value: &[u8]) -> Vec<ConfigurationEntry> {
    vec![ConfigurationEntry {
        key: "value".into(),
        value: ConfigurationValue::Text(hex(value)),
    }]
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let configured = configured_values(placement)?;
    let count = u16::try_from(configured.len()).map_err(|_| "too many structured fixtures")?;
    let maximum = configured.iter().map(Vec::len).max().unwrap_or_default() as u32;
    Ok(OperationBudget {
        value_items: count.saturating_add(1),
        value_bytes: configured
            .iter()
            .map(|value| value.len() as u32)
            .sum::<u32>()
            .saturating_add(maximum),
        host_requests: configured.len(),
        sign_items: 8,
        maximum_value_bytes: maximum,
    })
}

fn prepare_source(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<InstalledOperation, String> {
    let stored = configured_values(placement)?
        .iter()
        .map(|value| {
            values
                .store(value)
                .map_err(|error| format!("store fixture: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let waits = if stored.len() > 1 {
        (0..stored.len())
            .map(|_| {
                values
                    .store(&conduit_time::encode_tick(0))
                    .map_err(|error| format!("store fixture wait: {error:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    Ok(InstalledOperation::TestStructuredSource(SourceOperation {
        values: stored,
        waits,
        next: 0,
        pending: None,
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
    let values = configured_values(placement)?;
    let [value] = values.as_slice() else {
        return Err("structured sink requires one value".into());
    };
    StructuredInfoValue::from_canonical_bytes(value)
        .map_err(|error| format!("structured fixture refusal: {error:?}"))
}

fn configured_values(placement: &PlannedGear) -> Result<Vec<Vec<u8>>, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("structured fixture requires one value".into());
    };
    let ConfigurationValue::Text(encoded) = &entry.value else {
        return Err("structured fixture value is malformed".into());
    };
    match entry.key.as_str() {
        "value" => Ok(vec![unhex(encoded)?]),
        "values" => encoded.split(',').map(unhex).collect(),
        _ => Err("structured fixture value is malformed".into()),
    }
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
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let text = core::str::from_utf8(pair).map_err(|_| "invalid fixture hex")?;
            u8::from_str_radix(text, 16).map_err(|_| "invalid fixture hex")
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(String::from)
}
