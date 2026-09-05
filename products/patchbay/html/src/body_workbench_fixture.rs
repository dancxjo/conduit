//! Deterministic documentary Body used only by browser proof entrances.

use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyGraduationChoice,
    BodyGraduationEvidence, BodyMembership, BodyWorkset, MembershipProofId, PartId, ResidentForm,
};
use conduit_core::{bind_sign, BootId, HostId, ImplementationId, OfferGeneration, PlanId, SignId};
use patchbay_model::FormCandidate;

use crate::{
    body_workbench_snapshot_with_forms, BodyWorkbenchError, BrowserBodyWorkbenchEntrance,
    RendererSnapshot,
};

/// Product entrances use caller-supplied serialized evidence instead.
pub fn body_workbench_fixture_snapshot(
    hosted: bool,
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    let forms = body_workbench_fixture_forms()?;
    body_workbench_fixture_snapshot_with_forms(hosted, &forms)
}

pub fn body_workbench_fixture_forms() -> Result<Vec<FormCandidate>, BodyWorkbenchError> {
    Ok(vec![
        reviewed_form(
            "Hello",
            "forms/hello/main.conduit",
            include_str!("../../../../forms/hello/main.conduit"),
            5,
        )?,
        reviewed_form(
            "Greet",
            "forms/greet/main.conduit",
            include_str!("../../../../forms/greet/main.conduit"),
            6,
        )?,
        reviewed_form(
            "Count",
            "forms/count/main.conduit",
            include_str!("../../../../forms/count/main.conduit"),
            7,
        )?,
        reviewed_form(
            "Desk Telegraph",
            "forms/desk-telegraph/main.conduit",
            include_str!("../../../../forms/desk-telegraph/main.conduit"),
            8,
        )?,
        reviewed_form(
            "Memory Lantern",
            "forms/memory-lantern/main.conduit",
            include_str!("../../../../forms/memory-lantern/main.conduit"),
            9,
        )?,
    ])
}

fn body_workbench_fixture_snapshot_with_forms(
    hosted: bool,
    available: &[FormCandidate],
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    const PLAN: &str = "plan/roseau-hosted-patchbay";
    const IMPLEMENTATION: &str = "browser/patchbay-surface@1";
    let host = HostId::from("host/roseau");
    let boot = BootId::from("boot/roseau/1");
    let body = Body::born_with_forms(
        BodyWorkset::from_forms([
            ResidentForm::new(
                available[3].source_document_id.clone(),
                available[3].checked_form_id.clone(),
            ),
            ResidentForm::new(
                available[4].source_document_id.clone(),
                available[4].checked_form_id.clone(),
            ),
        ])
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?,
        1,
        bind_sign(&host, &boot, None, 1).sign_id,
    )
    .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let mut membership = BodyMembership::new(body.body_id.clone())
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let part = PartId::bind(&body.body_id, "roseau/here", 1)
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let proof = MembershipProofId::bind("proof/roseau/here")
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            bind_sign(&host, &boot, None, 2).sign_id,
        )
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let joined = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            bind_sign(&host, &boot, None, 3).sign_id,
        )
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let mut evidence = BodyBiographyEvidence::born(
        body,
        BodyMembership::new(membership.body_id.clone())
            .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?,
        "Roseau".into(),
    )
    .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    evidence
        .append_membership_events(membership, &[(admitted, 2), (joined, 3)])
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let choice = if hosted {
        BodyGraduationChoice::HostedPatchbay
    } else {
        BodyGraduationChoice::ExternalReader
    };
    evidence
        .graduate(BodyGraduationEvidence {
            body_id: evidence.body_id.clone(),
            sequence: 4,
            sign_id: SignId::from("sign/roseau/graduated"),
            choice,
            patchbay_plan_id: hosted.then(|| PlanId::from(PLAN)),
            patchbay_implementation_id: hosted.then(|| ImplementationId::from(IMPLEMENTATION)),
        })
        .map_err(|error| BodyWorkbenchError::Projection(format!("{error:?}")))?;
    let encoded = serde_json::to_vec(&evidence).map_err(BodyWorkbenchError::Encode)?;
    let entrance = if hosted {
        BrowserBodyWorkbenchEntrance::Hosted {
            plan_id: PLAN.into(),
            implementation_id: IMPLEMENTATION.into(),
        }
    } else {
        BrowserBodyWorkbenchEntrance::ExternalReader
    };
    body_workbench_snapshot_with_forms(1, &encoded, entrance, available)
}

fn reviewed_form(
    label: &str,
    source_name: &str,
    source: &str,
    freshness: u64,
) -> Result<FormCandidate, BodyWorkbenchError> {
    FormCandidate::from_source(
        label,
        source_name,
        source,
        "reviewed canonical fixture Form",
        SignId::from(format!("sign/{}-reviewed", label.to_lowercase())),
        freshness,
    )
    .map_err(BodyWorkbenchError::Projection)
}
