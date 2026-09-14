//! Fixed deterministic topology proving Pete workload planning and projection.

#[cfg(test)]
mod tests {
    use conduit_ai::{TemporalEvidenceSelection, TemporalRetrievalIntent};
    use conduit_body::{Body, BodyFormPlan};
    use conduit_core::{
        ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
        HostAdvertisement, HostId, ImplementationId, ImplementationOffer, SignId,
    };
    use conduit_pete::{
        reviewed_pete_workload, BoundedAutobiography, ExperienceCandidate, ExperienceKind,
        ExperienceProvenance, MemoryQueryRefusal, PeteWorkloadRole, ReviewedPeteWorkload,
        Sensitivity,
    };
    use conduit_planner::{default_expanded_placements, plan_expanded_canonical};
    use conduit_std_host::StdHost;
    use patchbay_model::{project_body_plan, BodyPlanningSession, BodyPlanningTransition};

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
        assert_eq!(first.active_forms.len(), 4);
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
        assert_eq!(replacement.active_forms.len(), 4);
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
                    implementation_id: ImplementationId::from(format!("proof/{host_id}/{index}@1")),
                    artifact_id: ArtifactId::from("proof/pete-workload-fixture@1"),
                },
                host_operations: Vec::new(),
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
