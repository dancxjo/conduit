//! The bounded scan runs through the ordinary state operation and kernel.
use super::*;
use conduitos::{
    protocol_source,
    usb_base::{
        control_source_plan, hid_source_kernel::PreparedUsbSourceKernel,
        installation::UsbNativeIdentity,
    },
};
#[test]
fn descriptor_scan_reuses_one_state_cell_and_emits_through_the_production_kernel() {
    let parser = Parser::new();
    let native = UsbNativeIdentity {
        host_id: "fixture/hid-parser-host".into(),
        boot_id: "fixture/hid-parser-boot".into(),
        base_id: "fixture/control".into(),
        provider_instance_id: "fixture/provider".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/pio".into(),
        resource_generation_id: ResourceGenerationId("fixture/resources".into()),
        envelope_id: "fixture/envelope".into(),
        artifact_id: "fixture/artifact".into(),
    };
    // Descriptive fixture grant admits no physical owner; this plan has zero
    // native calls and must run only in the explicit pure preparation mode.
    let grant = AuthorityGrant {
        grant_id: "fixture/control-grant".into(),
        contract_id: "conduitos.authority/usb-control@1".into(),
        host_call_contract_id: "conduit.host/usb-control@1".into(),
        subject_kind: "machine/usb/control".into(),
        host_id: native.host_id.clone(),
        boot_id: native.boot_id.clone(),
        capability_id: "conduitos/usb-control@1".into(),
    };
    let artifact = control_source_plan::prepare(
        protocol_source::usb_hid_absolute_descriptor_package().unwrap(),
        "usb-hid-absolute-layout",
        &native,
        &grant,
    )
    .unwrap();
    assert_eq!(
        artifact.artifact().definition().internal_plan.fragments[0]
            .placements
            .len(),
        9
    );
    let mut run = PreparedUsbSourceKernel::prepare_pure(
        artifact,
        conduit_composite::KernelCompositeSignStorage::default(),
    )
    .unwrap();
    let input_port = run.kernel_mut().definition().boundary.input_fronts[0]
        .external_port
        .clone();
    let output_port = run.kernel_mut().definition().boundary.output_fronts[0]
        .external_port
        .clone();
    let incoming = ValuePayload {
        value_kind: input_port.value_kind,
        encoded: input(&parser.programs[0].input_type, PREFIX),
    };
    let mut output = ValuePayload {
        value_kind: output_port.value_kind,
        encoded: Vec::with_capacity(4096),
    };
    let mut seen = false;
    let mut complete = false;
    let allocations = super::super::allocation::allocations(|| {
        run.kernel_mut().start().unwrap();
        run.kernel_mut()
            .admit_input(&input_port.port_id, 0, &incoming)
            .unwrap();
        run.kernel_mut().close_input(&input_port.port_id).unwrap();
        for _ in 0..4096 {
            let status = run.kernel_mut().step().unwrap();
            if let Some(request) = run.kernel_mut().next_host_request() {
                assert!(
                    run.service_pure_call(&request).unwrap(),
                    "no native call is allowed"
                );
            }
            if let Some(sequence) = run
                .kernel_mut()
                .output_into(&output_port.port_id, &mut output)
                .unwrap()
            {
                let layout = validate_canonical_structured_value(&output.encoded)
                    .unwrap()
                    .variant_payload("layout")
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    layout
                        .record_field("maximum-x")
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u16")
                        .unwrap(),
                    1024_u16.to_le_bytes()
                );
                assert_eq!(
                    layout
                        .record_field("maximum-y")
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u16")
                        .unwrap(),
                    600_u16.to_le_bytes()
                );
                run.kernel_mut()
                    .complete_output(&output_port.port_id, sequence)
                    .unwrap();
                assert!(!seen, "one descriptor must emit one terminal result");
                seen = true;
            }
            if status == conduit_composite::KernelCompositeStatus::Complete {
                complete = true;
                break;
            }
        }
    });
    assert!(
        seen,
        "finite descriptor scan must emit within its admitted work"
    );
    assert!(complete, "terminal state must close the finite scan");
    assert_eq!(allocations, 0);
}
