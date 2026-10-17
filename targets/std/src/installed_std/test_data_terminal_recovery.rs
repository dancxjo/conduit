//! Test-only exact recovery sinks for exercising typed data terminal tracks.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{
    kind_id, port_id, AbnormalTerminalTransduction, ArtifactId, CancellationTransduction,
    CapabilityId, CapabilityLimits, CapabilityOffer, CheckedValueContract, ExecutionProfileId,
    FrontValueContract, FrontValueLocation, ImplementationId, Kind, KindIdentity, KindSemanticLaw,
    NormalCloseTransduction, PlannedGear, PortDescriptor, PortDirection, PortTemporal,
    TerminalTransductionProfile, EMPTY_INFO_ID,
};
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedNormalCloseTransduction, AssignedTerminalTransduction, StepBack, StepInputBytes,
        StepIo, StepOutcome,
    },
    Failure, FailureCode, PortId, ValueRef, ValueStorage,
};
use conduit_plot::ProfileCatalog;

const SAVE_KIND: &str = "conduit-test/data-save-terminal-recovery";
const LOAD_KIND: &str = "conduit-test/data-load-terminal-recovery";
const SAVE_REVISION: &str = "conduit-test/data-save-terminal-recovery@1";
const LOAD_REVISION: &str = "conduit-test/data-load-terminal-recovery@1";
const SAVE_IMPLEMENTATION: &str = "conduit-test/data-save-terminal-recovery-kernel@1";
const LOAD_IMPLEMENTATION: &str = "conduit-test/data-load-terminal-recovery-kernel@1";
const TEXT_SINK_KIND: &str = "conduit-test/data-text-sink";
const TEXT_SINK_REVISION: &str = "conduit-test/data-text-sink@1";
const TEXT_SINK_IMPLEMENTATION: &str = "conduit-test/data-text-sink-kernel@1";
const TEXT_SOURCE_KIND: &str = "conduit-test/data-text-source";
const TEXT_SOURCE_REVISION: &str = "conduit-test/data-text-source@1";
const TEXT_SOURCE_IMPLEMENTATION: &str = "conduit-test/data-text-source-kernel@1";

pub(super) static SAVE_FACTORY: BackFactory = BackFactory {
    implementation_id: SAVE_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static LOAD_FACTORY: BackFactory = BackFactory {
    implementation_id: LOAD_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static TEXT_SINK_FACTORY: BackFactory = BackFactory {
    implementation_id: TEXT_SINK_IMPLEMENTATION,
    budget: sink_budget,
    prepare: prepare_sink,
};
pub(super) static TEXT_SOURCE_FACTORY: BackFactory = BackFactory {
    implementation_id: TEXT_SOURCE_IMPLEMENTATION,
    budget: source_budget,
    prepare: prepare_source,
};

#[derive(Clone, Copy)]
enum TerminalKind {
    Save,
    Load,
}

pub(super) struct DataTerminalRecoveryBack {
    kind: TerminalKind,
    done: bool,
}

pub(super) struct DataTextSinkBack {
    done: bool,
}

pub(super) struct DataTextSourceBack {
    value: ValueRef,
    done: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for DataTextSourceBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.done {
            return StepOutcome::Complete;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.send(PortId(0), self.value)
            .expect("ready data Text source output");
        self.done = true;
        StepOutcome::Progress
    }
}

impl<const PORTS: usize> StepBack<PORTS> for DataTextSinkBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.done {
            return StepOutcome::Complete;
        }
        if io.input(PortId(0)).is_some() {
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(11);
            };
            if bytes != b"Hello" {
                return fail(12);
            }
            io.consume(PortId(0)).expect("present loaded Text");
            self.done = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            return fail(13);
        }
        StepOutcome::Await
    }
}

impl<const PORTS: usize> StepBack<PORTS> for DataTerminalRecoveryBack {
    fn terminal_transduction(&self) -> Option<AssignedTerminalTransduction> {
        Some(AssignedTerminalTransduction {
            input: PortId(0),
            output: PortId(0),
            normal_close: AssignedNormalCloseTransduction::NotAccepted,
            abnormal: AssignedAbnormalTransduction::Recover,
            cancellation: AssignedCancellationTransduction::NotCancellable,
        })
    }

    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.done {
            return StepOutcome::Complete;
        }
        if let Some(terminal) = io.input_abnormal(PortId(0)) {
            if !self.valid_terminal(terminal.as_slice()) {
                return fail(2);
            }
            io.consume_abnormal(PortId(0))
                .expect("present abnormal data terminal");
            self.done = true;
            return StepOutcome::Progress;
        }
        if io.input(PortId(0)).is_some() {
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(1);
            };
            if !self.valid_terminal(bytes) {
                return fail(2);
            }
            io.consume(PortId(0))
                .expect("present projected abnormal data terminal");
            self.done = true;
            return StepOutcome::Progress;
        }
        if io.input(PortId(1)).is_some() {
            io.consume(PortId(1))
                .expect("present successful data operation value");
            self.done = true;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
}

