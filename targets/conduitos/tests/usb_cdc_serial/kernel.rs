use super::super::*;
use super::execution::Execution;
use conduit_composite::{AdmittedKernelCompositeHostRequest, KernelCompositeStatus};
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduitos::usb_base::{
    control_contract::ControlContract, control_decode::PreparedControlRequestDecoder,
    control_result::PreparedControlResultEncoder, endpoint_read_contract::EndpointReadContract,
    endpoint_read_request::PreparedEndpointReadRequestDecoder,
    endpoint_read_result::PreparedEndpointReadResultEncoder,
    endpoint_write_contract::EndpointWriteContract,
    endpoint_write_request::PreparedEndpointWriteRequestDecoder,
    endpoint_write_result::PreparedEndpointWriteResultEncoder,
};
const CONFIG: &[u8] = &[
    9, 2, 67, 0, 2, 1, 0, 128, 50, 9, 4, 0, 0, 1, 2, 2, 1, 0, 5, 36, 0, 16, 1, 5, 36, 1, 0, 1, 4,
    36, 2, 2, 5, 36, 6, 0, 1, 7, 5, 131, 3, 16, 0, 16, 9, 4, 1, 0, 2, 10, 0, 0, 0, 7, 5, 129, 2,
    64, 0, 0, 7, 5, 2, 2, 64, 0, 0,
];
#[test]
fn whole_selected_back_kernel_closes_zero_one_or_thirty_two_with_pending_completion_and_pressure() {
    for count in [0usize, 1, 32] {
        execute(count);
    }
}
fn execute(count: usize) {
    let mut run = Execution::prepare(package());
    let kernel = &mut run.kernel;
    let input = kernel.definition().external_capability.inputs[0].clone();
    let output = kernel.definition().external_capability.outputs[0].clone();
    let contract = serial_contract::SerialDuplexContract::prepare().unwrap();
    let maximum = 64u64.to_le_bytes();
    let inputs: Vec<_> = (0..count)
        .map(|ordinal| ValuePayload {
            value_kind: input.value_kind.clone(),
            encoded: request(
                contract.request_type(),
                if ordinal % 2 == 0 {
                    "transmit"
                } else {
                    "receive"
                },
                if ordinal % 2 == 0 {
                    &[255, 0, 128]
                } else {
                    &maximum
                },
            )
            .canonical_bytes()
            .unwrap(),
        })
        .collect();
    let control = ControlContract::prepare().unwrap();
    let read = EndpointReadContract::prepare().unwrap();
    let write = EndpointWriteContract::prepare().unwrap();
    let cd = PreparedControlRequestDecoder::new(&control).unwrap();
    let rd = PreparedEndpointReadRequestDecoder::new(&read).unwrap();
    let wd = PreparedEndpointWriteRequestDecoder::new(&write).unwrap();
    let mut ce = PreparedControlResultEncoder::new(&control).unwrap();
    let mut re = PreparedEndpointReadResultEncoder::new(&read).unwrap();
    let mut we = PreparedEndpointWriteResultEncoder::new(&write).unwrap();
    let mut admitted_inputs = 0;
    let mut closed = false;
    let mut controls = 0;
    let mut writes = 0;
    let mut reads = 0;
    let mut seen = 0;
    let mut pending: Option<(AdmittedKernelCompositeHostRequest, Vec<u8>, usize)> = None;
    let mut held = false;
    let mut buffer = ValuePayload {
        value_kind: output.value_kind,
        encoded: Vec::with_capacity(4096),
    };
    run.kernel.start().unwrap();
    let mut complete = false;
    for step in 0..20000 {
        if admitted_inputs < count
            && matches!(
                run.kernel
                    .admit_input(
                        &input.port_id,
                        admitted_inputs as u64,
                        &inputs[admitted_inputs]
                    )
                    .unwrap(),
                RemoteIngressOutcome::Accepted { .. }
            )
        {
            admitted_inputs += 1;
        }
        if admitted_inputs == count && !closed {
            run.kernel.close_input(&input.port_id).unwrap();
            closed = true;
        }
        let status = run.kernel.step().unwrap_or_else(|error| {
            panic!(
                "count{count} step{step} native{controls}/{writes}/{reads} output{seen}: {error:?}"
            )
        });
        if let Some((token, bytes, delay)) = pending.take() {
            if delay == 0 {
                run.kernel.complete_host_call_bytes(&token, &bytes).unwrap();
            } else {
                pending = Some((token, bytes, delay - 1));
            }
        }
        if let Some(token) = run.kernel.next_host_request()
            && !run.service_pure(&token)
        {
            assert!(
                pending.is_none(),
                "no second physical effect while completion is retained"
            );
            let requirement = run.kernel.host_request_obligation(&token).unwrap();
            let call = run
                .kernel
                .admit_host_request(
                    &token,
                    &requirement.host,
                    &requirement.resources,
                    &requirement.authorities,
                )
                .unwrap();
            let bytes = run.kernel.host_request_input(&call).unwrap();
            let result = match requirement.requirement.contract_id.as_str() {
                conduitos::usb_base::control_contract::CONTROL_CALL => {
                    let decoded = cd.decode(bytes).unwrap();
                    let request = decoded.request(256).unwrap();
                    let data: &[u8] = match controls {
                        0 => CONFIG,
                        3 => &[0, 194, 1, 0, 0, 0, 8],
                        _ => &[],
                    };
                    controls += 1;
                    ce.completed(
                        &request,
                        if request.input() {
                            data.len() as u16
                        } else {
                            request.length()
                        },
                        data,
                    )
                    .unwrap()
                    .to_vec()
                }
                conduitos::usb_base::endpoint_write_contract::ENDPOINT_WRITE_CALL => {
                    let wire = wd.decode(bytes, 64).unwrap();
                    assert_eq!(wire, &[255, 0, 128]);
                    writes += 1;
                    we.completed(wire.len() as u16, wire.len() as u16)
                        .unwrap()
                        .to_vec()
                }
                conduitos::usb_base::endpoint_read_contract::ENDPOINT_READ_CALL => {
                    let length = rd.decode(bytes, 64).unwrap();
                    assert_eq!(length, 64);
                    let result = re
                        .completed(reads as u64, length, 3, &[255, 0, 128])
                        .unwrap()
                        .to_vec();
                    reads += 1;
                    result
                }
                other => panic!("unexpected effect {other}"),
            };
            if controls == 5 && !held {
                held = true;
                pending = Some((call, result, 8));
            } else {
                run.kernel.complete_host_call_bytes(&call, &result).unwrap();
            }
        }
        // Hold the semantic output queue periodically to exercise the same admitted
        // backpressure and closure paths as ordinary consumers.
        if step % 7 == 0
            && let Some(sequence) = run
                .kernel
                .output_into(&output.port_id, &mut buffer)
                .unwrap()
        {
            let value = StructuredInfoValue::from_canonical_bytes(&buffer.encoded).unwrap();
            let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
                panic!()
            };
            assert_eq!(
                tag,
                if seen % 2 == 0 {
                    "transmitted"
                } else {
                    "received"
                }
            );
            assert_eq!(
                scalar(member(payload, "ordinal")),
                (seen as u64).to_le_bytes()
            );
            assert_eq!(scalar(member(payload, "actual")), 3u64.to_le_bytes());
            seen += 1;
            run.kernel
                .complete_output(&output.port_id, sequence)
                .unwrap();
        }
        if status == KernelCompositeStatus::Complete {
            complete = true;
            break;
        }
    }
    assert!(
        complete,
        "count{count} native{controls}/{writes}/{reads} outputs{seen} signs {:?}",
        run.kernel.signs()
    );
    assert!(pending.is_none());
    assert_eq!(seen, count);
    assert_eq!(controls, if count == 0 { 0 } else { 5 });
    assert_eq!(writes, count.div_ceil(2));
    assert_eq!(reads, count / 2);
}
