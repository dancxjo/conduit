use conduit_composite::{
    KernelCompositeSignStorage, KernelCompositeStatus, KernelCompositeTerminal,
};
use conduit_core::{ValuePayload, validate_canonical_structured_value};
use conduit_plot::PreparedPortableExpressionEvaluator;
use conduitos::usb_base::{
    control_contract::{CONTROL_CALL, ControlContract},
    control_proof_plan::ControlProofSubject,
    control_request::ControlTransferRequest,
    control_result::{ControlTransferDisposition, PreparedControlResultEncoder},
    device_probe_proof_kernel::PreparedDeviceProbeKernel,
};

fn program(entry: &str) -> conduit_plot::PortableExpressionProgram {
    let source = [
        include_str!("../../plots/usb/control-types.conduit"),
        include_str!("../../plots/usb/hid-boot-control.conduit"),
    ]
    .join("\n");
    let contract = ControlContract::prepare().unwrap();
    let (startup, profile) = contract.catalogs();
    super::program_with_catalog(&source, entry, &startup, &profile)
}

fn tag(bytes: &[u8], expected: &str) {
    assert!(
        validate_canonical_structured_value(bytes)
            .unwrap()
            .variant_payload(expected)
            .unwrap()
            .is_some(),
        "expected {expected}"
    );
}

#[test]
fn every_interface_has_the_exact_no_data_boot_selection_request() {
    let p = program("usb-hid-boot-request");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let allocations = super::allocation::allocations(|| {
        for interface in 0..=255_u8 {
            let value =
                validate_canonical_structured_value(evaluator.evaluate(&[interface]).unwrap())
                    .unwrap();
            assert_eq!(
                value
                    .record_field("setup")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                [0x21, 11, 0, 0, interface, 0, 0, 0]
            );
            assert_eq!(
                value
                    .record_field("output")
                    .unwrap()
                    .unwrap()
                    .collection_length()
                    .unwrap(),
                0
            );
        }
    });
    assert_eq!(allocations, 0);
}

#[test]
fn boot_readiness_requires_an_exact_empty_completion_and_retains_failure() {
    let p = program("usb-hid-boot-result");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let contract = ControlContract::prepare().unwrap();
    let mut encoder = PreparedControlResultEncoder::new(&contract).unwrap();
    let request = ControlTransferRequest::new([0x21, 11, 0, 0, 7, 0, 0, 0], &[], 256).unwrap();
    let completed = encoder.completed(&request, 0, &[]).unwrap().to_vec();
    tag(evaluator.evaluate(&completed).unwrap(), "ready");
    for (field, changed) in [("transferred", vec![1, 0]), ("short", vec![1])] {
        let invalid = super::control_reply::replace_completed_leaf(&completed, field, &changed);
        tag(evaluator.evaluate(&invalid).unwrap(), "malformed");
    }
    let input_request = ControlTransferRequest::new([0x80, 6, 0, 1, 0, 0, 1, 0], &[], 256).unwrap();
    let unexpected_input = encoder.completed(&input_request, 1, &[42]).unwrap();
    tag(evaluator.evaluate(unexpected_input).unwrap(), "malformed");
    let inconsistent =
        super::control_reply::replace_completed_leaf(unexpected_input, "transferred", &[0, 0]);
    tag(evaluator.evaluate(&inconsistent).unwrap(), "malformed");
    for (disposition, expected) in [
        (ControlTransferDisposition::Stalled, "stalled"),
        (ControlTransferDisposition::ProviderLost, "provider-lost"),
        (ControlTransferDisposition::Unsupported, "unsupported"),
    ] {
        tag(
            evaluator
                .evaluate(encoder.disposition(disposition).unwrap())
                .unwrap(),
            expected,
        );
    }
}