impl DataTerminalRecoveryBack {
    fn valid_terminal(&self, encoded: &[u8]) -> bool {
        match self.kind {
            TerminalKind::Save => conduit_data::DataSaveTextTerminal::decode(encoded).is_ok(),
            TerminalKind::Load => conduit_data::DataLoadTextTerminal::decode(encoded).is_ok(),
        }
    }
}

pub(super) fn offers() -> [CapabilityOffer; 2] {
    [offer(TerminalKind::Save), offer(TerminalKind::Load)]
}

pub(super) fn text_sink_offer() -> CapabilityOffer {
    text_fixture_offer(text_sink_contract(), TEXT_SINK_IMPLEMENTATION)
}

pub(super) fn text_source_offer() -> CapabilityOffer {
    text_fixture_offer(text_source_contract(), TEXT_SOURCE_IMPLEMENTATION)
}

fn text_fixture_offer(contract: Kind, implementation: &str) -> CapabilityOffer {
    let kind_name = contract.kind_id.as_str().to_string();
    conduit_core::BackOfferBuilder::new(
        contract,
        conduit_core::Back {
            capability_id: CapabilityId::from(kind_name),
            execution_profile_id: ExecutionProfileId::from("conduit-test/data-text-fixture@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-std-host/test-data-text-fixture@1"),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

pub(super) fn install_catalog(catalog: &mut ProfileCatalog) {
    for kind in [TerminalKind::Save, TerminalKind::Load] {
        catalog
            .insert_kind(contract(kind))
            .expect("data terminal recovery fixture is unique");
    }
    catalog
        .insert_kind(text_sink_contract())
        .expect("data Text sink fixture is unique");
    catalog
        .insert_kind(text_source_contract())
        .expect("data Text source fixture is unique");
}

fn text_sink_contract() -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(TEXT_SINK_KIND),
        kind_contract_revision: KindIdentity::from(TEXT_SINK_REVISION),
        inputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id("value/text"),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        outputs: Vec::new(),
        configuration: Vec::new(),
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: FrontValueLocation::Input(port_id("value")),
            contract: CheckedValueContract::new(
                kind_id("value/text"),
                conduit_data::MAXIMUM_DATA_TEXT_BYTES,
                vec![],
            )
            .expect("test data Text input is bounded"),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: conduit_data::MAXIMUM_DATA_TEXT_BYTES,
        },
    }
}

fn text_source_contract() -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(TEXT_SOURCE_KIND),
        kind_contract_revision: KindIdentity::from(TEXT_SOURCE_REVISION),
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id("value/text"),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        configuration: Vec::new(),
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: FrontValueLocation::Output(port_id("value")),
            contract: CheckedValueContract::new(
                kind_id("value/text"),
                conduit_data::MAXIMUM_DATA_TEXT_BYTES,
                vec![],
            )
            .expect("test data Text output is bounded"),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: conduit_data::MAXIMUM_DATA_TEXT_BYTES,
        },
    }
}

