//! Actual canonical Source checking feeds conditional, non-mutating proposals.
use conduit_body::{Body, BodyWorkset, BodyWorksetError, ResidentPlot};
use conduit_body_make::*;
use conduit_core::*;
use conduit_plot::*;

fn report(gear: &str) -> SourceCheckInfoV1 {
    let mut profiles = ProfileCatalog::new();
    profiles
        .insert_kind(Kind {
            kind_id: "fixture/unit".into(),
            kind_contract_revision: "unit@1".into(),
            startup_parameters: vec![],
            shorthand: None,
            configuration: vec![],
            inputs: vec![],
            outputs: vec![],
            semantic_laws: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 64,
            },
        })
        .unwrap();
    let context = SourceCheckContext::new(
        CheckContextRevision {
            grammar_edition: "conduitese-v1".into(),
            catalog: "fixture/catalog@1".into(),
            kinds_fores: "fixture/unit@1".into(),
            types_laws: "empty@1".into(),
            backs: "empty@1".into(),
        },
        profiles.startup_catalog().unwrap(),
        profiles,
        CanonicalBackCatalog::new(),
    )
    .unwrap();
    let mut owner = SourceCheckProjection::new(
        "fixture/source-owner",
        7,
        &format!("plot main {{\n {gear}: fixture/unit\n}}\n"),
        context,
    )
    .unwrap();
    let request = SourceCheckRequest {
        snapshot: owner.snapshot().clone(),
        context: owner.context().clone(),
        entry: "main".into(),
        limits: SourceCheckLimits::default(),
    };
    owner.project(&request).unwrap()
}
fn resident(report: &SourceCheckInfoV1) -> ResidentPlot {
    assert_eq!(report.check, CheckOutcome::Accepted, "{report:?}");
    ResidentPlot::new(
        report.snapshot.source_document_id.as_str().into(),
        report.candidate.checked_plot_id.as_deref().unwrap().into(),
    )
}
fn body(plot: &ResidentPlot, sequence: u64) -> Body {
    Body::born(
        plot.source_document_id.clone(),
        plot.checked_plot_id.clone(),
        sequence,
        "sign/birth".into(),
    )
    .unwrap()
}
fn candidate(report: &SourceCheckInfoV1) -> CheckedWorksetCandidate<'_> {
    CheckedWorksetCandidate {
        snapshot: &report.snapshot,
        context: &report.context,
        report,
    }
}
#[test]
fn actual_checked_candidate_proposes_addition_then_removal_without_owner_effects() {
    let a = resident(&report("first"));
    let checked = report("second");
    let b = resident(&checked);
    let observed = body(&a, 1);
    let before = observed.clone();
    let proposal = propose_checked_workset(
        &observed,
        &observed.body_id,
        0,
        vec![b.clone(), a.clone()],
        Some(candidate(&checked)),
    )
    .unwrap();
    assert_eq!(
        proposal.proposed_workset,
        BodyWorkset::from_plots([a.clone(), b.clone()]).unwrap()
    );
    assert_eq!(proposal.expected_body_id, observed.body_id);
    assert_eq!(proposal.expected_workload_revision, 0);
    assert_eq!(
        proposal.candidate_source.as_ref().unwrap().snapshot,
        checked.snapshot
    );
    assert_eq!(observed, before);
    // Separate fixture transition represents a later observed workset, not adoption by the proposal.
    let later = observed
        .admit_plot(b, "sign/later-admission".into())
        .unwrap();
    let prior = later.clone();
    let removal = propose_checked_workset(
        &later,
        &later.body_id,
        later.workload_revision,
        vec![a],
        None,
    )
    .unwrap();
    assert_eq!(removal.proposed_workset.len(), 1);
    assert!(removal.candidate_source.is_none());
    assert_eq!(later, prior);
}
#[test]
fn wrong_body_revision_source_context_and_source_checked_pair_refuse() {
    use WorksetProposalRefusal as E;
    let checked = report("second");
    let a = resident(&report("first"));
    let b = resident(&checked);
    let observed = body(&a, 1);
    let wrong_body = body(&a, 2);
    assert_eq!(
        propose_checked_workset(
            &observed,
            &wrong_body.body_id,
            0,
            vec![b.clone()],
            Some(candidate(&checked))
        ),
        Err(E::WrongBody)
    );
    assert_eq!(
        propose_checked_workset(
            &observed,
            &observed.body_id,
            1,
            vec![b.clone()],
            Some(candidate(&checked))
        ),
        Err(E::StaleWorkset)
    );
    let mut stale = checked.snapshot.clone();
    stale.generation += 1;
    assert_eq!(
        propose_checked_workset(
            &observed,
            &observed.body_id,
            0,
            vec![b.clone()],
            Some(CheckedWorksetCandidate {
                snapshot: &stale,
                ..candidate(&checked)
            })
        ),
        Err(E::StaleSource)
    );
    let mut context = checked.context.clone();
    context.catalog = "fixture/new-catalog@2".into();
    assert_eq!(
        propose_checked_workset(
            &observed,
            &observed.body_id,
            0,
            vec![b.clone()],
            Some(CheckedWorksetCandidate {
                context: &context,
                ..candidate(&checked)
            })
        ),
        Err(E::StaleContext)
    );
    let mismatch = ResidentPlot::new(a.source_document_id, b.checked_plot_id.clone());
    assert_eq!(
        propose_checked_workset(
            &observed,
            &observed.body_id,
            0,
            vec![mismatch],
            Some(candidate(&checked))
        ),
        Err(E::UncheckedPlot)
    );
    let mut rejected = checked.clone();
    rejected.check = CheckOutcome::Rejected;
    assert_eq!(
        propose_checked_workset(
            &observed,
            &observed.body_id,
            0,
            vec![b],
            Some(candidate(&rejected))
        ),
        Err(E::InvalidCheckInfo)
    );
}
#[test]
fn canonical_workset_limits_and_unknown_candidates_remain_exact() {
    use WorksetProposalRefusal as E;
    let a = resident(&report("first"));
    let observed = body(&a, 1);
    assert_eq!(
        propose_checked_workset(
            &observed,
            &observed.body_id,
            0,
            vec![a.clone(), a.clone()],
            None
        ),
        Err(E::Workset(BodyWorksetError::DuplicatePlot))
    );
    assert_eq!(
        propose_checked_workset(&observed, &observed.body_id, 0, vec![a.clone()], None),
        Err(E::NoChange)
    );
    let unknown = resident(&report("second"));
    assert_eq!(
        propose_checked_workset(&observed, &observed.body_id, 0, vec![unknown], None),
        Err(E::UncheckedPlot)
    );
    let malformed = ResidentPlot::new("".into(), "checked/invalid".into());
    assert_eq!(
        propose_checked_workset(&observed, &observed.body_id, 0, vec![malformed], None),
        Err(E::Workset(BodyWorksetError::InvalidPlotIdentity))
    );
    let too_many_bytes = (0..16)
        .map(|i| {
            ResidentPlot::new(
                format!("source/{i}/{}", "x".repeat(128)).into(),
                format!("checked/{i}/{}", "y".repeat(128)).into(),
            )
        })
        .collect();
    assert_eq!(
        propose_checked_workset(&observed, &observed.body_id, 0, too_many_bytes, None),
        Err(E::Workset(BodyWorksetError::IdentityBytesExhausted))
    );
    let plots: Vec<_> = (0..17)
        .map(|i| ResidentPlot::new(format!("source/{i}").into(), format!("checked/{i}").into()))
        .collect();
    assert_eq!(
        propose_checked_workset(&observed, &observed.body_id, 0, plots.clone(), None),
        Err(E::Workset(BodyWorkset::from_plots(plots).unwrap_err()))
    );
}

#[path = "workset_proposal/authored.rs"]
mod authored;

#[test]
fn malformed_descriptive_check_info_never_forms_a_checked_candidate() {
    let original = report("second");
    let a = resident(&report("first"));
    let b = resident(&original);
    let observed = body(&a, 1);
    for variation in 0..6 {
        let mut invalid = original.clone();
        match variation {
            0 => invalid.schema_version = 2,
            1 => invalid.snapshot.generation = 0,
            2 => invalid.snapshot.sha256 = "not-a-digest".into(),
            3 => invalid.candidate.checked_plot_id = None,
            4 => invalid.requested_entry.clear(),
            _ => invalid.context.grammar_edition = "unknown-grammar".into(),
        }
        assert_eq!(
            propose_checked_workset(
                &observed,
                &observed.body_id,
                0,
                vec![b.clone()],
                Some(candidate(&invalid))
            ),
            Err(WorksetProposalRefusal::InvalidCheckInfo)
        );
    }
}
