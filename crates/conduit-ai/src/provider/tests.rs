extern crate std;

use std::collections::BTreeMap;

use conduit_core::{
    bind_active_play, kind_id, resource_offer, seal_plan_with_realization_backs,
    AuthorityContractId, AuthorityGrant, AuthorityGrantId, BootId, ConnectionBase, GearId,
    HostAdvertisement, HostId, HostOperationContractId, HostProfileId, LineId, LinkBindingId,
    LinkEndpointId, OfferGeneration, ProtectedResourceAccess, ProtectedResourceCommitPolicy,
    ProtectedResourceGrant, ResourceBindingRoleId, ResourceClassId, ResourceHandleId, SignId,
    PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, expand_canonical_form_with_backs,
    parse_syntax_document, CanonicalBackCatalog, ProfileCatalog, StartupCatalog,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical, plan_expanded_canonical_with_options,
    PlacementChoice, PlacementChoices, PlanningOptions,
};

use super::*;
use crate::{
    classify_missing_llm_plan, generate_text_base_fixtures, generate_text_contract,
    install_generate_text_catalog, CrossHostLlmError, CrossHostLlmRun, LlmInterruptionReason,
    LlmPlanningRefusal, ReplacementLlmRun,
};

fn catalogs() -> (StartupCatalog, ProfileCatalog, CanonicalBackCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_generate_text_catalog(&mut startup, &mut profile).unwrap();
    install_provider_catalogs(&mut startup, &mut profile).unwrap();
    let mut backs = CanonicalBackCatalog::new();
    install_provider_back(&startup, &profile, &mut backs).unwrap();
    (startup, profile, backs)
}

fn checked(startup: &StartupCatalog) -> conduit_form::CheckedSyntaxDocument {
    check_syntax_document(
        &parse_syntax_document("form answer {\n generate: ai/generate-text\n}\n"),
        startup,
    )
    .unwrap()
}

fn line(
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
    suffix: &str,
) -> conduit_core::LineOffer {
    let mut line = conduit_signal_conformance::distributed_websocket_line_offer();
    line.line_id = LineId::from(format!("provider/{suffix}"));
    line.binding.binding_id = LinkBindingId::from(format!("provider/{suffix}/binding"));
    line.binding.source.host_id = source.host_id.clone();
    line.binding.source.boot_id = source.boot_id.clone();
    line.binding.source.endpoint_id = LinkEndpointId::from(format!("{suffix}/out"));
    line.binding.sink.host_id = sink.host_id.clone();
    line.binding.sink.boot_id = sink.boot_id.clone();
    line.binding.sink.endpoint_id = LinkEndpointId::from(format!("{suffix}/in"));
    line.binding.limits.maximum_in_flight_items = 1;
    line.binding.limits.maximum_payload_bytes = 4_096;
    line.binding.limits.maximum_buffered_bytes = 4_096;
    line.binding.limits.maximum_frame_bytes = 4_096;
    line.availability.line_id = line.line_id.clone();
    line.availability.binding_id = line.binding.binding_id.clone();
    line.availability.sign_id = SignId::from(format!("provider/{suffix}/ready"));
    line
}

