use crate::RendererSnapshot;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyGraduationChoice,
    BodyGraduationEvidence, BodyMembership, MembershipProofId, PartId,
};
use conduit_core::{BootId, HostId, ImplementationId, OfferGeneration, PlanId, SignId};
use patchbay_model::{
    CurrentBodyFrame, PatchbayBodyApplicationEntrance, PatchbayBodyAttachment,
    PatchbayNavigationProjection, ReadableBodyHistory, RendererAdapterIdentity,
    RendererAdapterKind, RendererExecution,
};

pub fn demonstration_snapshot() -> Result<RendererSnapshot, String> {
    let (presentation, parts) = patchbay_model::portable_demonstration_with_parts_and_adapter(
        &patchbay_hosted::HostedPatchbayAdapter,
    )?;
    let execution = RendererExecution::prepare(
        presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/host"),
            boot_id: BootId::from("patchbay-html/boot"),
            target_subject: "patchbay-html/document-0".into(),
        },
        SignId::from("patchbay-html/manifestation-prepared"),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    snapshot
        .attach_parts(parts)
        .map_err(|error| error.to_string())?;
    let navigation = PatchbayNavigationProjection::for_embodied(&snapshot.presentation)?;
    snapshot
        .attach_navigation(navigation)
        .map_err(|error| error.to_string())?;
    snapshot
        .attach_workbench(demonstration_workbench(&snapshot)?)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

pub(crate) fn demonstration_workbench(
    snapshot: &RendererSnapshot,
) -> Result<crate::BrowserBodyWorkbench, String> {
    demonstration_workbench_for(
        snapshot,
        BodyGraduationChoice::HostedPatchbay,
        PatchbayBodyApplicationEntrance::Hosted {
            plan_id: PlanId::from("plan/roseau-hosted-patchbay"),
            implementation_id: ImplementationId::from("browser/patchbay-surface@1"),
        },
    )
}

fn demonstration_workbench_for(
    snapshot: &RendererSnapshot,
    graduation_choice: BodyGraduationChoice,
    entrance: PatchbayBodyApplicationEntrance,
) -> Result<crate::BrowserBodyWorkbench, String> {
    const PATCHBAY_PLAN: &str = "plan/roseau-hosted-patchbay";
    const PATCHBAY_IMPLEMENTATION: &str = "browser/patchbay-surface@1";
    let basis = &snapshot.presentation.basis;
    let source = basis
        .source_document_id
        .clone()
        .ok_or("documentary Presentation has no source document")?;
    let checked = basis
        .checked_form_id
        .clone()
        .ok_or("documentary Presentation has no checked Form")?;
    let body = Body::born(source, checked, 1, SignId::from("patchbay/bornd"))
        .map_err(|error| format!("rebuild documentary Body: {error:?}"))?;
    let (body, _) = body
        .wake(1, SignId::from("patchbay/woke"))
        .map_err(|error| format!("rebuild documentary Wake: {error:?}"))?;
    if basis.body_id.as_ref() != Some(&body.body_id) {
        return Err("documentary workbench Body differs from Presentation basis".into());
    }

    let mut membership = BodyMembership::new(body.body_id.clone())
        .map_err(|error| format!("create documentary membership: {error:?}"))?;
    let mut changes = Vec::new();
    for (index, subject, current) in [
        (
            0,
            "here",
            Some((
                HostId::from("patchbay-portable/host"),
                BootId::from("patchbay-portable/boot"),
            )),
        ),
        (
            1,
            "browser-tab-2",
            Some((
                HostId::from("browser/tab-2"),
                BootId::from("browser/tab-2/boot"),
            )),
        ),
        (2, "pico-w", None),
    ] {
        let part = PartId::bind(&body.body_id, subject, index)
            .map_err(|error| format!("bind documentary Part: {error:?}"))?;
        let proof = MembershipProofId::bind(&format!("patchbay/{subject}/admitted"))
            .map_err(|error| format!("bind documentary proof: {error:?}"))?;
        let admitted = membership
            .admit(
                &body.body_id,
                membership.revision,
                part.clone(),
                proof,
                SignId::from(format!("patchbay/{subject}/admitted")),
            )
            .map_err(|error| format!("admit documentary Part: {error:?}"))?;
        changes.push(admitted);
        if let Some((host_id, boot_id)) = current {
            let present = membership
                .observe_present(
                    &body.body_id,
                    membership.revision,
                    &part,
                    AuthenticatedHostObservation {
                        host_id,
                        boot_id,
                        offer_generation: OfferGeneration(1),
                        proof_id: MembershipProofId::bind(&format!("patchbay/{subject}/current"))
                            .map_err(|error| {
                            format!("bind documentary presence: {error:?}")
                        })?,
                        sequence: 1,
                    },
                    SignId::from(format!("patchbay/{subject}/present")),
                )
                .map_err(|error| format!("observe documentary Host: {error:?}"))?;
            changes.push(present);
        }
    }
    let mut evidence = BodyBiographyEvidence::born(
        body,
        BodyMembership::new(membership.body_id.clone())
            .map_err(|error| format!("create initial documentary membership: {error:?}"))?,
        "Roseau".into(),
        "Hello".into(),
    )
    .map_err(|error| format!("create documentary biography: {error:?}"))?;
    let sequenced = changes
        .into_iter()
        .enumerate()
        .map(|(index, change)| (change, index as u64 + 2))
        .collect::<Vec<_>>();
    evidence
        .append_membership_events(membership, &sequenced)
        .map_err(|error| format!("record documentary membership: {error:?}"))?;
    let (patchbay_plan_id, patchbay_implementation_id) = match graduation_choice {
        BodyGraduationChoice::HostedPatchbay => (
            Some(PlanId::from(PATCHBAY_PLAN)),
            Some(ImplementationId::from(PATCHBAY_IMPLEMENTATION)),
        ),
        BodyGraduationChoice::ExternalReader => (None, None),
    };
    evidence
        .graduate(BodyGraduationEvidence {
            body_id: evidence.body_id.clone(),
            sequence: sequenced.len() as u64 + 2,
            sign_id: SignId::from("patchbay/roseau/graduated"),
            choice: graduation_choice,
            patchbay_plan_id,
            patchbay_implementation_id,
        })
        .map_err(|error| format!("graduate documentary Body: {error:?}"))?;
    let attachment = PatchbayBodyAttachment::open_serialized(
        &serde_json::to_vec(&evidence).map_err(|error| error.to_string())?,
        entrance,
    )
    .map_err(|error| format!("open documentary Body: {error:?}"))?;
    let current = CurrentBodyFrame::from_attachment(1, &attachment);
    let history = ReadableBodyHistory::from_attachment(1, &attachment)
        .map_err(|error| format!("project documentary history: {error:?}"))?;
    crate::BrowserBodyWorkbench::from_models(&current, &history)
        .map_err(|error| format!("compose documentary workbench: {error:?}"))
}

pub fn llm_documentary_snapshot() -> Result<RendererSnapshot, String> {
    let presentation = patchbay_model::llm_documentary_presentation_with_adapter(
        &patchbay_hosted::HostedPatchbayAdapter,
    )?;
    let execution = RendererExecution::prepare(
        presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/llm-documentary"),
            boot_id: BootId::from("patchbay-html/llm-documentary/boot"),
            target_subject: "patchbay-html/llm-documentary/document".into(),
        },
        SignId::from("patchbay-html/llm-documentary/prepared"),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    let navigation = PatchbayNavigationProjection::for_embodied(&snapshot.presentation)?;
    snapshot
        .attach_navigation(navigation)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

pub fn llm_embodiment_snapshot(stage: usize) -> Result<RendererSnapshot, String> {
    let presentation = patchbay_model::llm_embodiment_documentary_presentations()
        .map_err(|error| format!("{error:?}"))?
        .into_iter()
        .nth(stage)
        .ok_or("LLM embodiment stage must be 0, 1, or 2")?;
    let execution = RendererExecution::prepare(
        presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/llm-embodiment"),
            boot_id: BootId::from("patchbay-html/llm-embodiment/boot"),
            target_subject: format!("patchbay-html/llm-embodiment/{stage}"),
        },
        SignId::from(format!("patchbay-html/llm-embodiment/{stage}/prepared")),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    let navigation = PatchbayNavigationProjection::for_embodied(&snapshot.presentation)?;
    snapshot
        .attach_navigation(navigation)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

pub fn text_lab_split_snapshot(base: &str) -> Result<RendererSnapshot, String> {
    let explanation = patchbay_model::text_lab_split_explanation(base)?;
    text_lab_snapshot(explanation)
}

pub fn text_lab_split_loss_snapshot(
    base: &str,
    receipt: &conduit_semantic_catalog::TextLabLineLossReceipt,
) -> Result<RendererSnapshot, String> {
    text_lab_snapshot(patchbay_model::text_lab_split_loss_explanation(
        base, receipt,
    )?)
}

fn text_lab_snapshot(
    explanation: patchbay_model::TextLabSplitExplanation,
) -> Result<RendererSnapshot, String> {
    let execution = RendererExecution::prepare(
        explanation.presentation,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/text-lab"),
            boot_id: BootId::from("patchbay-html/text-lab/boot"),
            target_subject: "patchbay-html/text-lab/document".into(),
        },
        SignId::from("patchbay-html/text-lab/prepared"),
    )
    .map_err(|error| error.to_string())?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(|error| error.to_string())?;
    snapshot
        .attach_navigation(explanation.navigation)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[cfg(test)]
mod workbench_tests {
    use super::*;

    #[test]
    fn external_reader_remains_distinct_from_the_hosted_documentary_body() {
        let snapshot = demonstration_snapshot().unwrap();
        let external = demonstration_workbench_for(
            &snapshot,
            BodyGraduationChoice::ExternalReader,
            PatchbayBodyApplicationEntrance::ExternalReader,
        )
        .unwrap();

        assert!(matches!(
            external.current.reader,
            crate::BrowserPatchbayReader::ExternalReadingUnhostedBody
        ));
        assert!(external
            .history
            .entries
            .last()
            .unwrap()
            .narrative
            .contains("No Patchbay was hosted"));
        assert!(external
            .validate_against(Some(&external.current.body_id))
            .is_ok());
    }
}
