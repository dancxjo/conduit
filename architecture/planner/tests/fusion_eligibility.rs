//! Planner refusal proof; no fused execution or runtime speed claim.
use conduit_core::*;
use conduit_planner::{
    select_fusion_candidate, CandidatePlacementDisposition, FusionBoundary, FusionCandidate,
    FusionPlanningInputs,
};
mod common;
#[path = "common/fusion_fixture.rs"]
mod fixture;
use fixture::*;

fn rejected(f: &Fixture, offer: conduit_planner::FusionRealizationOffer) -> String {
    rejected_with_boundaries(f, offer, &candidates(f), &[])
}

fn rejected_with_boundaries(
    f: &Fixture,
    offer: conduit_planner::FusionRealizationOffer,
    choices: &[FusionCandidate],
    boundaries: &[FusionBoundary],
) -> String {
    let selection = select_fusion_candidate(
        &f.plot,
        &f.hosts,
        choices,
        &basis(f, 2),
        FusionPlanningInputs {
            semantic_kinds: &f.semantic_kinds,
            offers: &[offer],
            observations: &[fusion_observation(1)],
            boundaries,
            line_offers: std::slice::from_ref(&f.line),
        },
    )
    .expect("ordinary unfused realization remains available");
    assert_ne!(selection.selected_candidate_id, "all-local-fused");
    match &selection.considered[1].disposition {
        CandidatePlacementDisposition::Rejected(reason) => reason.clone(),
        other => panic!("unsafe fusion was not refused: {other:?}"),
    }
}

#[test]
fn independent_semantic_axes_have_specific_planner_refusals() {
    // Adversarial supplied contracts must fail closed even when an offer claims
    // it preserves every boundary. This is not new authored purity authority.
    for (law, expected) in [
        (
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable),
            "ExternalEffect",
        ),
        (
            KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained),
            "TemporalState",
        ),
        (
            KindSemanticLaw::TimeDependence(SemanticDependence::Ambient),
            "AmbientTime",
        ),
        (
            KindSemanticLaw::RandomDependence(SemanticDependence::Ambient),
            "AmbientRandom",
        ),
        (
            KindSemanticLaw::ResourceDependence(SemanticDependence::Ambient),
            "AmbientResource",
        ),
        (
            KindSemanticLaw::Suspension(SuspensionBehavior::MaySuspend),
            "Suspension",
        ),
        (
            KindSemanticLaw::Variability(VariabilityBehavior::AdmittedVariability),
            "Variability",
        ),
        (
            KindSemanticLaw::Replay(ReplayBehavior::Ineligible),
            "ReplayIneligible",
        ),
    ] {
        let mut f = fixture();
        let laws = &mut f.semantic_kinds[0].semantic_laws;
        let original = laws
            .iter_mut()
            .find(|old| std::mem::discriminant(*old) == std::mem::discriminant(&law))
            .unwrap();
        *original = law;
        let offered = offer(&f);
        assert!(
            rejected(&f, offered).contains(expected),
            "missing named {expected} refusal"
        );
    }
}

#[test]
fn each_required_runtime_preservation_and_finite_envelope_fails_closed() {
    for axis in 0..4 {
        let f = fixture();
        let mut offered = offer(&f);
        match axis {
            0 => offered.preserves_typed_ports = false,
            1 => offered.preserves_atomic_pressure = false,
            2 => offered.preserves_cancellation = false,
            _ => offered.preserves_required_evidence = false,
        }
        assert!(rejected(&f, offered).contains("does not preserve required runtime semantics"));
    }
    for axis in 0..3 {
        let mut f = fixture();
        let limits = &mut f.semantic_kinds[0].limits;
        match axis {
            0 => limits.max_active_instances = 0,
            1 => limits.max_queue_items = 0,
            _ => limits.max_queue_bytes = 0,
        }
        let offered = offer(&f);
        assert!(rejected(&f, offered).contains("InvalidFiniteEnvelope"));
    }
}

#[test]
fn resource_and_authority_back_requirements_cannot_be_erased() {
    let f = fixture();
    let mut resource = offer(&f);
    resource.resource_requirements.push(ResourceRequirement {
        class_id: ResourceClassId::from("test/resource"),
        units: 1,
        protected_role: None,
        compute: None,
        content: None,
    });
    assert!(rejected(&f, resource).contains("BackResource"));
    let mut authority = offer(&f);
    authority.authority_requirements.push(AuthorityRequirement {
        contract_id: AuthorityContractId::from("test/authority"),
        host_call_contract_id: HostCallContractId::from("test/host-call"),
        subject_kind: kind_id("test/subject"),
    });
    assert!(rejected(&f, authority).contains("BackAuthority"));
}

#[test]
fn cross_host_and_required_observation_boundaries_have_named_refusals() {
    let f = fixture();
    let mut choices = candidates(&f);
    choices[1].realization = choices[2].realization.clone();
    choices[1].realization.candidate_id = choices[1].candidate_id.clone();
    assert!(rejected_with_boundaries(&f, offer(&f), &choices, &[])
        .contains("not all local on the offered Host"));
    for requires_observation in [true, false] {
        let boundary = FusionBoundary {
            source_gear_id: GearId::from("fusion/transform"),
            sink_gear_id: GearId::from("fusion/analysis"),
            requires_observation,
            requires_authority: !requires_observation,
        };
        assert!(
            rejected_with_boundaries(&f, offer(&f), &candidates(&f), &[boundary])
                .contains("required observation or authority boundary forbids fusion")
        );
    }
}