#[test]
fn portable_face_and_provider_protocol_keep_realization_and_failures_distinct() {
    let contract = generate_text_contract();
    let encoded = serde_json::to_string(&contract).unwrap();
    for forbidden in ["http", "credential", "openai", "socket", "address"] {
        assert!(!encoded.to_ascii_lowercase().contains(forbidden));
    }

    let request_value = provider_request("hello").unwrap();
    let request_json = request_value.encode_text().unwrap();
    let request =
        provider_http_request(7, "fixture.invalid", "/v1/responses", &request_json).unwrap();
    assert_eq!(request.headers.len(), 1);
    assert!(request
        .headers
        .iter()
        .all(|header| header.name != "authorization"));

    let response = conduit_web::HttpResponse {
        transaction_id: conduit_web::HttpTransactionId(7),
        status: 200,
        headers: vec![],
        body: conduit_web::HttpBody::inline(br#"{"output":"world"}"#.to_vec()),
    };
    let decoded =
        conduit_core::JsonValue::decode_text(provider_http_response(&response).unwrap()).unwrap();
    assert_eq!(provider_result(&decoded).unwrap(), "world");

    let mut limited = response.clone();
    limited.status = 429;
    assert_eq!(
        provider_http_response(&limited),
        Err(ProviderFailure::ProviderCapacity)
    );
    limited.status = 503;
    assert_eq!(
        provider_http_response(&limited),
        Err(ProviderFailure::HttpStatus)
    );
    assert_ne!(
        ProviderFailure::HttpTransport,
        ProviderFailure::CredentialRefused
    );
    assert_ne!(ProviderFailure::Pressure, ProviderFailure::Cancelled);
    assert_ne!(ProviderFailure::Cancelled, ProviderFailure::PartOrLineLost);
    let evidence = ProviderEvidence::redacted(7, Err(ProviderFailure::CredentialRefused));
    assert!(evidence.credential_present);
    assert_eq!(evidence.credential_value, None);
}

#[test]
fn unchanged_form_selects_direct_face_or_distributed_provider_back_exactly() {
    let (startup, profile, backs) = catalogs();
    let checked = checked(&startup);
    let direct = expand_canonical_form(&checked, "answer", &profile).unwrap();
    let recursive = expand_canonical_form_with_backs(&checked, "answer", &profile, &backs).unwrap();
    assert_eq!(direct.source_document_id, recursive.source_document_id);
    assert_eq!(direct.checked_form_id, recursive.checked_form_id);
    assert_ne!(direct.expanded_form_id, recursive.expanded_form_id);
    assert_eq!(direct.gears.len(), 1);
    assert_eq!(recursive.gears.len(), 7);
    assert_eq!(recursive.realization_backs.len(), 1);

    let direct_host = generate_text_base_fixtures()[0].advertisement.clone();
    let direct_placements =
        default_expanded_placements(&direct, std::slice::from_ref(&direct_host)).unwrap();
    let direct_plan = plan_expanded_canonical(
        &direct,
        std::slice::from_ref(&direct_host),
        &direct_placements,
        &[ConnectionBase::Local],
    )
    .unwrap();

    let offers = provider_offers();
    let tiny = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("tiny-part"),
        boot_id: BootId::from("tiny-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("fixture/tiny"),
        resources: vec![],
        capabilities: offers
            .iter()
            .filter(|offer| {
                matches!(
                    offer.kind_id.as_str(),
                    PROVIDER_REQUEST_KIND | PROVIDER_RESULT_KIND
                )
            })
            .cloned()
            .collect(),
        planner_capabilities: vec![],
    };
    let mut provider_capabilities = offers
        .into_iter()
        .filter(|offer| {
            !matches!(
                offer.kind_id.as_str(),
                PROVIDER_REQUEST_KIND | PROVIDER_RESULT_KIND
            )
        })
        .collect::<Vec<_>>();
    provider_capabilities.push(conduit_std_catalog::json_encode_std_offer());
    provider_capabilities.push(conduit_std_catalog::json_decode_std_offer());
    provider_capabilities.push(provider_http_offer());
    let provider = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("provider-part"),
        boot_id: BootId::from("provider-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("fixture/provider"),
        resources: vec![
            resource_offer("provider/credential", PROVIDER_CREDENTIAL_CLASS, 1),
            resource_offer("provider/http", PROVIDER_HTTP_RESOURCE, 1),
        ],
        capabilities: provider_capabilities,
        planner_capabilities: vec![],
    };
    assert!(tiny.capabilities.iter().all(|offer| {
        offer.kind_id.as_str() != conduit_web::HTTP_CLIENT_KIND
            && offer.kind_id.as_str() != GENERATE_TEXT_KIND
    }));

    let hosts = [tiny.clone(), provider.clone()];
    let placements = PlacementChoices {
        by_gear: recursive
            .gears
            .iter()
            .map(|gear| {
                let host = if matches!(
                    gear.kind_id.as_str(),
                    PROVIDER_REQUEST_KIND | PROVIDER_RESULT_KIND
                ) {
                    &tiny
                } else {
                    &provider
                };
                let capability = host
                    .capabilities
                    .iter()
                    .find(|offer| offer.checked_face() == gear.checked_face())
                    .unwrap();
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: capability.capability_id.clone(),
                    },
                )
            })
            .collect(),
    };
    let http_gear = recursive
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_web::HTTP_CLIENT_KIND)
        .unwrap();
    let http_capability = provider
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_web::HTTP_CLIENT_KIND)
        .unwrap();
    let authority = AuthorityGrant {
        grant_id: AuthorityGrantId::from("grant/provider-endpoint"),
        contract_id: AuthorityContractId::from(PROVIDER_ENDPOINT_AUTHORITY),
        host_operation_contract_id: HostOperationContractId::from(PROVIDER_HTTP_OPERATION),
        subject_kind: kind_id(conduit_web::HTTP_CLIENT_KIND),
        host_id: provider.host_id.clone(),
        boot_id: provider.boot_id.clone(),
        capability_id: http_capability.capability_id.clone(),
    };
    let credential = ProtectedResourceGrant {
        role_id: ResourceBindingRoleId::from(PROVIDER_CREDENTIAL_ROLE),
        handle_id: ResourceHandleId::from("opaque/provider-credential"),
        gear_id: GearId::from(http_gear.gear_id.as_str()),
        host_id: provider.host_id.clone(),
        boot_id: provider.boot_id.clone(),
        capability_id: http_capability.capability_id.clone(),
        class_id: ResourceClassId::from(PROVIDER_CREDENTIAL_CLASS),
        access: ProtectedResourceAccess::ReadExisting,
        maximum_bytes: 256,
        commit_policy: ProtectedResourceCommitPolicy::NotApplicable,
    };
    let lines = [
        line(&tiny, &provider, "tiny-to-provider"),
        line(&provider, &tiny, "provider-to-tiny"),
    ];
    assert!(plan_expanded_canonical_with_options(
        &recursive,
        &hosts,
        &placements,
        &[ConnectionBase::Local, ConnectionBase::WebSocket],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4_096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &lines,
        },
    )
    .is_err());
    let options = PlanningOptions {
        connection_bases: &BTreeMap::new(),
        line_candidates: &BTreeMap::new(),
        connection_item_capacity: 1,
        connection_byte_capacity: 4_096,
        authority_grants: std::slice::from_ref(&authority),
        protected_resource_grants: std::slice::from_ref(&credential),
        line_offers: &lines,
    };
    let recursive_plan = plan_expanded_canonical_with_options(
        &recursive,
        &hosts,
        &placements,
        &[ConnectionBase::Local, ConnectionBase::WebSocket],
        options,
    )
    .unwrap();
    assert_ne!(direct_plan.plan_id, recursive_plan.plan_id);
    assert_eq!(recursive_plan.fragments.len(), 2);
    assert!(recursive_plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .any(|connection| connection.selected_line.is_some()));
    let binding = recursive_plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.gear_id == http_gear.gear_id)
        .unwrap();
    assert_eq!(binding.authority.len(), 1);
    let protected = binding
        .resources
        .iter()
        .find_map(|resource| resource.protected.as_ref())
        .unwrap();
    assert_eq!(protected.handle_id.as_str(), "opaque/provider-credential");
    assert!(conduit_core::verify_plan(&recursive_plan));

    // Loss never patches Plan A. Fresh advertisement truth seals a distinct Plan B.
    let plan_a_snapshot = recursive_plan.clone();
    let play_a = bind_active_play(&recursive_plan.plan_id, &tiny.host_id, &tiny.boot_id, 1);
    let run_a = CrossHostLlmRun::observe(&recursive_plan, &play_a, "request/a".into()).unwrap();
    assert!(run_a.parts.iter().any(|part| part.host_id == tiny.host_id));
    assert!(run_a
        .parts
        .iter()
        .any(|part| part.host_id == provider.host_id));
    let interrupted = run_a.interrupted(LlmInterruptionReason::ModelProviderLost);

    assert_eq!(
        classify_missing_llm_plan::<conduit_core::Plan>(None),
        Err(LlmPlanningRefusal::MissingLlmRealization)
    );

    let fresh_host = HostId::from("replacement-provider-part");
    let fresh_boot = BootId::from("replacement-provider-boot");
    let mut fragments = recursive_plan.fragments.clone();
    for fragment in &mut fragments {
        if fragment.host_id == provider.host_id {
            fragment.host_id = fresh_host.clone();
            fragment.boot_id = fresh_boot.clone();
            fragment.offer_generation = OfferGeneration(2);
        }
        for placement in &mut fragment.placements {
            if placement.host_id == provider.host_id {
                placement.host_id = fresh_host.clone();
                placement.boot_id = fresh_boot.clone();
                placement.offer_generation = OfferGeneration(2);
                for authority in &mut placement.authority {
                    authority.host_id = fresh_host.clone();
                    authority.boot_id = fresh_boot.clone();
                }
            }
        }
        for connection in &mut fragment.connections {
            for admitted in connection
                .admitted_lines
                .iter_mut()
                .chain(connection.selected_line.iter_mut())
            {
                if admitted.binding.source.host_id == provider.host_id {
                    admitted.binding.source.host_id = fresh_host.clone();
                    admitted.binding.source.boot_id = fresh_boot.clone();
                }
                if admitted.binding.sink.host_id == provider.host_id {
                    admitted.binding.sink.host_id = fresh_host.clone();
                    admitted.binding.sink.boot_id = fresh_boot.clone();
                }
            }
        }
    }
    let plan_b = seal_plan_with_realization_backs(
        conduit_core::FormIdentity {
            source_document_id: recursive_plan.source_document_id.clone(),
            checked_form_id: recursive_plan.checked_form_id.clone(),
            expanded_form_id: recursive_plan.expanded_form_id.clone(),
        },
        recursive_plan.realization_backs.clone(),
        fragments,
    );
    let play_b = bind_active_play(&plan_b.plan_id, &tiny.host_id, &tiny.boot_id, 2);
    let run_b = CrossHostLlmRun::observe(&plan_b, &play_b, "request/b".into()).unwrap();
    let replacement = ReplacementLlmRun::start(interrupted, run_b).unwrap();
    assert_eq!(recursive_plan, plan_a_snapshot);
    assert_ne!(recursive_plan.plan_id, plan_b.plan_id);
    assert_eq!(recursive_plan.source_document_id, plan_b.source_document_id);
    assert_eq!(recursive_plan.checked_form_id, plan_b.checked_form_id);
    assert_eq!(recursive_plan.expanded_form_id, plan_b.expanded_form_id);
    assert_eq!(
        replacement.accept_completion(&recursive_plan.plan_id, &play_a.active_play_id, "request/a"),
        Err(CrossHostLlmError::StaleCompletion)
    );
    assert_eq!(
        replacement.accept_completion(&plan_b.plan_id, &play_b.active_play_id, "request/b"),
        Ok(())
    );
}
