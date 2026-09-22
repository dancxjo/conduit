//! Fixed deterministic topology proving Pete workload planning and projection.

#[cfg(test)]
mod tests {
    use conduit_ai::{TemporalEvidenceSelection, TemporalRetrievalIntent};
    use conduit_body::{Body, BodyFormPlan, BodyPlan, BodyPlayIdentity, BodyWorkset};
    use conduit_composite::{
        KernelCompositeBoundary, KernelCompositeDefinition, KernelCompositeFrontBinding,
        KernelCompositeHost, KernelOperationRegistry,
    };
    use conduit_core::{
        ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
        FailureReason, HostAdvertisement, HostCallContractId, HostCallRequirement, HostId,
        HostProfileId, ImplementationId, ImplementationOffer, OfferGeneration, Plan, SignId,
        TemporalInstant, TemporalScale, ValuePayload,
    };
    use conduit_form::{CompositeFrontTerminal, ExpandedAuthoringForm};
    use conduit_human::{
        body_self_experience, BodySelfObservation, CurrentExperience, ExperienceCertainty,
        ExperienceLimits, ExperienceSourceRef, ExperienceTemporalPolicy, ExperienceTemporalRole,
        SourceAvailability, SourceObservation, MAXIMUM_BODY_SELF_STATE_BYTES,
    };
    use conduit_pete::{
        reviewed_pete_workload, BoundedAutobiography, CapabilityCondition, ExperienceCandidate,
        ExperienceKind, ExperienceProvenance, HomeostasisHostAdapter, HomeostasisKernelFactory,
        HomeostaticInputs, MemoryQueryRefusal, PeteWorkloadRole, PowerCondition, PressureCondition,
        ReviewedPeteWorkload, SafetyCondition, Sensitivity, ThermalCondition,
        HOMEOSTASIS_IMPLEMENTATION, HOMEOSTASIS_POLICY_REVISION, HOMEOSTATIC_STATE_KIND,
    };
    use conduit_planner::{default_expanded_placements, plan_expanded_canonical};
    use conduit_presentation::{
        Presentation, PresentationBasis, PresentationDisclosure, PresentationDisclosureLevel,
        PresentationRole, PresentationSubject, PresentationText,
    };
    use conduit_std_host::StdHost;
    use patchbay_model::{
        inspect_presented_current_experience, project_body_plan, BodyPlanningSession,
        BodyPlanningTransition,
    };

    #[test]
    fn one_body_spans_three_required_hosts_and_survives_optional_browser_loss() {
        let workload = reviewed_pete_workload().unwrap();
        let forebrain = proof_host(
            &workload,
            "proof/pete-forebrain",
            "proof/forebrain-boot",
            &[
                PeteWorkloadRole::HistoricalIndex,
                PeteWorkloadRole::Conversation,
                PeteWorkloadRole::Homeostasis,
            ],
        );
        let motherbrain = proof_host(
            &workload,
            "proof/pete-motherbrain",
            "proof/motherbrain-boot",
            &[PeteWorkloadRole::AutobiographicalMemory],
        );
        let brainstem = proof_host(
            &workload,
            "proof/pete-brainstem",
            "proof/brainstem-boot",
            &[PeteWorkloadRole::Situation],
        );
        let browser = proof_host(
            &workload,
            "proof/pete-optional-browser",
            "proof/browser-boot",
            &[PeteWorkloadRole::Conversation],
        );
        let initial_hosts = [
            forebrain.clone(),
            motherbrain.clone(),
            brainstem.clone(),
            browser,
        ];
        let body = Body::born_with_forms(
            workload.initial.clone(),
            21,
            SignId::from("proof/pete-body-born"),
        )
        .unwrap();
        let mut session = BodyPlanningSession::start(
            &body,
            1,
            SignId::from("proof/pete-wake"),
            proof_plans(&workload, &initial_hosts, true),
            SignId::from("proof/pete-plan-a-ready"),
            1,
            SignId::from("proof/pete-play-a-started"),
        )
        .unwrap();
        let first = project_body_plan(session.current_plan());
        assert_eq!(first.body_id, body.body_id);
        assert_eq!(first.active_forms.len(), 5);
        assert!(first
            .active_forms
            .iter()
            .all(|form| !form.placements.is_empty()));
        assert_eq!(hosts_in(&first).len(), 4);
        assert!(hosts_in(&first).contains("proof/pete-forebrain"));
        assert!(hosts_in(&first).contains("proof/pete-motherbrain"));
        assert!(hosts_in(&first).contains("proof/pete-brainstem"));
        assert!(hosts_in(&first).contains("proof/pete-optional-browser"));

        let old_plan = first.clone();
        let required_hosts = [forebrain, motherbrain, brainstem];
        session
            .replan(
                proof_plans(&workload, &required_hosts, false),
                BodyPlanningTransition {
                    unsatisfied_sign_id: Some(SignId::from("proof/pete-browser-offers-left")),
                    plan_ready_sign_id: SignId::from("proof/pete-plan-b-ready"),
                    play_sequence: 2,
                    play_started_sign_id: SignId::from("proof/pete-play-b-started"),
                },
            )
            .unwrap();
        let replacement = project_body_plan(session.current_plan());
        assert_eq!(replacement.body_id, first.body_id);
        assert_ne!(replacement.plan_id, first.plan_id);
        assert_eq!(replacement.active_forms.len(), 5);
        assert_eq!(hosts_in(&replacement).len(), 3);
        assert!(!hosts_in(&replacement).contains("proof/pete-optional-browser"));
        assert!(hosts_in(&replacement).contains("proof/pete-forebrain"));
        assert_eq!(first, old_plan, "the replaced Plan projection is immutable");
    }

