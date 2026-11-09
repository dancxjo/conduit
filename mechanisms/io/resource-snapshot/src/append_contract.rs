//! One shared finite append Kind for Source checking and native preparation.
//! Construct during preparation; execution does not retain this owned graph.
use alloc::{vec, vec::Vec};
use conduit_core::*;
pub const APPEND_KIND: &str = "resource/try-append-record";
pub const APPEND_REVISION: &str = "conduit.resource/try-append-record@1";
pub fn definition() -> Kind {
    let input = PortDescriptor {
        port_id: port_id("record"),
        value_kind: kind_id(BYTES_INFO_ID),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    let output = PortDescriptor {
        port_id: port_id("index"),
        value_kind: kind_id(crate::append_result::APPEND_RESULT_PROFILE),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(APPEND_KIND),
        kind_contract_revision: APPEND_REVISION.into(),
        startup_parameters: Vec::new(),
        shorthand: None,
        configuration: Vec::new(),
        inputs: vec![input.clone()],
        outputs: vec![output.clone()],
        semantic_laws: vec![
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Input(input.port_id.clone()),
                    contract: CheckedValueContract::new(input.value_kind, 200, vec![])
                        .expect("finite Bytes"),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(output.port_id.clone()),
                    contract: CheckedValueContract::new(
                        output.value_kind,
                        crate::append_result::APPEND_RESULT_BYTES,
                        vec![],
                    )
                    .expect("finite canonical outcome"),
                },
            ]),
            KindSemanticLaw::TerminalTransduction(TerminalTransductionProfile {
                input_port_id: input.port_id,
                output_port_id: output.port_id,
                normal_close: NormalCloseTransduction::PropagateAfterDrain,
                abnormal: AbnormalTerminalTransduction::NotAccepted,
                cancellation: CancellationTransduction::NotCancellable,
            }),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 200 + crate::append_result::APPEND_RESULT_BYTES,
        },
    }
}
