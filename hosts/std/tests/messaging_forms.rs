use conduit_chat::{
    delivery_state_type, deterministic_cancel, deterministic_delivery_request,
    deterministic_messaging_fixture, deterministic_provider_acknowledgement, deterministic_submit,
    install_messaging_catalogs, message_attachments_type, message_metadata_type,
    message_recipients_type, messaging_delivery_request_view, notification_event_type,
    presence_event_type, text_messaging_fixture, TextMessagingFixtureSpec,
    MAXIMUM_DELIVERY_ATTEMPTS, MAXIMUM_MESSAGE_ATTACHMENTS, MAXIMUM_MESSAGE_METADATA,
    MAXIMUM_MESSAGE_RECIPIENTS, MESSAGE_ATTACHMENT_ACCESS_CLASS, MESSAGE_ATTACHMENT_PROFILE,
};
use conduit_core::{
    authority_grant, BootId, BoundedResourceRef, ConnectionBase, HostAdvertisement, HostId,
    HostProfileId, OfferGeneration, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape, DEFAULT_CONNECTION_BYTE_CAPACITY, DEFAULT_CONNECTION_ITEM_CAPACITY,
    PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::hosted_messaging::{messaging_std_offers, MESSAGING_HOST_OPERATION};

const SOURCE: &str = include_str!("../../../examples/messaging-delivery.conduit");

#[test]
fn canonical_form_constructs_and_routes_one_structured_message() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_messaging_catalogs(&mut startup, &mut profile).unwrap();
    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "messaging-delivery", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 2);
    assert_eq!(authored.output_bindings.len(), 4);

    let host = host();
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let delivery_offer = host
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_chat::MESSAGING_DELIVERY_KIND)
        .unwrap();
    let grant = authority_grant(
        "grant/messaging-delivery",
        &delivery_offer.authority_requirements[0],
        host.host_id.clone(),
        host.boot_id.clone(),
        delivery_offer.capability_id.clone(),
    );
    let connection_bases = std::collections::BTreeMap::new();
    let line_candidates = std::collections::BTreeMap::new();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &authored.expanded,
        &[host],
        &placements,
        &[ConnectionBase::Local],
        conduit_planner::PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    for placement in &plan.fragments[0].placements {
        assert_eq!(
            placement.host_operations[0].contract_id.as_str(),
            MESSAGING_HOST_OPERATION
        );
        assert!(placement.resources.is_empty());
        if placement.kind_id.as_str() == conduit_chat::MESSAGING_DELIVERY_KIND {
            assert_eq!(placement.authority.len(), 1);
        } else {
            assert!(placement.authority.is_empty());
        }
    }
}

#[test]
fn attachment_is_an_exact_bounded_resource_not_inline_content() {
    let fixture = deterministic_messaging_fixture().unwrap();
    let attachments = collection(record_field(&fixture.message, "attachments"));
    assert_eq!(attachments.len(), usize::from(MAXIMUM_MESSAGE_ATTACHMENTS));
    assert_eq!(variant_tag(&attachments[0]), "attachment");
    assert_eq!(variant_tag(&attachments[1]), "unused");
    let attachment = variant_payload(&attachments[0]);
    let reference =
        BoundedResourceRef::decode(leaf_bytes(record_field(attachment, "content"))).unwrap();
    assert_eq!(
        reference.content_profile.as_str(),
        MESSAGE_ATTACHMENT_PROFILE
    );
    assert_eq!(
        reference.access_class.as_str(),
        MESSAGE_ATTACHMENT_ACCESS_CLASS
    );
    assert_eq!(reference.extent.bytes, 24);

    let rendered = format!("{:?}", fixture.message.value_type()).to_ascii_lowercase();
    assert!(rendered.contains(conduit_core::RESOURCE_REFERENCE_INFO_ID));
    for forbidden in ["base64", "json", "value/bytes", "smtp", "sms", "webhook"] {
        assert!(!rendered.contains(forbidden), "message leaked {forbidden}");
    }
}

#[test]
fn provider_evidence_never_promotes_acknowledgement_to_delivery() {
    let fixture = deterministic_messaging_fixture().unwrap();
    let queued = deterministic_submit(&fixture.request, false).unwrap();
    let queued_state = record_field(&queued.update, "state");
    assert_eq!(variant_tag(queued_state), "queued");
    assert_eq!(variant_tag(variant_payload(queued_state)), "local_queue");

    let sent = deterministic_provider_acknowledgement(&fixture.request).unwrap();
    let sent_state = record_field(&sent.update, "state");
    assert_eq!(variant_tag(sent_state), "sent");
    assert_eq!(
        variant_tag(variant_payload(sent_state)),
        "provider_acknowledgement"
    );
    assert_ne!(variant_tag(sent_state), "delivered");
    assert_eq!(
        leaf_text(record_field(&sent.notification, "source_request_identity")),
        "delivery/fixture/1"
    );
}

