use crate::{StdHost, StdHostConfig};
use conduit_body::{Body, BodyConversationContext, BodyConversationContextBasis};
use conduit_core::{BootId, CheckedFormId, HostId, OfferGeneration, SignId, SourceDocumentId};

fn context() -> BodyConversationContext {
    let body = Body::born(
        SourceDocumentId::from("source/body-chat"),
        CheckedFormId::from("checked/body-chat"),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let (_, wake) = body.wake(1, SignId::from("sign/wake")).unwrap();
    BodyConversationContext {
        schema: "conduit.body/conversation-context-value@2".into(),
        display_name: "Roseau".into(),
        body_id: wake.body_id.clone(),
        wake_id: wake.wake_id.clone(),
        wake_sequence: 1,
        basis: BodyConversationContextBasis {
            body_id: wake.body_id.clone(),
            wake_id: wake.wake_id.clone(),
            wake_sequence: 1,
            revision: 0,
        },
        hosts: vec![],
        active_forms: vec!["Tour".into()],
        current_plan_id: None,
        active_play_id: None,
        lines: vec![],
        recent_sign_ids: vec![],
    }
}

#[test]
fn generic_host_cannot_offer_body_truth_until_supervisor_installs_canonical_context() {
    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("host/body"),
        boot_id: BootId::from("boot/body"),
        offer_generation: OfferGeneration(7),
    });
    assert!(!host
        .advertisement()
        .capabilities
        .iter()
        .any(|offer| offer.kind_id.as_str() == conduit_chat::BODY_CONVERSATION_CONTEXT_KIND));
    host.install_body_conversation_context(&context()).unwrap();
    assert_eq!(host.advertisement().offer_generation, OfferGeneration(8));
    assert!(host
        .advertisement()
        .capabilities
        .iter()
        .any(|offer| offer.kind_id.as_str() == conduit_chat::BODY_CONVERSATION_CONTEXT_KIND));
    host.install_body_conversation_context(&context()).unwrap();
    assert_eq!(host.advertisement().offer_generation, OfferGeneration(8));
}

#[test]
fn installed_supervisor_source_accepts_only_fresh_current_truth() {
    let mut host = StdHost::new();
    let initial = context();
    host.install_body_conversation_context(&initial).unwrap();
    let source = host.body_conversation_context_source().unwrap();
    let mut replacement = initial.clone();
    replacement.basis.revision = 1;
    replacement.active_forms.push("form/next".into());
    assert_eq!(
        source.replace(&replacement),
        Ok(crate::BodyConversationContextReplacement::Published)
    );
    assert_eq!(
        source.replace(&replacement),
        Ok(crate::BodyConversationContextReplacement::Coalesced)
    );
    assert_eq!(
        source.replace(&initial),
        Err(crate::BodyConversationContextUpdateRefusal::StaleBasis)
    );
}
