use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{
    port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationEntry,
    ConfigurationValue, ExecutionProfileId, ImplementationId, ImplementationOffer, KindId,
    KindIdentity, PlannedGear, PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
    StructuredInfoValue,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, HostedValueStore, PortId, RequestId,
    ValueRef, ValueStorage,
};

pub(crate) const SOURCE_KIND: &str = "conduit-test/structured-source";
pub(crate) const SINK_KIND: &str = "conduit-test/structured-sink";
const SOURCE_IMPLEMENTATION: &str = "conduit-test/structured-source@1";
const SINK_IMPLEMENTATION: &str = "conduit-test/structured-sink@1";

pub(super) static SOURCE_FACTORY: BackFactory = BackFactory {
    implementation_id: SOURCE_IMPLEMENTATION,
    budget,
    prepare: prepare_source,
};
pub(super) static SINK_FACTORY: BackFactory = BackFactory {
    implementation_id: SINK_IMPLEMENTATION,
    budget,
    prepare: prepare_sink,
};

pub(super) struct SourceBack {
    pub(super) values: Vec<ValueRef>,
    pub(super) waits: Vec<ValueRef>,
    next: usize,
    pending: Option<RequestId>,
}

impl<const PORTS: usize> StepBack<PORTS> for SourceBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return structured_fixture_fail(154);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(value) = self.values.get(self.next).copied() else {
                return structured_fixture_fail(154);
            };
            io.consume_host_completion()
                .expect("observed structured fixture wait");
            io.send(PortId(0), value)
                .expect("ready structured fixture output");
            self.pending = None;
            self.next += 1;
            return StepOutcome::Progress;
        }
        let Some(value) = self.values.get(self.next).copied() else {
            return StepOutcome::Complete;
        };
        if self.pending.is_none() && !self.waits.is_empty() {
            let Some(wait) = self.waits.get(self.next).copied() else {
                return structured_fixture_fail(155);
            };
            let request = RequestId(u32::try_from(self.next).unwrap_or(u32::MAX));
            io.request_host_call(
                request,
                HostCallId(0),
                BoundedValueRef::new(wait, 8).expect("fixture wait is exactly eight bytes"),
            )
            .expect("structured fixture wait Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if self.pending.is_some() {
            return StepOutcome::Await;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.send(PortId(0), value)
            .expect("ready structured fixture output");
        self.next += 1;
        StepOutcome::Progress
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl SourceBack {}

pub(super) struct SinkBack {
    expected: Vec<Vec<Vec<u8>>>,
    received: usize,
}

impl<const PORTS: usize> StepBack<PORTS> for SinkBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if io.input(PortId(0)).is_some() {
            let valid = input_bytes.input(PortId(0)).is_some_and(|canonical| {
                self.expected.get(self.received).is_some_and(|choices| {
                    choices
                        .iter()
                        .any(|expected| expected.as_slice() == canonical)
                })
            });
            if !valid {
                return structured_fixture_fail(150);
            }
            io.consume(PortId(0))
                .expect("present structured fixture input");
            self.received += 1;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.received == self.expected.len() {
            io.consume_closed(PortId(0))
                .expect("observed structured fixture closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

const fn structured_fixture_fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

impl SinkBack {}

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
        startup_parameters: vec![conduit_core::FrontStartupParameter {
            name: "value".into(),
            value_type: conduit_core::kind_id("value/text"),
            has_default: true,
        }],
        shorthand: None,
        capability_id: CapabilityId::from(if source { source_kind } else { sink_kind }),
        kind_id: KindId::from(if source { source_kind } else { sink_kind }),
        kind_contract_revision: KindIdentity::from(if source {
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
        host_calls: Vec::new(),
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

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let configured = configured_values(placement)?;
    let count = u16::try_from(configured.len()).map_err(|_| "too many structured fixtures")?;
    let maximum = configured.iter().map(Vec::len).max().unwrap_or_default() as u32;
    Ok(BackBudget {
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
) -> Result<InstalledBack, String> {
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
    Ok(InstalledBack::TestStructuredSource(SourceBack {
        values: stored,
        waits,
        next: 0,
        pending: None,
    }))
}

fn prepare_sink(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<InstalledBack, String> {
    let expected = if let [ConfigurationEntry {
        key,
        value: ConfigurationValue::Text(encoded),
    }] = placement.configuration.as_slice()
    {
        if key == "choices" {
            encoded
                .split(',')
                .map(|row| row.split('|').map(unhex).collect::<Result<Vec<_>, _>>())
                .collect::<Result<Vec<_>, _>>()?
        } else {
            configured_values(placement)?
                .into_iter()
                .map(|value| vec![value])
                .collect()
        }
    } else {
        return Err("structured sink fixture configuration is malformed".into());
    };
    for value in expected.iter().flatten() {
        StructuredInfoValue::from_canonical_bytes(value)
            .map_err(|error| format!("structured fixture refusal: {error:?}"))?;
    }
    Ok(InstalledBack::TestStructuredSink(SinkBack {
        expected,
        received: 0,
    }))
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
        "choices" => encoded.split([',', '|']).map(unhex).collect(),
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
