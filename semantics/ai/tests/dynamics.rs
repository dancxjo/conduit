use conduit_ai::*;
use conduit_core::{QuantityUnit, StateContinuation};
use conduit_data::{
    tensor_content_digest, SampledSignal, SignalCadence, SignalContinuity, SignalStart, TensorAxis,
    TensorAxisRole, TensorBacking, TensorElement, TensorValue,
};

fn tensor(
    element: TensorElement,
    dimensions: Vec<u64>,
    axes: Vec<TensorAxis>,
    bytes: Vec<u8>,
) -> TensorValue {
    TensorValue {
        element,
        dimensions,
        axes,
        content_digest: tensor_content_digest(&bytes),
        backing: TensorBacking::Inline(bytes),
    }
}

fn state(values: [f64; 4]) -> DynamicsState {
    let bytes = values.into_iter().flat_map(f64::to_le_bytes).collect();
    DynamicsState {
        identity: "tongues/coupled-oscillator-state".into(),
        schema_version: 1,
        generation: 7,
        value: tensor(
            TensorElement::F64,
            vec![4],
            vec![TensorAxis {
                role: TensorAxisRole::Feature,
                identity: Some("oscillator-position-velocity".into()),
                unit: None,
            }],
            bytes,
        ),
    }
}

fn contract() -> IntegrateContract {
    IntegrateContract {
        identity: [41; 32],
        vector_field_artifact_identity: [42; 32],
        interval: IntegrationInterval {
            start: 0,
            end: 1_000,
            unit: QuantityUnit::Millisecond,
        },
        sampling: OutputSamplingGrid {
            clock_identity: "experiment/monotonic-ms".into(),
            coordinates: vec![0, 250, 500, 750, 1_000],
        },
        profile: DynamicsProfile::DeterministicOde,
        accuracy: IntegrationAccuracy {
            absolute_tolerance_millionths: 10,
            relative_tolerance_millionths: 10,
            maximum_estimated_error_millionths: 1_000,
        },
        resources: IntegrationResourceEnvelope {
            maximum_state_bytes: 128,
            maximum_context_bytes: 128,
            maximum_output_samples: 8,
            maximum_output_bytes: 512,
            maximum_internal_steps: 128,
            maximum_function_evaluations: 512,
            maximum_work_units: 2_048,
            memory_ceiling_bytes: 16_384,
        },
    }
}

fn candidate(solver: &str, internal_steps: u64) -> IntegrationCandidate {
    let coordinates = [0_i64, 250, 500, 750, 1_000]
        .into_iter()
        .flat_map(i64::to_le_bytes)
        .collect::<Vec<_>>();
    let samples = [
        1.0_f64, 0.0, -1.0, 0.0, 0.92, -0.38, -0.92, 0.38, 0.70, -0.70, -0.70, 0.70, 0.38, -0.92,
        -0.38, 0.92, 0.0, -1.0, 0.0, 1.0,
    ]
    .into_iter()
    .flat_map(f64::to_le_bytes)
    .collect::<Vec<_>>();
    let final_bytes = [0.0_f64, -1.0, 0.0, 1.0]
        .into_iter()
        .flat_map(f64::to_le_bytes)
        .collect::<Vec<_>>();
    IntegrationCandidate {
        trajectory: SampledSignal {
            clock_identity: "experiment/monotonic-ms".into(),
            start: SignalStart::SampleIndex(0),
            cadence: SignalCadence::Irregular {
                coordinates: Box::new(tensor(
                    TensorElement::I64,
                    vec![5],
                    vec![TensorAxis {
                        role: TensorAxisRole::Time,
                        identity: Some("requested-output-grid".into()),
                        unit: Some(QuantityUnit::Millisecond),
                    }],
                    coordinates,
                )),
            },
            sample_count: 5,
            continuity: SignalContinuity::Continuous,
            samples: tensor(
                TensorElement::F64,
                vec![5, 4],
                vec![
                    TensorAxis {
                        role: TensorAxisRole::Time,
                        identity: Some("requested-output-grid".into()),
                        unit: Some(QuantityUnit::Millisecond),
                    },
                    TensorAxis {
                        role: TensorAxisRole::Feature,
                        identity: Some("oscillator-position-velocity".into()),
                        unit: None,
                    },
                ],
                samples,
            ),
        },
        final_state: tensor(
            TensorElement::F64,
            vec![4],
            vec![TensorAxis {
                role: TensorAxisRole::Feature,
                identity: Some("oscillator-position-velocity".into()),
                unit: None,
            }],
            final_bytes,
        ),
        internal_steps,
        function_evaluations: internal_steps * 4,
        consumed_work_units: internal_steps * 5,
        estimated_error_millionths: 50,
        realization: SolverRealization {
            implementation_identity: format!("host/solver/{solver}"),
            solver_family: solver.into(),
            adapter_name: "reference-dynamics".into(),
            adapter_version: "1".into(),
            runtime_build_identity: format!("build/{solver}/1"),
            device_profile: "cpu/f64".into(),
        },
    }
}