    #[test]
    fn second_episode_retrieves_first_and_provider_loss_stays_explicit() {
        let workload = reviewed_pete_workload().unwrap();
        let body =
            Body::born_with_forms(workload.initial, 21, SignId::from("proof/pete-body-born"))
                .unwrap();
        let body_identity = body.body_id.as_str().to_owned();
        let mut memory = BoundedAutobiography::new(4).unwrap();
        memory
            .retain(
                ExperienceCandidate {
                    identity: "experience/pete/episode-a/resistor-change".into(),
                    kind: ExperienceKind::HumanStatement,
                    content: b"I changed the resistor.".to_vec(),
                    provenance: vec![ExperienceProvenance {
                        source_identity: "sign/human/episode-a/utterance".into(),
                        event_at_millis: 1_000,
                        recorded_at_millis: 1_001,
                        body_identity: body_identity.clone(),
                        host_identity: Some("proof/pete-motherbrain".into()),
                        boot_identity: Some("proof/motherbrain-boot".into()),
                        plan_identity: Some("proof/pete-plan-a".into()),
                        play_identity: Some("proof/pete-play-a".into()),
                    }],
                    sensitivity: Sensitivity::LocalPrivate,
                    supersedes: None,
                    explicit_remember: true,
                },
                true,
            )
            .unwrap();

        let selected = memory
            .select_temporal(2_000, &TemporalRetrievalIntent::LatestEvidence, true)
            .unwrap();
        assert_eq!(
            selected,
            TemporalEvidenceSelection::Selected {
                identities: vec!["experience/pete/episode-a/resistor-change".into()]
            }
        );
        let retained = memory.records(true).unwrap();
        assert_eq!(retained[0].candidate.provenance[0].event_at_millis, 1_000);
        assert_eq!(
            retained[0].candidate.provenance[0].recorded_at_millis,
            1_001
        );
        assert_eq!(
            retained[0].candidate.provenance[0].body_identity,
            body_identity
        );

        memory.set_provider_available(false);
        assert_eq!(
            memory.select_temporal(2_100, &TemporalRetrievalIntent::LatestEvidence, true),
            Err(MemoryQueryRefusal::ProviderUnavailable)
        );
        assert_eq!(body.body_id.as_str(), body_identity);
    }

