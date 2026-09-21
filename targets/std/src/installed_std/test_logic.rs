use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, ImplementationId, KindIdentity, PlannedGear, PortDescriptor, PortDirection,
    PortTemporal, Scalar, SCALAR_ENCODED_LEN, SCALAR_INFO_ID,
};
use conduit_form::{KindProjection, ProfileCatalog};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId, ValueRef, ValueStorage,
};

const KIND: &str = "conduit-test/logic-script";
const REVISION: &str = "conduit-test/logic-script@1";
const PROFILE: &str = "conduit-test/logic-script-kernel@1";
const IMPLEMENTATION: &str = "conduit-test/logic-script-kernel@1";
const ARTIFACT: &str = "conduit-std-host/test-logic-script@1";
const SINK_KIND: &str = "conduit-test/logic-sink";
const SINK_REVISION: &str = "conduit-test/logic-sink@1";
const SINK_PROFILE: &str = "conduit-test/logic-sink-kernel@1";
const SINK_IMPLEMENTATION: &str = "conduit-test/logic-sink-kernel@1";
const SINK_ARTIFACT: &str = "conduit-std-host/test-logic-sink@1";

pub(super) static TEST_LOGIC_SCRIPT_FACTORY: BackFactory = BackFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) static TEST_LOGIC_SINK_FACTORY: BackFactory = BackFactory {
    implementation_id: SINK_IMPLEMENTATION,
    budget: sink_budget,
    prepare: prepare_sink,
};

pub(super) struct TestLogicScriptBack {
    pub(super) values: [ValueRef; 4],
    pub(super) next: usize,
}

pub(super) struct TestLogicSinkBack;

impl<const PORTS: usize> StepBack<PORTS> for TestLogicScriptBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        let Some(value) = self.values.get(self.next).copied() else {
            return StepOutcome::Complete;
        };
        let port = PortId(u16::try_from(self.next).unwrap_or(u16::MAX));
        if !io.output_ready(port) {
            return StepOutcome::Await;
        }
        io.send(port, value).expect("ready logic fixture output");
        self.next += 1;
        StepOutcome::Progress
    }
}

impl<const PORTS: usize> StepBack<PORTS> for TestLogicSinkBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        let Some(value) = io.input(PortId(0)) else {
            return StepOutcome::Await;
        };
        let valid = value.byte_len == SCALAR_ENCODED_LEN as u32
            && input_bytes
                .input(PortId(0))
                .is_some_and(|bytes| Scalar::decode(bytes) == Ok(Scalar::from_raw_microunits(-1)));
        if !valid {
            return StepOutcome::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 24,
            });
        }
        io.consume(PortId(0)).expect("present logic fixture result");
        StepOutcome::Complete
    }
}

impl TestLogicScriptBack {}

impl TestLogicSinkBack {}

pub(super) fn offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("test-logic-script"),
        kind_id: kind_id(KIND),
        kind_contract_revision: KindIdentity::from(REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: ["compare-left", "compare-right", "when-false", "when-true"]
            .into_iter()
            .map(scalar_output)
            .collect(),
        host_calls: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: SCALAR_ENCODED_LEN as u32,
        },
    }
}

pub(super) fn sink_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("test-logic-sink"),
        kind_id: kind_id(SINK_KIND),
        kind_contract_revision: KindIdentity::from(SINK_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(SINK_PROFILE),
            implementation_id: ImplementationId::from(SINK_IMPLEMENTATION),
            artifact_id: ArtifactId::from(SINK_ARTIFACT),
        },
        inputs: vec![PortDescriptor {
            port_id: port_id("in"),
            value_kind: kind_id(SCALAR_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: Vec::new(),
        host_calls: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: SCALAR_ENCODED_LEN as u32,
        },
    }
}

pub(super) fn install_catalog(catalog: &mut ProfileCatalog) {
    catalog
        .insert(KindProjection {
            kind_id: kind_id(KIND),
            kind_contract_revision: KindIdentity::from(REVISION),
            inputs: Vec::new(),
            outputs: offer().outputs,
            configuration: Default::default(),
        })
        .expect("logic script fixture kind is unique");
    catalog
        .insert(KindProjection {
            kind_id: kind_id(SINK_KIND),
            kind_contract_revision: KindIdentity::from(SINK_REVISION),
            inputs: sink_offer().inputs,
            outputs: Vec::new(),
            configuration: Default::default(),
        })
        .expect("logic sink fixture kind is unique");
}

fn scalar_output(name: &str) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(SCALAR_INFO_ID),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
    }
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 4,
        value_bytes: (SCALAR_ENCODED_LEN * 4) as u32,
        host_requests: 0,
        sign_items: 64,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    store: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    let values = [
        Scalar::MIN,
        Scalar::MIN,
        Scalar::from_raw_microunits(-1),
        Scalar::MAX,
    ]
    .map(|scalar| {
        store
            .store(&scalar.encode())
            .map_err(|error| format!("store logic fixture scalar: {error:?}"))
    })
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?
    .try_into()
    .map_err(|_| "logic fixture value count changed".to_string())?;
    Ok(InstalledBack::TestLogicScript(TestLogicScriptBack {
        values,
        next: 0,
    }))
}

fn sink_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate_sink(placement)?;
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 32,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    })
}

fn prepare_sink(
    placement: &PlannedGear,
    _store: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate_sink(placement)?;
    Ok(InstalledBack::TestLogicSink(TestLogicSinkBack))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != KIND
        || placement.kind_contract_revision.as_str() != REVISION
        || placement.execution_profile_id.as_str() != PROFILE
        || placement.implementation_id.as_str() != IMPLEMENTATION
        || placement.artifact_id.as_str() != ARTIFACT
        || !placement.inputs.is_empty()
        || placement.outputs != offer().outputs
        || !placement.configuration.is_empty()
    {
        return Err("planned logic script does not match its fixture".into());
    }
    Ok(())
}

fn validate_sink(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != SINK_KIND
        || placement.kind_contract_revision.as_str() != SINK_REVISION
        || placement.execution_profile_id.as_str() != SINK_PROFILE
        || placement.implementation_id.as_str() != SINK_IMPLEMENTATION
        || placement.artifact_id.as_str() != SINK_ARTIFACT
        || placement.inputs != sink_offer().inputs
        || !placement.outputs.is_empty()
        || !placement.configuration.is_empty()
    {
        return Err("planned logic sink does not match its fixture".into());
    }
    Ok(())
}