#[test]
fn two_solver_realizations_share_one_contract_and_emit_sampled_coupled_trajectory() {
    let contract = contract();
    contract.validate().unwrap();
    let initial = state([1.0, 0.0, -1.0, 0.0]);
    let boundary = contract.planned_state_boundary(&initial).unwrap();
    assert_eq!(
        boundary.continuation,
        StateContinuation::MaximumTransitions(1)
    );
    let request = IntegrateRequest {
        expected_generation: 7,
        initial_state: initial,
        context: vec![],
    };

    let rk = contract
        .realize(
            &request,
            HostIntegrationTerminal::Candidate(Box::new(candidate("adaptive-rk", 20))),
        )
        .unwrap();
    let fixed = contract
        .realize(
            &request,
            HostIntegrationTerminal::Candidate(Box::new(candidate("fixed-step", 40))),
        )
        .unwrap();
    let (IntegrationOutcome::Completed(rk), IntegrationOutcome::Completed(fixed)) = (rk, fixed)
    else {
        panic!("both admitted candidates must complete")
    };
    assert_eq!(rk.next_state.generation, 8);
    assert_eq!(
        rk.trajectory.semantic_digest(),
        fixed.trajectory.semantic_digest()
    );
    assert_eq!(
        rk.receipt.contract_descriptor_identity,
        fixed.receipt.contract_descriptor_identity
    );
    assert_ne!(rk.receipt.realization, fixed.receipt.realization);
    assert_ne!(rk.receipt.internal_steps, rk.trajectory.sample_count);
    assert_ne!(fixed.receipt.internal_steps, fixed.trajectory.sample_count);
}

#[test]
fn work_exhaustion_cancellation_and_failures_never_commit_state() {
    let contract = contract();
    let request = IntegrateRequest {
        expected_generation: 7,
        initial_state: state([1.0, 0.0, -1.0, 0.0]),
        context: vec![],
    };
    for terminal in [
        IntegrationTerminal::WorkLimitExhausted,
        IntegrationTerminal::Cancelled,
        IntegrationTerminal::Discontinuity,
        IntegrationTerminal::ProviderLost,
        IntegrationTerminal::Failed,
    ] {
        assert_eq!(
            contract
                .realize(&request, HostIntegrationTerminal::NoCommit(terminal))
                .unwrap(),
            IntegrationOutcome::NotCommitted {
                terminal,
                retained_generation: 7
            }
        );
    }

    let mut overrun = candidate("adaptive-rk", 20);
    overrun.function_evaluations = contract.resources.maximum_function_evaluations + 1;
    assert_eq!(
        contract.realize(
            &request,
            HostIntegrationTerminal::Candidate(Box::new(overrun))
        ),
        Err(DynamicsRefusal::WorkBoundExceeded)
    );
}

#[test]
fn exact_grid_stale_state_resource_bounds_and_unsupported_sde_refuse() {
    let contract = contract();
    let mut request = IntegrateRequest {
        expected_generation: 6,
        initial_state: state([1.0, 0.0, -1.0, 0.0]),
        context: vec![],
    };
    assert_eq!(
        contract.realize(
            &request,
            HostIntegrationTerminal::Candidate(Box::new(candidate("fixed-step", 40)))
        ),
        Err(DynamicsRefusal::StaleState)
    );
    request.expected_generation = 7;

    let mut wrong_grid = candidate("fixed-step", 40);
    let SignalCadence::Irregular { coordinates } = &mut wrong_grid.trajectory.cadence else {
        unreachable!()
    };
    let wrong = [0_i64, 200, 500, 750, 1_000]
        .into_iter()
        .flat_map(i64::to_le_bytes)
        .collect::<Vec<_>>();
    coordinates.content_digest = tensor_content_digest(&wrong);
    coordinates.backing = TensorBacking::Inline(wrong);
    assert_eq!(
        contract.realize(
            &request,
            HostIntegrationTerminal::Candidate(Box::new(wrong_grid))
        ),
        Err(DynamicsRefusal::InvalidTrajectory)
    );

    let mut sde = contract.clone();
    sde.profile = DynamicsProfile::Stochastic {
        profile: "ito/additive-gaussian".into(),
        randomness: RandomnessProfile::ExplicitSeed(9),
    };
    assert_eq!(
        sde.validate(),
        Err(DynamicsRefusal::UnsupportedStochasticProfile)
    );

    let mut unbounded = contract;
    unbounded.resources.maximum_internal_steps = 0;
    assert_eq!(unbounded.validate(), Err(DynamicsRefusal::InvalidResources));
}