#[test]
fn duplicate_cancel_authority_refusal_and_retry_limit_are_explicit() {
    let fixture = deterministic_messaging_fixture().unwrap();
    let duplicate = deterministic_submit(&fixture.request, true).unwrap();
    assert_eq!(
        variant_tag(record_field(&duplicate.update, "state")),
        "duplicate"
    );
    let cancelled = deterministic_cancel(&fixture.request).unwrap();
    assert_eq!(
        variant_tag(record_field(&cancelled.update, "state")),
        "cancelled"
    );

    let no_authority = deterministic_delivery_request(
        &fixture.message,
        1,
        None,
        "correlation/no-authority",
        "delivery/no-authority",
    )
    .unwrap();
    let refused = deterministic_submit(&no_authority, false).unwrap();
    assert_eq!(
        variant_tag(record_field(&refused.update, "state")),
        "refused"
    );

    let exhausted = deterministic_delivery_request(
        &fixture.message,
        MAXIMUM_DELIVERY_ATTEMPTS + 1,
        Some("authority/fixture-recipient/1"),
        "correlation/exhausted",
        "delivery/exhausted",
    )
    .unwrap();
    let failed = deterministic_submit(&exhausted, false).unwrap();
    assert_eq!(variant_tag(record_field(&failed.update, "state")), "failed");
}

#[test]
fn lifecycle_and_separate_notification_presence_families_are_finite() {
    let state = delivery_state_type();
    let StructuredInfoTypeShape::Variant { cases, .. } = state.shape() else {
        panic!("delivery state must be a variant")
    };
    assert_eq!(
        cases.iter().map(|case| case.tag()).collect::<Vec<_>>(),
        [
            "cancelled",
            "delivered",
            "duplicate",
            "expired",
            "failed",
            "queued",
            "refused",
            "sent",
        ]
    );
    assert_ne!(notification_event_type(), presence_event_type());

    for (value_type, exact_length) in [
        (message_recipients_type(), MAXIMUM_MESSAGE_RECIPIENTS),
        (message_metadata_type(), MAXIMUM_MESSAGE_METADATA),
        (message_attachments_type(), MAXIMUM_MESSAGE_ATTACHMENTS),
    ] {
        let StructuredInfoTypeShape::Collection { length, .. } = value_type.shape() else {
            panic!("messaging slots must be fixed collections")
        };
        assert_eq!(length, exact_length);
    }
}

#[test]
fn provider_neutral_text_fixture_exposes_exact_authorized_request_without_attachment() {
    let fixture = text_messaging_fixture(TextMessagingFixtureSpec {
        message_identity: "message/live/1",
        request_identity: "delivery/live/1",
        correlation_identity: "correlation/live/1",
        authority_identity: "authority/live/1",
        recipient_address: "issue/1404",
        recipient_address_profile: "messaging/conduit-issue@1",
        body: "Live provider proof.",
    })
    .unwrap();
    let view = messaging_delivery_request_view(&fixture.request).unwrap();
    assert_eq!(view.request_identity, "delivery/live/1");
    assert_eq!(view.correlation_identity, "correlation/live/1");
    assert_eq!(view.authority_identity.as_deref(), Some("authority/live/1"));
    assert_eq!(view.attempt, 1);
    assert_eq!(view.body, "Live provider proof.");
    assert_eq!(view.recipients.len(), 1);
    assert_eq!(view.recipients[0].address, "issue/1404");
    assert_eq!(
        view.recipients[0].address_profile,
        "messaging/conduit-issue@1"
    );
    assert_eq!(view.attachment_count, 0);
}

fn host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/messaging-proof"),
        boot_id: BootId::from("boot/messaging-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/messaging-proof@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: messaging_std_offers(),
    }
}

fn record_field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("expected record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}

fn collection(value: &StructuredInfoValue) -> &[StructuredInfoValue] {
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        panic!("expected collection")
    };
    values
}

fn variant_tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("expected variant")
    };
    tag
}

fn variant_payload(value: &StructuredInfoValue) -> &StructuredInfoValue {
    let StructuredInfoValueShape::Variant { payload, .. } = value.shape() else {
        panic!("expected variant")
    };
    payload
}

fn leaf_text(value: &StructuredInfoValue) -> &str {
    core::str::from_utf8(leaf_bytes(value)).unwrap()
}

fn leaf_bytes(value: &StructuredInfoValue) -> &[u8] {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("expected leaf")
    };
    bytes
}
