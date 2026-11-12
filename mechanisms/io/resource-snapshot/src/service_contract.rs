//! One finite residence service, two independently typed operations.
//! Preparation must bind append and read through selected_operation_set with
//! distinct target Kinds and call IDs, ONE affine residence resource binding,
//! and the same actual initialized exclusive owner. This Kind does not reserve
//! a second residence and is not itself a media, authority or capacity grant.
use alloc::vec;
use conduit_core::*;

pub const SERVICE_KIND: &str = "resource/record-service";
pub const SERVICE_REVISION: &str = "conduit.resource/record-service@1";
pub const APPEND_INPUT: &str = "record";
pub const APPEND_OUTPUT: &str = "index";
pub const READ_INPUT: &str = "read-index";
pub const READ_OUTPUT: &str = "read-result";

pub fn definition() -> Kind {
    let mut append = crate::append_contract::definition();
    let mut read = crate::read_contract::definition();
    let read_input = read.inputs[0].port_id.clone();
    let read_output = read.outputs[0].port_id.clone();
    read.inputs[0].port_id = port_id(READ_INPUT);
    read.outputs[0].port_id = port_id(READ_OUTPUT);
    let mut contracts = append.value_contracts().to_vec();
    for contract in read.value_contracts() {
        let mut contract = contract.clone();
        contract.location = match contract.location {
            FrontValueLocation::Input(port) if port == read_input => {
                FrontValueLocation::Input(port_id(READ_INPUT))
            }
            FrontValueLocation::Output(port) if port == read_output => {
                FrontValueLocation::Output(port_id(READ_OUTPUT))
            }
            _ => unreachable!("read contract has only its exact ordinary pair"),
        };
        contracts.push(contract);
    }
    let mut transductions = alloc::vec::Vec::new();
    for law in append.semantic_laws.iter().chain(&read.semantic_laws) {
        if let KindSemanticLaw::TerminalTransduction(profile) = law {
            let mut profile = profile.clone();
            if profile.input_port_id == read_input && profile.output_port_id == read_output {
                profile.input_port_id = port_id(READ_INPUT);
                profile.output_port_id = port_id(READ_OUTPUT);
            }
            profile.normal_close =
                NormalCloseTransduction::FlushThenPropagateWhenAllClose(FiniteTerminalEmission {
                    maximum_items: 2,
                    maximum_bytes: crate::append_result::APPEND_RESULT_BYTES
                        .checked_add(
                            u32::try_from(crate::read_result::READ_RESULT_BYTES)
                                .expect("finite read flush"),
                        )
                        .expect("finite service flush"),
                });
            transductions.push(profile);
        }
    }
    // Distinct input ports need canonical order, not stable sorting's large
    // temporary buffer on constrained preparation stacks.
    transductions.sort_unstable_by(|a, b| a.input_port_id.as_str().cmp(b.input_port_id.as_str()));
    append.kind_id = kind_id(SERVICE_KIND);
    append.kind_contract_revision = SERVICE_REVISION.into();
    append.inputs.extend(read.inputs);
    append.outputs.extend(read.outputs);
    append.semantic_laws = vec![
        KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
        KindSemanticLaw::ValueContracts(contracts),
    ];
    append.semantic_laws.extend(
        transductions
            .into_iter()
            .map(KindSemanticLaw::TerminalTransduction),
    );
    append.limits.max_queue_bytes = append
        .limits
        .max_queue_bytes
        .checked_add(read.limits.max_queue_bytes)
        .expect("finite service queues");
    append
}
#[cfg(test)]
#[path = "service_contract/tests.rs"]
mod tests;