    #[test]
    fn standing_homeostasis_play_revises_experience_and_presenter_without_action() {
        let workload = reviewed_pete_workload().unwrap();
        let resident = workload
            .resident_forms
            .iter()
            .find(|item| item.role == PeteWorkloadRole::Homeostasis)
            .unwrap();
        let host = proof_host(
            &workload,
            "proof/pete-forebrain",
            "proof/forebrain-boot",
            &[PeteWorkloadRole::Homeostasis],
        );
        let placements =
            default_expanded_placements(&resident.expanded, std::slice::from_ref(&host)).unwrap();
        let plan = plan_expanded_canonical(
            &resident.expanded,
            &[host],
            &placements,
            &["conduit.base/local@1".into()],
        )
        .unwrap();
        assert!(plan.fragments[0].placements[0]
            .configuration
            .iter()
            .any(|entry| entry.key == "policy-revision"
                && entry.value
                    == conduit_core::ConfigurationValue::Text(HOMEOSTASIS_POLICY_REVISION.into())));
        let workset = BodyWorkset::from_forms([resident.form.clone()]).unwrap();
        let body =
            Body::born_with_forms(workset, 21, SignId::from("proof/pete-body-born")).unwrap();
        let (body, wake) = body.wake(1, SignId::from("proof/pete-wake")).unwrap();
        let body_plan = BodyPlan::seal(
            &wake,
            vec![BodyFormPlan {
                form: resident.form.clone(),
                plan: plan.clone(),
            }],
        )
        .unwrap();
        let play = BodyPlayIdentity::bind(&body_plan, 1);

        let revisions = [
            inputs(99, 300, false, 300, false, true),
            inputs(100, 90, true, 850, true, false),
            inputs(101, 90, true, 850, false, false),
        ];
        for (revision, source_inputs) in revisions.iter().enumerate() {
            let reduced_at = at(100 + revision as u64);
            let canonical_state =
                execute_homeostasis(&resident.authoring, &plan, source_inputs, &reduced_at);
            let reduction_sign = SignId::from(format!("proof/homeostasis/reduction/{revision}"));
            let observation = BodySelfObservation {
                state_kind: conduit_core::kind_id(HOMEOSTATIC_STATE_KIND),
                canonical_state,
                observation_sign_id: reduction_sign.clone(),
                observed_at: reduced_at,
                certainty: ExperienceCertainty::Certain,
                source_refs: source_refs(source_inputs),
            };
            let item = body_self_experience(
                format!("self/homeostasis/{revision}"),
                &observation,
                ExperienceTemporalRole::Current,
            )
            .unwrap();
            let mut experience = current_experience(100 + revision as u64);
            experience.try_admit(item).unwrap();
            assert_eq!(experience.items().len(), 1);
            let subject = format!("self/homeostasis/{revision}");
            let mut basis_signs = vec![reduction_sign];
            basis_signs.extend(source_refs(source_inputs).into_iter().filter_map(|source| {
                match source {
                    ExperienceSourceRef::Sign(sign) => Some(sign),
                    _ => None,
                }
            }));
            let presentation = Presentation::new_with_semantics(
                4,
                PresentationBasis {
                    body_id: None,
                    wake_id: None,
                    source_document_id: None,
                    checked_form_id: None,
                    expanded_form_id: None,
                    plan_id: None,
                    active_play_id: None,
                    sign_ids: basis_signs,
                },
                vec![PresentationSubject {
                    identity: subject.clone(),
                    role: PresentationRole::Info,
                    label: "Homeostatic state".into(),
                    accessibility_name: "Homeostatic state".into(),
                }],
                vec![],
                vec![],
                vec![PresentationText {
                    subject: subject.clone(),
                    text: "Pete's current structured self-state.".into(),
                }],
                vec![],
                vec![PresentationDisclosure {
                    subject: subject.clone(),
                    level: PresentationDisclosureLevel::Primary,
                }],
            )
            .unwrap();
            inspect_presented_current_experience(&experience, &presentation, &subject).unwrap();
            let manifestation =
                conduit_presentation::render_linear_presentation(&presentation).unwrap();
            assert!(format!("{manifestation:?}").contains("structured self-state"));
        }
        assert_eq!(body.body_id, body_plan.body_id);
        assert_eq!(play.body_id, body.body_id);
        assert_eq!(play.plan_id, body_plan.plan_id);
    }

    fn at(ticks: u64) -> TemporalInstant {
        TemporalInstant {
            ticks,
            scale: TemporalScale::Milliseconds,
            clock_basis: "proof/pete-homeostasis".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        }
    }

    fn observed<T>(source: &str, value: T, ticks: u64) -> SourceObservation<T> {
        SourceObservation {
            source_identity: source.into(),
            subject_identity: source.into(),
            availability: SourceAvailability::Present,
            value: Some(value),
            observation_sign_id: Some(SignId::from(format!("sign/{source}/{ticks}"))),
            observed_at: Some(at(ticks)),
            freshness_limit_ticks: 5,
            uncertainty_permille: 0,
            calibration_profile_identity: Some(format!("calibration/{source}@1")),
        }
    }

