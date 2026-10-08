#![cfg(feature = "burn-model")]
#[path = "../../../mechanisms/implementations/burn-model/tests/common/mod.rs"]
mod common;
#[path = "../../../mechanisms/implementations/burn-model/tests/common/corpus.rs"]
mod corpus_fixture;
use conduit_ai::*;
use conduit_burn_model::{
    BurnAdapter, DeviceRequest, DirectoryCheckpointStore, InferenceContext, OptimizerRecipe,
};
use conduit_core::{BaseImplementationId, ConnectionTrack, PortDirection};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    hosted_burn_work::HostedBurnWork, ExternalForeDelivery, ExternalForeInput,
    ExternalForeOutputAdapter, StdHost, StdHostComposition, StdHostConfig, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("model journey needs no timer");
    }
}
#[derive(Default)]
struct Replies(Vec<ModelWorkReply>);
impl ExternalForeOutputAdapter for Replies {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0
                .push(ModelWorkReply::decode(&value.bytes).map_err(|e| format!("{e:?}"))?);
        }
        Ok(())
    }
}
fn make_plan(host: &StdHost) -> conduit_core::Plan {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_model_work_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../../../plots/model-authoring/main.conduit")),
        &startup,
    )
    .unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "model-authoring", &profile).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "request"),
        (PortDirection::Output, "reply"),
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
                byte_capacity: 1048576,
            },
        )
    })
    .collect();
    conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1048576,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap()
}
fn invoke(
    host: &mut StdHost,
    plan: &conduit_core::Plan,
    session: [u8; 32],
    generation: u64,
    sequence: u8,
    operation: ModelWorkOperation,
) -> ModelWorkReply {
    let request = ModelWorkRequest {
        request_identity: [sequence; 32],
        session_identity: session,
        expected_generation: generation,
        operation,
    };
    let mut replies = Replies::default();
    let report = host
        .run_external_plot_sequence_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "request".into(),
                track: ConnectionTrack::Payload,
                bytes: request.encode().unwrap(),
            }],
            &mut replies,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap_or_else(|error| {
            panic!("model operation {sequence}, generation {generation}: {error}")
        });
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: conduit_core::TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("ordinary installed Kernel runs");
    assert_eq!(
        kernel
            .kernel_sign
            .iter()
            .filter(|event| event.kind == conduit_kernel::KernelEventKind::HostCallRequested)
            .count(),
        1
    );
    assert_eq!(
        kernel
            .kernel_sign
            .iter()
            .filter(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted)
            .count(),
        1
    );
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(replies.0.len(), 1);
    let reply = replies.0.pop().unwrap();
    assert_eq!(reply.request_identity, [sequence; 32]);
    reply
}
#[test]
fn ordinary_plot_trains_saves_reloads_and_infers() {
    let context = common::context();
    let fresh_context = context.clone();
    let session = context.session.identity;
    let inference_artifact = context.artifact.clone();
    let determinism_profile = context.realization.deterministic_profile.clone();
    let training = BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        OptimizerRecipe {
            learning_rate: 0.05,
            weight_decay: 0.,
            gradient_clip: 10.,
            seed: 42,
        },
        context,
    )
    .unwrap();
    let inference_context = InferenceContext {
        artifact: inference_artifact,
        runtime: training.runtime().clone(),
        determinism_profile,
    };
    let fresh_inference_context = inference_context.clone();
    let directory = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(directory.path(), 65536, 16).unwrap();
    let adapter = HostedBurnWork::prepare(
        training,
        || common::RegressionDefinition,
        DeviceRequest::Cpu,
        inference_context,
        store,
        "regression-journey",
    )
    .unwrap();
    let mut host = StdHost::new_with_model_work(
        StdHostConfig {
            host_id: "model-host".into(),
            boot_id: "model-boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(adapter),
    )
    .unwrap();
    let plan = make_plan(&host);
    let batch = common::batch();
    let before = invoke(
        &mut host,
        &plan,
        session,
        0,
        1,
        ModelWorkOperation::Evaluate {
            batch: common::request(1).batch,
            inputs: batch.inputs.clone(),
            targets: batch.targets.clone(),
        },
    );
    for step in 1..=80 {
        let reply = invoke(
            &mut host,
            &plan,
            session,
            step - 1,
            (step + 1) as u8,
            ModelWorkOperation::Train {
                request: common::request(step),
                inputs: batch.inputs.clone(),
                targets: batch.targets.clone(),
            },
        );
        assert_eq!(reply.generation, step);
        assert!(matches!(
            reply.result,
            ModelWorkResult::Trained(TrainStepOutcome::Committed(_))
        ));
    }
    // A stale request is rejected through the real HostCall without advancing
    // the retained training owner; the next correct Play still uses generation80.
    let stale = ModelWorkRequest {
        request_identity: [91; 32],
        session_identity: session,
        expected_generation: 79,
        operation: ModelWorkOperation::Train {
            request: common::request(80),
            inputs: batch.inputs.clone(),
            targets: batch.targets.clone(),
        },
    };
    let mut refusals = Replies::default();
    host.run_external_plot_sequence_to(
        plan.fragments[0].clone(),
        &[ExternalForeInput {
            front_port_id: "request".into(),
            track: ConnectionTrack::Payload,
            bytes: stale.encode().unwrap(),
        }],
        &mut refusals,
        &mut Vec::new(),
        &mut NoTimer,
    )
    .unwrap();
    assert!(matches!(
        refusals.0[0].result,
        ModelWorkResult::Refused(ModelWorkRefusal::Training(TrainingRefusal::StaleState))
    ));
    assert_eq!(refusals.0[0].generation, 80);
    let after = invoke(
        &mut host,
        &plan,
        session,
        80,
        82,
        ModelWorkOperation::Evaluate {
            batch: common::request(81).batch,
            inputs: batch.inputs.clone(),
            targets: batch.targets.clone(),
        },
    );
    let (ModelWorkResult::Evaluated(before), ModelWorkResult::Evaluated(after)) =
        (before.result, after.result)
    else {
        panic!("evaluation receipts")
    };
    assert!(*after.metrics[0].value_millionths() < *before.metrics[0].value_millionths() / 10);
    let saved = invoke(
        &mut host,
        &plan,
        session,
        80,
        83,
        ModelWorkOperation::Checkpoint {
            metrics: after.metrics.clone(),
        },
    );
    let ModelWorkResult::Checkpointed(saved) = saved.result else {
        panic!("checkpoint receipt");
    };
    let resume_identity = saved.checkpoint.content.identity.digest();
    let export = invoke(
        &mut host,
        &plan,
        session,
        80,
        84,
        ModelWorkOperation::Export,
    );
    let ModelWorkResult::Exported(checkpoint) = export.result else {
        panic!("export receipt")
    };
    let identity = checkpoint.content.identity.digest();
    invoke(
        &mut host,
        &plan,
        session,
        80,
        85,
        ModelWorkOperation::ReloadInference {
            checkpoint_identity: identity,
        },
    );
    let result = invoke(
        &mut host,
        &plan,
        session,
        80,
        86,
        ModelWorkOperation::Infer {
            inputs: batch.inputs.clone(),
        },
    );
    let ModelWorkResult::Inferred {
        checkpoint_identity,
        outputs,
    } = result.result
    else {
        panic!("inference result")
    };
    assert_eq!(checkpoint_identity, Some(identity));
    assert_eq!(outputs.len(), 1);
    drop(host);
    let fresh_training = BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        OptimizerRecipe {
            learning_rate: 0.05,
            weight_decay: 0.,
            gradient_clip: 10.,
            seed: 42,
        },
        fresh_context,
    )
    .unwrap();
    let fresh_store = DirectoryCheckpointStore::new(directory.path(), 65536, 16).unwrap();
    let fresh_adapter = HostedBurnWork::prepare(
        fresh_training,
        || common::RegressionDefinition,
        DeviceRequest::Cpu,
        fresh_inference_context,
        fresh_store,
        "regression-resumed",
    )
    .unwrap();
    let mut fresh_host = StdHost::new_with_model_work(
        StdHostConfig {
            host_id: "fresh-model-host".into(),
            boot_id: "fresh-model-boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::minimal(),
        Box::new(fresh_adapter),
    )
    .unwrap();
    let fresh_plan = make_plan(&fresh_host);
    let resumed = invoke(
        &mut fresh_host,
        &fresh_plan,
        session,
        0,
        87,
        ModelWorkOperation::Resume {
            checkpoint_identity: resume_identity,
        },
    );
    assert_eq!(resumed.generation, 80);
    invoke(
        &mut fresh_host,
        &fresh_plan,
        session,
        80,
        88,
        ModelWorkOperation::ReloadInference {
            checkpoint_identity: identity,
        },
    );
    let reloaded = invoke(
        &mut fresh_host,
        &fresh_plan,
        session,
        80,
        89,
        ModelWorkOperation::Infer {
            inputs: batch.inputs.clone(),
        },
    );
    let ModelWorkResult::Inferred {
        outputs: reloaded, ..
    } = reloaded.result
    else {
        panic!("fresh inference")
    };
    assert_eq!(reloaded, outputs);
    let advanced = invoke(
        &mut fresh_host,
        &fresh_plan,
        session,
        80,
        90,
        ModelWorkOperation::Train {
            request: common::request(81),
            inputs: batch.inputs,
            targets: batch.targets,
        },
    );
    assert_eq!(advanced.generation, 81);
    println!("ordinary model Plot: loss {} -> {} millionths; 80 steps; fresh resume80 ->81; exact inference reload; 90 successful HostCalls; stale step refused", before.metrics[0].value_millionths(),after.metrics[0].value_millionths());
}

