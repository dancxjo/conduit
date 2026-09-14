//! Fixed deterministic topology proving Pete workload planning and projection.

#[cfg(test)]
mod tests {
    use conduit_body::{Body, BodyFormPlan};
    use conduit_core::{
        ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
        HostAdvertisement, HostId, ImplementationId, ImplementationOffer, SignId,
    };
    use conduit_pete::{reviewed_pete_workload, ReviewedPeteWorkload};
    use conduit_planner::{default_expanded_placements, plan_expanded_canonical};
    use conduit_std_host::StdHost;
    use patchbay_model::{project_body_plan, BodyPlanningSession, BodyPlanningTransition};

    #[test]
    fn one_body_plan_projects_four_forms_and_relocates_after_offers_change() {
        let workload = reviewed_pete_workload().unwrap();
        let host_a = proof_host(&workload, "proof/pete-host-a", "proof/boot-a");
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
            proof_plans(&workload, &host_a),
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
        assert!(first
            .active_forms
            .iter()
            .flat_map(|form| &form.placements)
            .all(|placement| placement.host_id.as_str() == "proof/pete-host-a"));

        let host_b = proof_host(&workload, "proof/pete-host-b", "proof/boot-b");
        session
            .replan(
                proof_plans(&workload, &host_b),
                BodyPlanningTransition {
                    unsatisfied_sign_id: Some(SignId::from("proof/pete-host-a-offers-left")),
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
        assert!(replacement
            .active_forms
            .iter()
            .flat_map(|form| &form.placements)
            .all(|placement| placement.host_id.as_str() == "proof/pete-host-b"));
    }

    fn proof_host(
        workload: &ReviewedPeteWorkload,
        host_id: &str,
        boot_id: &str,
    ) -> HostAdvertisement {
        let mut host = StdHost::new().advertisement().clone();
        host.host_id = HostId::from(host_id);
        host.boot_id = BootId::from(boot_id);
        host.capabilities = workload
            .resident_forms
            .iter()
            .filter(|item| workload.initial.contains(&item.form))
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

    fn proof_plans(workload: &ReviewedPeteWorkload, host: &HostAdvertisement) -> Vec<BodyFormPlan> {
        workload
            .resident_forms
            .iter()
            .filter(|item| workload.initial.contains(&item.form))
            .map(|item| {
                let placements =
                    default_expanded_placements(&item.expanded, std::slice::from_ref(host))
                        .unwrap();
                let plan = plan_expanded_canonical(
                    &item.expanded,
                    std::slice::from_ref(host),
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
}