    fn inputs(
        ticks: u64,
        reserve: u16,
        charging: bool,
        pressure: u16,
        inhibited: bool,
        capability: bool,
    ) -> HomeostaticInputs {
        HomeostaticInputs {
            power: observed(
                "create/battery",
                PowerCondition {
                    reserve_permille: reserve,
                    charging,
                },
                ticks,
            ),
            thermal: observed(
                "motherbrain/thermal",
                ThermalCondition {
                    milli_celsius: 60_000,
                },
                ticks,
            ),
            compute_pressure: observed(
                "motherbrain/compute",
                PressureCondition {
                    pressure_permille: pressure,
                },
                ticks,
            ),
            storage_pressure: observed(
                "motherbrain/storage",
                PressureCondition {
                    pressure_permille: 300,
                },
                ticks,
            ),
            motion_safety: observed(
                "brainstem/safety",
                SafetyCondition {
                    motion_available: true,
                    inhibited,
                },
                ticks,
            ),
            important_capability: observed(
                "provider/model",
                CapabilityCondition {
                    available: capability,
                },
                ticks,
            ),
        }
    }

    fn current_experience(ticks: u64) -> CurrentExperience {
        CurrentExperience::new(
            ExperienceLimits {
                maximum_items: 4,
                maximum_current_items: 4,
                maximum_recent_items: 4,
                maximum_stale_items: 4,
                maximum_historical_items: 4,
                maximum_items_per_domain: 4,
                maximum_model_derived_items: 4,
                maximum_selected_memory_items: 4,
                maximum_source_refs: 16,
                maximum_relationships: 4,
                maximum_item_bytes: MAXIMUM_BODY_SELF_STATE_BYTES,
                maximum_encoded_bytes: 16_384,
                maximum_conflict_alternatives: 2,
                maximum_identity_bytes: 128,
            },
            at(ticks),
            ExperienceTemporalPolicy {
                maximum_current_age_ticks: 5,
                maximum_recent_age_ticks: 20,
            },
        )
        .unwrap()
    }

    fn source_refs(inputs: &HomeostaticInputs) -> Vec<ExperienceSourceRef> {
        [
            &inputs.power.source_identity,
            &inputs.thermal.source_identity,
            &inputs.compute_pressure.source_identity,
            &inputs.storage_pressure.source_identity,
            &inputs.motion_safety.source_identity,
            &inputs.important_capability.source_identity,
        ]
        .into_iter()
        .map(|source_id| ExperienceSourceRef::Source {
            source_id: source_id.clone(),
        })
        .chain(
            [
                &inputs.power.observation_sign_id,
                &inputs.thermal.observation_sign_id,
                &inputs.compute_pressure.observation_sign_id,
                &inputs.storage_pressure.observation_sign_id,
                &inputs.motion_safety.observation_sign_id,
                &inputs.important_capability.observation_sign_id,
            ]
            .into_iter()
            .flatten()
            .cloned()
            .map(ExperienceSourceRef::Sign),
        )
        .collect()
    }

