//! Genuine reference-linear inference through authored Source and installed HostCall.
use conduit_ai::model_infer_catalog::*;
use conduit_ai::*;
use conduit_core::*;
use conduit_data::{
    tensor_content_digest, NamedTensor, NamedTensorBundle, TensorAxis, TensorAxisRole,
    TensorBacking, TensorElement, TensorValue,
};
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::hosted_model_infer::prepare_linear_model;
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost,
    StdHostComposition, StdHostConfig, TimerAdapter,
};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
fn offer() -> ModelComputeOffer {
    ModelComputeOffer {
        identity: "std/linear-f32".into(),
        supported_operations: vec![ModelComputeOperation::Inference],
        accepted_formats: vec!["model/reference-linear".into()],
        supported_elements: vec![TensorElement::F32],
        solver_profiles: vec![],
        determinism_profiles: vec!["deterministic/f32".into()],
        checkpoint_loading: false,
        checkpoint_writing: false,
        limits: ModelComputeLimits {
            maximum_model_bytes: 1024,
            maximum_working_memory_bytes: 1024,
            maximum_device_memory_bytes: 0,
            maximum_input_bytes: 8,
            maximum_output_bytes: 8,
            maximum_batch_items: 1,
            maximum_rank: 1,
            maximum_in_flight: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 8,
            cancellation_supported: true,
            compute: ComputeCapacity {
                class: PortableComputeClass::GeneralCpu,
                minimum_lanes: 1,
                preferred_lanes: 1,
                maximum_lanes: 1,
                service: ComputeServiceGuarantee::Shared,
            },
        },
        cache_policy: ModelCachePolicy::bounded(1024, 1).unwrap(),
    }
}

fn runtime(adapter: &str) -> ModelComputeRuntimeIdentity {
    ModelComputeRuntimeIdentity {
        provider_name: "conduit-std".into(),
        runtime_name: "native-rust".into(),
        runtime_version: "1".into(),
        runtime_build_identity: format!("build/{adapter}/1"),
        adapter_artifact_identity: format!("adapter/{adapter}/1"),
        device_evidence: "observed/cpu".into(),
        precision_profile: "f32".into(),
    }
}

fn input(values: [f32; 2]) -> TensorValue {
    let bytes = values
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    TensorValue {
        element: TensorElement::F32,
        dimensions: BoundedSequence::try_from_iter([2]).unwrap(),
        axes: BoundedSequence::try_from_iter([TensorAxis {
            role: TensorAxisRole::Feature,
            identity: Some("linear-feature".into()),
            unit: None,
        }])
        .unwrap(),
        content_digest: tensor_content_digest(&bytes),
        backing: TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap()),
    }
}

fn bundle(value: TensorValue) -> NamedTensorBundle {
    NamedTensorBundle::from_values(vec![NamedTensor {
        name: "input".into(),
        value,
    }])
    .unwrap()
}

struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("no timer is required");
    }
}
#[derive(Default)]
struct Outputs(Vec<NamedTensorBundle>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0
                .push(NamedTensorBundle::decode(&value.bytes).map_err(|e| format!("{e:?}"))?);
        }
        Ok(())
    }
}
fn model(signature: ModelSignature) -> AdmittedModelResource {
    let bytes: Arc<[u8]> = Arc::from(
        [2_f32, 0., 0., -1., 1., 4.]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(model_content_digest(&bytes)),
        content_profile: "model/reference-linear".into(),
        access_class: "test/immutable-linear".into(),
        extent: ResourceExtent {
            bytes: 24,
            items: None,
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([7; 32]),
            expires_at: None,
        },
    };
    let binding = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: "test/local-linear".into(),
        authority_contract: MODEL_READ_AUTHORITY.into(),
        authority_grant: "test/linear-read".into(),
        maximum_bytes: 24,
        maximum_items: None,
        availability: ResourceReferenceAvailability::Available,
    };
    let artifact = ModelArtifact {
        architecture_profile: "reference/affine-2x2@1".into(),
        format_profile: "model/reference-linear".into(),
        precision_profile: "f32".into(),
        state_schema_version: 1,
        signature_identity: signature.semantic_digest().unwrap(),
        content: reference,
    };
    AdmittedModelResource::adopt(artifact, signature, bytes, &binding).unwrap()
}
fn requirement() -> ModelComputeRequirement {
    ModelComputeRequirement {
        operation: ModelComputeOperation::Inference,
        model_format: "model/reference-linear".into(),
        elements: vec![TensorElement::F32],
        rank: 1,
        model_bytes: 24,
        working_memory_bytes: 64,
        device_memory_bytes: 0,
        input_bytes: 8,
        output_bytes: 8,
        batch_items: 1,
        compute_class: PortableComputeClass::GeneralCpu,
        minimum_lanes: 1,
        preferred_lanes: 1,
        maximum_lanes: 1,
        minimum_service: ComputeServiceGuarantee::Shared,
        solver_profile: None,
        determinism_profile: "deterministic/f32".into(),
        requires_checkpoint_load: false,
        requires_checkpoint_write: false,
    }
}
#[test]
fn source_selects_admitted_model_and_kernel_executes_its_coefficients() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_model_infer_catalog(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(include_str!("fixtures/model-linear.conduit"));
    let checked = check_syntax_document(&syntax, &startup)
        .map_err(|e| format!("{}: {}", e.code, e.message))
        .unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "linear-model", &profile).unwrap();
    let signature = configured_model_signature(&authored.expanded.gears[0].configuration).unwrap();
    let adapter =
        prepare_linear_model(model(signature), offer(), runtime("linear"), requirement()).unwrap();
    let mut host = StdHost::new_with_model_work(
        StdHostConfig {
            host_id: "model-infer-host".into(),
            boot_id: "model-infer-boot".into(),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(adapter),
    )
    .unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "inputs"),
        (PortDirection::Output, "outputs"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 262144,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 262144,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    let mut outputs = Outputs::default();
    let report = host
        .run_external_plot_sequence_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "inputs".into(),
                track: ConnectionTrack::Payload,
                bytes: bundle(input([2., 3.])).encode().unwrap(),
            }],
            &mut outputs,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert!(matches!(
        report.observations.last().map(|o| &o.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("ordinary kernel");
    for kind in [
        conduit_kernel::KernelEventKind::HostCallRequested,
        conduit_kernel::KernelEventKind::HostCallCompleted,
    ] {
        assert_eq!(
            kernel.kernel_sign.iter().filter(|e| e.kind == kind).count(),
            1
        );
    }
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(outputs.0.len(), 1);
    let output = &outputs.0[0].get()[0];
    assert_eq!(output.name, "output");
    let TensorBacking::Inline(bytes) = &output.value.backing else {
        panic!("inline output");
    };
    let values = bytes
        .as_slice()
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(values, [5., 1.]);
}

#[test]
fn preparation_refuses_ineligible_compute_and_output_below_signature_maximum() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_model_infer_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("fixtures/model-linear.conduit")),
        &startup,
    )
    .map_err(|e| format!("{}: {}", e.code, e.message))
    .unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "linear-model", &profile).unwrap();
    let signature = configured_model_signature(&authored.expanded.gears[0].configuration).unwrap();
    let mut scarce = offer();
    scarce.limits.maximum_working_memory_bytes = 32;
    let error = prepare_linear_model(
        model(signature.clone()),
        scarce,
        runtime("linear"),
        requirement(),
    )
    .err()
    .expect("insufficient memory must refuse before advertisement");
    assert!(error.contains("ResourceBoundExceeded"), "{error}");
    let mut wrong_dtype = offer();
    wrong_dtype.supported_elements = vec![TensorElement::F64];
    let error = prepare_linear_model(
        model(signature.clone()),
        wrong_dtype,
        runtime("linear"),
        requirement(),
    )
    .err()
    .expect("ineligible provider must refuse before advertisement");
    assert!(error.contains("UnsupportedElement"), "{error}");
    let mut short = requirement();
    short.output_bytes = 4;
    let error = prepare_linear_model(model(signature), offer(), runtime("linear"), short)
        .err()
        .expect("all declared output bytes must be admitted before advertisement");
    assert!(
        error.contains("signature exceeds admitted tensor bytes"),
        "{error}"
    );
}