#[test]
fn production_kernel_selects_boot_without_retries_or_play_allocations() {
    let subject = ControlProofSubject {
        host_id: "host/hid-boot",
        boot_id: "boot/hid-boot",
        controller_base_id: "base/hid-boot",
        device_instance_id: "device/hid-boot",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
    };
    let mut run = PreparedDeviceProbeKernel::prepare_hid_boot(
        &subject,
        KernelCompositeSignStorage {
            additional_local_items: 4096,
            additional_remote_items: 512,
        },
    )
    .unwrap();
    let input = run.kernel.definition().boundary.input_fronts[0]
        .external_port
        .clone();
    let output = run.kernel.definition().boundary.output_fronts[0]
        .external_port
        .clone();
    let contract = ControlContract::prepare().unwrap();
    let mut encoder = PreparedControlResultEncoder::new(&contract).unwrap();
    let request = ControlTransferRequest::new([0x21, 11, 0, 0, 7, 0, 0, 0], &[], 256).unwrap();
    let replies = [
        (
            encoder.completed(&request, 0, &[]).unwrap().to_vec(),
            "ready",
        ),
        (
            encoder
                .disposition(ControlTransferDisposition::Stalled)
                .unwrap()
                .to_vec(),
            "stalled",
        ),
        (
            encoder
                .disposition(ControlTransferDisposition::ProviderLost)
                .unwrap()
                .to_vec(),
            "provider-lost",
        ),
        (
            encoder
                .disposition(ControlTransferDisposition::Unsupported)
                .unwrap()
                .to_vec(),
            "unsupported",
        ),
    ];
    let pulse = ValuePayload {
        value_kind: input.value_kind.clone(),
        encoded: vec![7],
    };
    let mut value = ValuePayload {
        value_kind: output.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    let mut transfers = 0;
    let mut complete = false;
    let allocations = super::allocation::allocations(|| {
        run.kernel.start().unwrap();
        for (at, (reply, expected)) in replies.iter().enumerate() {
            run.kernel
                .admit_input(&input.port_id, at as u64, &pulse)
                .unwrap();
            if at + 1 == replies.len() {
                run.kernel.close_input(&input.port_id).unwrap();
            }
            let mut seen = false;
            for _ in 0..128 {
                let status = run.kernel.step().unwrap();
                if let Some(request) = run.kernel.next_host_request()
                    && !run.dispatch_pure(&request).unwrap()
                {
                    let obligation = run.kernel.host_request_obligation(&request).unwrap();
                    assert_eq!(obligation.requirement.contract_id.as_str(), CONTROL_CALL);
                    let admitted = run
                        .kernel
                        .admit_host_request(
                            &request,
                            &obligation.host,
                            &obligation.resources,
                            &obligation.authorities,
                        )
                        .unwrap();
                    let wire = validate_canonical_structured_value(
                        run.kernel.host_request_input(&admitted).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(
                        wire.record_field("setup")
                            .unwrap()
                            .unwrap()
                            .primitive_bytes("value/u64")
                            .unwrap(),
                        [0x21, 11, 0, 0, 7, 0, 0, 0]
                    );
                    transfers += 1;
                    assert_eq!(transfers, at + 1, "hidden retry");
                    run.kernel
                        .complete_host_call_bytes(&admitted, reply)
                        .unwrap();
                }
                if let Some(sequence) = run.kernel.output_into(&output.port_id, &mut value).unwrap()
                {
                    assert!(!seen);
                    assert_eq!(sequence, at as u64);
                    tag(&value.encoded, expected);
                    run.kernel
                        .complete_output(&output.port_id, sequence)
                        .unwrap();
                    seen = true;
                }
                if status == KernelCompositeStatus::Complete {
                    complete = true;
                    break;
                }
                if seen && at + 1 < replies.len() {
                    break;
                }
            }
            assert!(seen);
        }
        assert!(complete);
        assert_eq!(transfers, replies.len());
        assert_eq!(
            run.kernel
                .output_terminal_into(&output.port_id, &mut value)
                .unwrap(),
            Some(KernelCompositeTerminal::Normal)
        );
    });
    assert_eq!(allocations, 0);
}