#[test]
fn ordinary_plot_train_next_resumes_actual_corpus_on_a_fresh_host() {
    fn corpus_host(directory: &std::path::Path, name: &str) -> StdHost {
        let context = common::context();
        let corpus = corpus_fixture::finite_corpus(&context, 41, false);
        let mut training = BurnAdapter::initialize(
            common::RegressionDefinition,
            DeviceRequest::Cpu,
            OptimizerRecipe {
                learning_rate: 0.05,
                weight_decay: 0.,
                gradient_clip: 10.,
                seed: 42,
            },
            context.clone(),
        )
        .unwrap();
        training.attach_corpus(corpus).unwrap();
        let inference = InferenceContext {
            artifact: context.artifact,
            runtime: training.runtime().clone(),
            determinism_profile: context.realization.deterministic_profile,
        };
        let store = DirectoryCheckpointStore::new(directory, 65536, 16).unwrap();
        let adapter = HostedBurnWork::prepare(
            training,
            || common::RegressionDefinition,
            DeviceRequest::Cpu,
            inference,
            store,
            name,
        )
        .unwrap();
        StdHost::new_with_model_work(
            StdHostConfig {
                host_id: name.into(),
                boot_id: format!("{name}/boot").into(),
                offer_generation: conduit_core::OfferGeneration(1),
            },
            StdHostComposition::minimal(),
            Box::new(adapter),
        )
        .unwrap()
    }
    let directory = tempfile::tempdir().unwrap();
    let mut host = corpus_host(directory.path(), "corpus-first");
    let plan = make_plan(&host);
    let session = common::context().session.identity;
    for (step, batch) in [(1, 32), (2, 31), (3, 32)] {
        let reply = invoke(
            &mut host,
            &plan,
            session,
            step - 1,
            step as u8,
            ModelWorkOperation::TrainNext,
        );
        let ModelWorkResult::Trained(TrainStepOutcome::Committed(result)) = reply.result else {
            panic!("committed corpus step")
        };
        assert_eq!(result.receipt.batch_identity, [batch; 32]);
    }
    let metrics = invoke(
        &mut host,
        &plan,
        session,
        3,
        10,
        ModelWorkOperation::Evaluate {
            batch: common::request(1).batch,
            inputs: common::batch().inputs,
            targets: common::batch().targets,
        },
    );
    let ModelWorkResult::Evaluated(metrics) = metrics.result else {
        panic!("evaluation")
    };
    let saved = invoke(
        &mut host,
        &plan,
        session,
        3,
        11,
        ModelWorkOperation::Checkpoint {
            metrics: metrics.metrics,
        },
    );
    let ModelWorkResult::Checkpointed(saved) = saved.result else {
        panic!("durable checkpoint")
    };
    let id = saved.checkpoint.content.identity.digest();
    drop(host);
    let mut fresh = corpus_host(directory.path(), "corpus-fresh");
    let plan = make_plan(&fresh);
    let resumed = invoke(
        &mut fresh,
        &plan,
        session,
        0,
        12,
        ModelWorkOperation::Resume {
            checkpoint_identity: id,
        },
    );
    assert_eq!(resumed.generation, 3);
    for (step, batch) in [(4, 31), (5, 31), (6, 32), (7, 31), (8, 32)] {
        let reply = invoke(
            &mut fresh,
            &plan,
            session,
            step - 1,
            step as u8 + 12,
            ModelWorkOperation::TrainNext,
        );
        let ModelWorkResult::Trained(TrainStepOutcome::Committed(result)) = reply.result else {
            panic!("committed corpus resume")
        };
        assert_eq!(result.receipt.batch_identity, [batch; 32]);
        assert_eq!(result.state.completed_steps, step);
    }
    let exhausted = invoke(
        &mut fresh,
        &plan,
        session,
        8,
        30,
        ModelWorkOperation::TrainNext,
    );
    assert!(matches!(
        exhausted.result,
        ModelWorkResult::Refused(ModelWorkRefusal::ResourceBound)
    ));
    assert_eq!(exhausted.generation, 8);
}
