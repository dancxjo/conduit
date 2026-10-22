//! Explicit installed-runtime proof; normal CI compiles, supported xtask executes.
#![cfg(all(
    feature = "native-model-backing",
    target_os = "linux",
    target_arch = "x86_64",
    target_env = "gnu"
))]
#[path = "native_model_infer/allocation.rs"]
mod allocation;
#[path = "common_model_infer/mod.rs"]
mod common;
#[path = "native_model_infer/fixture.rs"]
mod fixture;
use conduit_ai::{
    imported_back_admission::ImportedBackAdmissionReceipt,
    imported_model_descriptor::ImportedModelInspection, ModelComputeRuntimeIdentity,
};
use conduit_core::{ConnectionTrack, ObservationKind, TerminalDisposition};
use conduit_data::NamedTensorBundle;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{hosted_native_model::prepare_native_model, ExternalForeInput};
use sha2::{Digest, Sha256};
#[test]
#[ignore = "explicit installed runtime required; cargo xtask check model-native-inference"]
fn actual_native_identity_preserves_source_plan_play_bits_without_parent_growth() {
    let runtime = fixture::profile();
    let mut receipts = Vec::new();
    for bits in fixture::BIT_CONTROLS {
        let mut actual: Option<(ModelComputeRuntimeIdentity, ImportedBackAdmissionReceipt)> = None;
        let (mut host, plan) = common::host_plan(fixture::SOURCE, |signature| {
            let prepared = prepare_native_model(
                fixture::model(signature),
                fixture::worker(),
                runtime.clone(),
                fixture::grant(),
            )
            .unwrap();
            actual = Some((
                prepared.runtime_identity().clone(),
                prepared.imported_admission().unwrap().clone(),
            ));
            Box::new(prepared)
        });
        let (actual_runtime, admission) = actual.unwrap();
        assert_eq!(actual_runtime.provider_name, "CPUExecutionProvider");
        assert_eq!(actual_runtime.runtime_name, "onnxruntime");
        assert_eq!(actual_runtime.runtime_version, "1.23.2");
        assert_eq!(actual_runtime.precision_profile, "tensor/f32@1");
        let expected_runtime_digest: String = runtime
            .runtime_sha256
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            actual_runtime.runtime_build_identity,
            expected_runtime_digest
        );
        let placement = &plan.fragments[0].placements[0];
        admission
            .verify_selected(
                placement.implementation_id.as_str(),
                placement.artifact_id.as_str(),
                &actual_runtime,
                placement.execution_profile_id.as_str(),
            )
            .unwrap();
        let inspection = ImportedModelInspection::from_planned_gear(placement).unwrap();
        assert_eq!(inspection.admission_identity, admission.identity());
        assert_eq!(inspection.checkpoint, *admission.checkpoint());
        assert_eq!(inspection.inputs[0].semantic_port, "scalar");
        assert_eq!(inspection.inputs[0].graph_name, "x");
        assert_eq!(inspection.outputs[0].graph_name, "y");
        let input = fixture::scalar(bits);
        let inputs = [ExternalForeInput {
            front_port_id: "inputs".into(),
            track: ConnectionTrack::Payload,
            bytes: input.clone().encode().unwrap(),
        }];
        let mut sink = allocation::Sink::new();
        let mut probe = allocation::Probe::default();
        let mut events = Vec::new();
        let report = host.run_external_plot_to(
            plan.fragments[0].clone(),
            &inputs,
            &mut sink,
            &mut events,
            &mut probe,
        );
        allocation::stop();
        let report = report.unwrap();
        assert_eq!(probe.phases, 2);
        assert_eq!(probe.calls, 0);
        assert_eq!(probe.bytes, 0);
        assert_eq!(sink.calls, 1);
        assert!(matches!(
            report.observations.last().map(|v| &v.kind),
            Some(ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            })
        ));
        let kernel = report.kernel.unwrap();
        assert_eq!(kernel.active_play.plan_id, plan.plan_id);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        let output = NamedTensorBundle::decode(&sink.bytes[..sink.length]).unwrap();
        assert_eq!(output, input);
        let conduit_data::TensorBacking::Inline(actual_bytes) = &output.get()[0].value.backing
        else {
            panic!("admitted scalar output is inline");
        };
        let output_bits = u32::from_le_bytes(actual_bytes.as_slice().try_into().unwrap());
        assert_eq!(output_bits, bits);
        receipts.push(serde_json::json!({"input_bits":bits,"output_bits":output_bits,"source":plan.source_document_id.as_str(),"plan":plan.plan_id.as_str(),"play":kernel.active_play_id.as_str(),"graph":inspection.checkpoint.graph,"sidecar":inspection.checkpoint.sidecar,"admission":inspection.admission_identity,"parent_play_allocations":probe.calls,"parent_play_requested_bytes":probe.bytes}));
    }
    println!(
        "{}",
        serde_json::json!({"proof":"native-model-source-plan-play","outputs":receipts,"runtime_sha256":runtime.runtime_sha256,"worker_sha256":Sha256::digest(std::fs::read(fixture::worker()).unwrap()).as_slice(),"not_established":["interruptible cancellation","cross-session warm reuse/retirement","stable publication"]})
    );
}

#[test]
#[ignore = "explicit installed runtime required; cargo xtask check model-native-inference"]
fn native_admission_failures_propagate_before_host_capability_publication() {
    use conduit_std_host::hosted_native_model::NativeModelPrepareRefusal;
    use conduit_std_host::native_model_backing::NativeBackingRefusal;
    let runtime = fixture::profile();
    let _ = common::host_plan(fixture::SOURCE, |signature| {
        let mut wrong = runtime.clone();
        wrong.runtime_sha256[0] ^= 1;
        assert!(matches!(
            prepare_native_model(
                fixture::model(signature.clone()),
                fixture::worker(),
                wrong,
                fixture::grant()
            ),
            Err(NativeModelPrepareRefusal::Native(
                NativeBackingRefusal::UnsupportedProfile
            ))
        ));
        let mut insufficient = fixture::grant();
        insufficient.maximum_private_bytes = 1;
        assert!(matches!(
            prepare_native_model(
                fixture::model(signature.clone()),
                fixture::worker(),
                runtime.clone(),
                insufficient
            ),
            Err(NativeModelPrepareRefusal::Native(
                NativeBackingRefusal::InsufficientBacking { .. }
            ))
        ));
        Box::new(
            prepare_native_model(
                fixture::model(signature),
                fixture::worker(),
                runtime.clone(),
                fixture::grant(),
            )
            .unwrap(),
        )
    });
}
