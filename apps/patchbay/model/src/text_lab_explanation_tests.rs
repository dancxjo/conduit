use crate::{
    text_lab_split_explanation, RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
};
use conduit_core::{BootId, HostId, SignId};
use conduit_presentation::{PresentationDepth, PresentationPlace, PresentationRole};
use conduit_std_catalog::TextLabLineLossReceipt;
use conduit_std_catalog::{
    exact_text_lab_line_loss_outcome, exact_text_lab_split_plan, TEXT_LAB_RETURN_LINE,
};

fn loss_receipt(base: &str) -> TextLabLineLossReceipt {
    let exact = exact_text_lab_split_plan(base).unwrap();
    let outcome = exact_text_lab_line_loss_outcome(base, TEXT_LAB_RETURN_LINE).unwrap();
    let active = conduit_core::bind_active_play(
        &exact.plan.plan_id,
        &exact.native.host_id,
        &exact.native.boot_id,
        0,
    );
    let sign = conduit_core::bind_sign(
        &exact.native.host_id,
        &exact.native.boot_id,
        Some(&active.active_play_id),
        2,
    );
    TextLabLineLossReceipt {
        schema: "conduit.text-lab/line-loss@1".into(),
        code: "CND-TEXT-LIVE-301".into(),
        phase: "return-offer".into(),
        sequence: 2,
        line_id: TEXT_LAB_RETURN_LINE.into(),
        plan_id: exact.plan.plan_id.as_str().into(),
        source_document_id: exact.plan.source_document_id.as_str().into(),
        checked_form_id: exact.plan.checked_form_id.as_str().into(),
        active_play_id: active.active_play_id.as_str().into(),
        sign_id: sign.sign_id.as_str().into(),
        old_plan_disposition: "immutable".into(),
        fresh_planning: "unrealizable".into(),
        form_unchanged: true,
        refusal: outcome.refusal,
        transport_failure: "Transport(Disconnected)".into(),
    }
}

#[test]
fn unchanged_text_lab_explains_split_program_and_body_without_mixing_domains() {
    let explanation = text_lab_split_explanation("ws://127.0.0.1:1/conduit").unwrap();
    assert_eq!(
        explanation.ordinary_path,
        "keyboard here -> uppercase there -> presentation here"
    );
    assert_eq!(
        explanation.upper_program_cursor.place,
        PresentationPlace::Program
    );
    assert_eq!(
        explanation.browser_host_cursor.place,
        PresentationPlace::Body
    );
    assert_eq!(
        explanation.browser_host_cursor.depth,
        PresentationDepth::Exact
    );
    assert_eq!(
        explanation.return_line_cursor.place,
        PresentationPlace::Body
    );
    assert_eq!(
        explanation.return_line_cursor.depth,
        PresentationDepth::Exact
    );
    assert_eq!(
        explanation.returned_program_cursor.place,
        PresentationPlace::Program
    );
    assert_eq!(
        explanation.presentation.basis.source_document_id,
        Some(
            conduit_std_catalog::exact_text_lab_split_plan("ws://127.0.0.1:1/conduit")
                .unwrap()
                .plan
                .source_document_id
        )
    );
    let program = explanation
        .navigation
        .navigation
        .places
        .iter()
        .find(|place| place.place == PresentationPlace::Program)
        .unwrap();
    assert!(program.aspects.iter().all(|aspect| {
        aspect.focusable_subjects.iter().all(|id| {
            explanation
                .presentation
                .subjects
                .iter()
                .find(|subject| subject.identity == *id)
                .is_none_or(|subject| {
                    !matches!(
                        subject.role,
                        PresentationRole::Host | PresentationRole::Line
                    )
                })
        })
    }));
}

#[test]
fn native_and_html_adapters_consume_the_same_text_lab_presentation_vocabulary() {
    let explanation = text_lab_split_explanation("ws://127.0.0.1:1/conduit").unwrap();
    let render = |adapter, name: &str| {
        RendererExecution::prepare(
            explanation.presentation.clone(),
            adapter,
            RendererAdapterIdentity {
                host_id: HostId::from(format!("patchbay/{name}")),
                boot_id: BootId::from(format!("patchbay/{name}/boot")),
                target_subject: format!("patchbay/{name}/document"),
            },
            SignId::from(format!("patchbay/{name}/prepared")),
        )
        .unwrap()
    };
    let native = render(RendererAdapterKind::NativeWayland, "native-text-lab");
    let html = render(RendererAdapterKind::HtmlDomSvg, "html-text-lab");
    assert_eq!(native.presentation, html.presentation);
    assert_eq!(
        native.presentation.identity,
        explanation.presentation.identity
    );
    assert_eq!(
        native.presentation.actions,
        explanation.presentation.actions
    );
    assert!(native
        .presentation
        .text
        .iter()
        .any(|line| { line.text == "keyboard here -> uppercase there -> presentation here" }));
}

#[test]
fn native_loss_receipt_rebuilds_signed_unavailable_truth_and_mutation_refuses() {
    let base = "ws://127.0.0.1:1/conduit";
    let receipt = loss_receipt(base);
    let explanation = crate::text_lab_split_loss_explanation(base, &receipt).unwrap();
    assert_eq!(explanation.presentation.revision, 2);
    assert_eq!(
        explanation.ordinary_path,
        "browser Part unavailable -> unchanged Form currently unrealizable"
    );
    assert!(explanation
        .presentation
        .basis
        .sign_ids
        .iter()
        .any(|sign| sign.as_str() == receipt.sign_id));
    assert!(explanation.presentation.text.iter().any(|line| {
        line.text.contains("fresh-planning=unrealizable")
            && line.text.contains("old-plan=immutable")
    }));
    let mut mutated = receipt;
    mutated.plan_id.push_str("-forged");
    assert!(crate::text_lab_split_loss_explanation(base, &mutated).is_err());
}