fn offer(kind: TerminalKind) -> CapabilityOffer {
    let contract = contract(kind);
    let implementation = match kind {
        TerminalKind::Save => SAVE_IMPLEMENTATION,
        TerminalKind::Load => LOAD_IMPLEMENTATION,
    };
    let kind_name = contract.kind_id.as_str().to_string();
    conduit_core::BackOfferBuilder::new(
        contract,
        conduit_core::Back {
            capability_id: CapabilityId::from(kind_name),
            execution_profile_id: ExecutionProfileId::from("conduit-test/data-terminal-recovery@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-std-host/test-data-terminal-recovery@1"),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

fn contract(kind: TerminalKind) -> Kind {
    let (kind_name, revision, terminal) = match kind {
        TerminalKind::Save => (
            SAVE_KIND,
            SAVE_REVISION,
            conduit_data::DATA_SAVE_TEXT_TERMINAL_INFO_ID,
        ),
        TerminalKind::Load => (
            LOAD_KIND,
            LOAD_REVISION,
            conduit_data::DATA_LOAD_TEXT_TERMINAL_INFO_ID,
        ),
    };
    let terminal_input = PortDescriptor {
        port_id: port_id("terminal"),
        value_kind: kind_id(terminal),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: Some(kind_id(terminal)),
    };
    let success_kind = match kind {
        TerminalKind::Save => conduit_core::data_reference_kind(&kind_id("value/text")),
        TerminalKind::Load => kind_id("value/text"),
    };
    let success_bytes = match kind {
        TerminalKind::Save => conduit_data::maximum_data_reference_encoded_bytes("value/text")
            .expect("Text reference is finite") as u32,
        TerminalKind::Load => conduit_data::MAXIMUM_DATA_TEXT_BYTES,
    };
    let success_input = PortDescriptor {
        port_id: port_id("success"),
        value_kind: success_kind.clone(),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let output = PortDescriptor {
        port_id: port_id("recovered"),
        value_kind: kind_id(EMPTY_INFO_ID),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(kind_name),
        kind_contract_revision: KindIdentity::from(revision),
        inputs: vec![terminal_input, success_input],
        outputs: vec![output],
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: FrontValueLocation::Input(port_id("success")),
                contract: CheckedValueContract::new(success_kind, success_bytes, vec![])
                    .expect("test recovery success input is bounded"),
            }]),
            KindSemanticLaw::TerminalTransduction(TerminalTransductionProfile {
                input_port_id: port_id("terminal"),
                output_port_id: port_id("recovered"),
                normal_close: NormalCloseTransduction::NotAccepted,
                abnormal: AbnormalTerminalTransduction::Recover,
                cancellation: CancellationTransduction::NotCancellable,
            }),
        ],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: conduit_data::MAXIMUM_DATA_TEXT_BYTES,
        },
    }
}

fn validate(placement: &PlannedGear) -> Result<TerminalKind, String> {
    for kind in [TerminalKind::Save, TerminalKind::Load] {
        let offer = offer(kind);
        if placement.implementation_id == offer.implementation.implementation_id {
            if placement.kind_id == offer.kind_id
                && placement.kind_contract_revision == offer.kind_contract_revision
                && placement.inputs == offer.inputs
                && placement.outputs == offer.outputs
                && placement.host_calls.is_empty()
                && placement.resources.is_empty()
                && placement.authority.is_empty()
            {
                return Ok(kind);
            }
            return Err("planned data terminal recovery differs from its test realization".into());
        }
    }
    Err("unsupported data terminal recovery fixture".into())
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 4,
        maximum_value_bytes: 1,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let kind = validate(placement)?;
    Ok(InstalledBack::TestDataTerminalRecovery(
        DataTerminalRecoveryBack { kind, done: false },
    ))
}

fn sink_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    if placement.kind_id.as_str() != TEXT_SINK_KIND
        || placement.kind_contract_revision.as_str() != TEXT_SINK_REVISION
        || placement.implementation_id.as_str() != TEXT_SINK_IMPLEMENTATION
        || placement.inputs != text_sink_contract().inputs
        || !placement.outputs.is_empty()
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err("planned data Text sink differs from its test realization".into());
    }
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 4,
        maximum_value_bytes: conduit_data::MAXIMUM_DATA_TEXT_BYTES,
    })
}

fn prepare_sink(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    sink_budget(placement)?;
    Ok(InstalledBack::TestDataTextSink(DataTextSinkBack {
        done: false,
    }))
}

fn source_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    if placement.kind_id.as_str() != TEXT_SOURCE_KIND
        || placement.kind_contract_revision.as_str() != TEXT_SOURCE_REVISION
        || placement.implementation_id.as_str() != TEXT_SOURCE_IMPLEMENTATION
        || !placement.inputs.is_empty()
        || placement.outputs != text_source_contract().outputs
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
    {
        return Err("planned data Text source differs from its test realization".into());
    }
    Ok(BackBudget {
        value_items: 1,
        value_bytes: 5,
        host_requests: 0,
        sign_items: 4,
        maximum_value_bytes: conduit_data::MAXIMUM_DATA_TEXT_BYTES,
    })
}

fn prepare_source(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    source_budget(placement)?;
    let value = values
        .store(b"Hello")
        .map_err(|error| format!("prepare data Text source: {error:?}"))?;
    Ok(InstalledBack::TestDataTextSource(DataTextSourceBack {
        value,
        done: false,
    }))
}

const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
