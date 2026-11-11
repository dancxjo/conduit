//! Finite physical read operation, prepared through the actual residence owner.
//! Each request captures its own recovered immutable physical tail. It reads at
//! most the requested index, with at most two media-unit reads. Equality with the
//! captured tail returns End; a larger index refuses. There is no cursor, ACK,
//! erasure, batch snapshot or authorization in this descriptive Source contract.
//! A consumer may retain the first captured_end as its own scan ceiling; later
//! requests may report a larger tail. This contract does not enforce that policy.
use alloc::{vec, vec::Vec};
use conduit_core::*;

pub const READ_KIND: &str = "resource/read-record";
pub const READ_REVISION: &str = "conduit.resource/read-record@1";
pub const READ_INPUT_BYTES: u32 = COUNT_ENCODED_LEN as u32;
pub const MAXIMUM_MEDIA_READS_PER_REQUEST: u64 = 2;
pub fn definition() -> Kind {
    let input = PortDescriptor {
        port_id: port_id("index"),
        value_kind: kind_id(COUNT_INFO_ID),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    let output = PortDescriptor {
        port_id: port_id("result"),
        value_kind: kind_id(crate::read_result::READ_RESULT_PROFILE),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    let output_bytes =
        u32::try_from(crate::read_result::READ_RESULT_BYTES).expect("linked finite read result");
    Kind {
        kind_id: kind_id(READ_KIND),
        kind_contract_revision: READ_REVISION.into(),
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
                    contract: CheckedValueContract::new(
                        input.value_kind,
                        READ_INPUT_BYTES,
                        vec![ValueConstraint::UnsignedRange {
                            minimum: Some(0),
                            maximum: Some(u64::from(u32::MAX)),
                            minimum_endpoint: IntervalEndpoint::Inclusive,
                            maximum_endpoint: IntervalEndpoint::Inclusive,
                        }],
                    )
                    .expect("finite physical Count"),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(output.port_id.clone()),
                    contract: CheckedValueContract::new(output.value_kind, output_bytes, vec![])
                        .expect("finite linked canonical read result"),
                },
            ]),
            KindSemanticLaw::TerminalTransduction(TerminalTransductionProfile {
                input_port_id: input.port_id,
                output_port_id: output.port_id,
                normal_close: NormalCloseTransduction::PropagateAfterDrain,
                abnormal: AbnormalTerminalTransduction::NotAccepted,
                // Adapter reporting of a genuine kernel Cancelled completion
                // does not create an authored cancellation initiation protocol.
                cancellation: CancellationTransduction::NotCancellable,
            }),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: READ_INPUT_BYTES + output_bytes,
        },
    }
}
#[cfg(test)]
#[path = "read_contract/tests.rs"]
mod tests;