    fn execute_homeostasis(
        authored: &ExpandedAuthoringForm,
        plan: &Plan,
        inputs: &HomeostaticInputs,
        reduced_at: &TemporalInstant,
    ) -> Vec<u8> {
        let placement = &plan.fragments[0].placements[0];
        let bind =
            |binding: &conduit_form::AuthoringFrontBinding,
             descriptor: &conduit_core::PortDescriptor| KernelCompositeFrontBinding {
                external_port: descriptor.clone(),
                internal_child: placement.host_id.clone(),
                internal_placement_id: placement.placement_id.clone(),
                internal_port_id: binding.gear_port_id.clone(),
                terminal: CompositeFrontTerminal::Independent,
            };
        let boundary = KernelCompositeBoundary {
            input_fronts: authored
                .input_bindings
                .iter()
                .map(|binding| {
                    bind(
                        binding,
                        authored
                            .front
                            .inputs()
                            .iter()
                            .find(|port| port.port_id == binding.front_port_id)
                            .unwrap(),
                    )
                })
                .collect(),
            output_fronts: authored
                .output_bindings
                .iter()
                .map(|binding| {
                    bind(
                        binding,
                        authored
                            .front
                            .outputs()
                            .iter()
                            .find(|port| port.port_id == binding.front_port_id)
                            .unwrap(),
                    )
                })
                .collect(),
        };
        let definition = KernelCompositeDefinition {
            host_id: HostId::from("proof/homeostasis-composite"),
            boot_id: BootId::from("proof/homeostasis-composite-boot"),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("proof/pete-homeostasis@1"),
            external_capability: CapabilityOffer {
                startup_parameters: vec![],
                shorthand: None,
                capability_id: CapabilityId::from("run"),
                kind_id: conduit_core::kind_id("proof/pete-homeostasis-composite"),
                kind_contract_revision: conduit_core::KindIdentity::from(
                    "proof/pete-homeostasis-composite@1",
                ),
                implementation: ImplementationOffer {
                    execution_profile_id: ExecutionProfileId::from("proof/pete-homeostasis@1"),
                    implementation_id: ImplementationId::from("proof/pete-homeostasis-composite@1"),
                    artifact_id: ArtifactId::from("proof/pete-homeostasis-composite-artifact@1"),
                },
                inputs: authored.front.inputs().to_vec(),
                outputs: authored.front.outputs().to_vec(),
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 8,
                    max_queue_bytes: 65_536,
                },
            },
            internal_plan: plan.clone(),
            boundary,
            failure_translation: FailureReason::CompositeCapabilityFailed,
        };
        let mut adapter = HomeostasisHostAdapter::from_placement(placement).unwrap();
        let mut registry = KernelOperationRegistry::new();
        registry
            .install(HomeostasisKernelFactory::default())
            .unwrap();
        let mut host = KernelCompositeHost::prepare(definition, &registry).unwrap();
        host.start().unwrap();
        let types = conduit_pete::homeostasis_registered_types();
        let payload = |name: &str, encoded: Vec<u8>| ValuePayload {
            value_kind: types
                .iter()
                .find(|(candidate, _)| *candidate == name)
                .unwrap()
                .1
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            encoded,
        };
        for (port, value) in [
            (
                "power",
                payload(
                    "PetePowerObservation",
                    conduit_pete::encode_power_observation(&inputs.power).unwrap(),
                ),
            ),
            (
                "thermal",
                payload(
                    "PeteThermalObservation",
                    conduit_pete::encode_thermal_observation(&inputs.thermal).unwrap(),
                ),
            ),
            (
                "compute-pressure",
                payload(
                    "PeteComputePressureObservation",
                    conduit_pete::encode_pressure_observation(&inputs.compute_pressure, false)
                        .unwrap(),
                ),
            ),
            (
                "storage-pressure",
                payload(
                    "PeteStoragePressureObservation",
                    conduit_pete::encode_pressure_observation(&inputs.storage_pressure, true)
                        .unwrap(),
                ),
            ),
            (
                "motion-safety",
                payload(
                    "PeteMotionSafetyObservation",
                    conduit_pete::encode_safety_observation(&inputs.motion_safety).unwrap(),
                ),
            ),
            (
                "important-capability",
                payload(
                    "PeteImportantCapabilityObservation",
                    conduit_pete::encode_capability_observation(&inputs.important_capability)
                        .unwrap(),
                ),
            ),
            (
                "reduction-at",
                payload(
                    "PeteReductionInstant",
                    conduit_pete::encode_reduction_instant(reduced_at).unwrap(),
                ),
            ),
        ] {
            host.admit_input(&conduit_core::port_id(port), 0, &value)
                .unwrap();
        }
        for _ in 0..128 {
            host.step().unwrap();
            while let Some(request) = host.next_host_request() {
                let input = host.host_request_input(&request).unwrap().to_vec();
                match adapter.handle(request.request.request, &input).unwrap() {
                    Some(output) => host.complete_host_call_bytes(&request, &output).unwrap(),
                    None => host
                        .complete_host_call(
                            &request,
                            conduit_kernel::HostCallOutcome {
                                disposition: conduit_kernel::HostCallDisposition::Completed,
                                output: None,
                                failure: None,
                            },
                        )
                        .unwrap(),
                }
            }
            if let Some((_, output)) = host.output(&conduit_core::port_id("state")).unwrap() {
                return output.encoded;
            }
        }
        panic!("executable homeostasis Gear produced no state")
    }

    fn proof_host(
        workload: &ReviewedPeteWorkload,
        host_id: &str,
        boot_id: &str,
        roles: &[PeteWorkloadRole],
    ) -> HostAdvertisement {
        let mut host = StdHost::new().advertisement().clone();
        host.host_id = HostId::from(host_id);
        host.boot_id = BootId::from(boot_id);
        host.capabilities = workload
            .resident_forms
            .iter()
            .filter(|item| workload.initial.contains(&item.form) && roles.contains(&item.role))
            .flat_map(|item| &item.expanded.gears)
            .enumerate()
            .map(|(index, gear)| CapabilityOffer {
                startup_parameters: gear.startup_parameters.clone(),
                shorthand: gear.shorthand.clone(),
                capability_id: CapabilityId::from(format!("proof/{host_id}/{index}")),
                kind_id: gear.kind_id.clone(),
                kind_contract_revision: gear.kind_contract_revision.clone(),
                inputs: gear.inputs.clone(),
                outputs: gear.outputs.clone(),
                implementation: ImplementationOffer {
                    execution_profile_id: ExecutionProfileId::from("proof/pete-workload@1"),
                    implementation_id: if gear.kind_id.as_str() == "pete/reduce-homeostasis" {
                        ImplementationId::from(HOMEOSTASIS_IMPLEMENTATION)
                    } else {
                        ImplementationId::from(format!("proof/{host_id}/{index}@1"))
                    },
                    artifact_id: ArtifactId::from("proof/pete-workload-fixture@1"),
                },
                host_calls: if gear.kind_id.as_str() == "pete/reduce-homeostasis" {
                    vec![HostCallRequirement {
                        contract_id: HostCallContractId::from(
                            "conduit.pete/homeostasis-reduce-host@1",
                        ),
                        target_kind: None,
                        maximum_in_flight: 1,
                        maximum_input_bytes: 8_192,
                        maximum_output_bytes: 8_192,
                    }]
                } else {
                    Vec::new()
                },
                resource_requirements: Vec::new(),
                authority_requirements: Vec::new(),
                limits: CapabilityLimits {
                    max_active_instances: 8,
                    max_queue_items: 64,
                    max_queue_bytes: 1_048_576,
                },
            })
            .collect();
        host.capabilities
            .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        host
    }

    fn proof_plans(
        workload: &ReviewedPeteWorkload,
        hosts: &[HostAdvertisement],
        use_browser: bool,
    ) -> Vec<BodyFormPlan> {
        workload
            .resident_forms
            .iter()
            .filter(|item| workload.initial.contains(&item.form))
            .map(|item| {
                let selected_host = hosts
                    .iter()
                    .find(|host| host.host_id.as_str() == selected_host_id(item.role, use_browser))
                    .unwrap();
                let placements = default_expanded_placements(
                    &item.expanded,
                    std::slice::from_ref(selected_host),
                )
                .unwrap();
                let plan = plan_expanded_canonical(
                    &item.expanded,
                    std::slice::from_ref(selected_host),
                    &placements,
                    &["conduit.base/local@1".into()],
                )
                .unwrap();
                BodyFormPlan {
                    form: item.form.clone(),
                    plan,
                }
            })
            .collect()
    }

    fn selected_host_id(role: PeteWorkloadRole, use_browser: bool) -> &'static str {
        match role {
            PeteWorkloadRole::Situation => "proof/pete-brainstem",
            PeteWorkloadRole::AutobiographicalMemory => "proof/pete-motherbrain",
            PeteWorkloadRole::HistoricalIndex => "proof/pete-forebrain",
            PeteWorkloadRole::Conversation if use_browser => "proof/pete-optional-browser",
            PeteWorkloadRole::Conversation => "proof/pete-forebrain",
            PeteWorkloadRole::Homeostasis => "proof/pete-forebrain",
            PeteWorkloadRole::Navigation => {
                unreachable!("navigation is not in the initial workset")
            }
        }
    }

    fn hosts_in(plan: &patchbay_model::BodyPlanProjection) -> std::collections::BTreeSet<&str> {
        plan.active_forms
            .iter()
            .flat_map(|form| &form.placements)
            .map(|placement| placement.host_id.as_str())
            .collect()
    }
}

#[cfg(test)]
mod learned_lifecycle;
