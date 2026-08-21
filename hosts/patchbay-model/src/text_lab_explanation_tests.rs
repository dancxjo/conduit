use crate::{
    text_lab_split_explanation, RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
};
use conduit_core::{BootId, HostId, SignId};
use conduit_presentation::{PresentationDepth, PresentationPlace, PresentationRole};

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
