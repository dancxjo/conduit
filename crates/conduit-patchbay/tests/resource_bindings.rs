use conduit_patchbay::{
    BindingPrincipal, GrantConfirmationOutcome, PATCHBAY_PROTOCOL_VERSION,
    ProtectedBindingExportPolicy, ProtectedBindingProfile, ProtectedGrantConfirmation,
    ProtectedSelectionCompletion, ProtectedValue, ResourceAccessScope, ResourceBindingError,
    ResourceBindingRequestAction, ResourceBindingRequestEnvelope, ResourceBindingSlot,
    ResourceSelectionAccessProfile, ResourceSelectionOperation, SelectionCompletionOutcome,
    SelectionProviderObservation, SelectionProviderState, Workspace,
};

const SOURCE_SLOT: &str = "conduit.binding/copy/source-file";
const DESTINATION_SLOT: &str = "conduit.binding/copy/destination-file";

fn copy_profile() -> ProtectedBindingProfile {
    copy_profile_for("conduit.binding-profile/copy-files", BindingPrincipal::Site)
}

fn copy_profile_for(profile_id: &str, principal: BindingPrincipal) -> ProtectedBindingProfile {
    ProtectedBindingProfile::new(
        profile_id,
        vec![
            ResourceBindingSlot {
                id: SOURCE_SLOT.to_owned(),
                resource_reference: "conduit.resource-binding/copy-source".to_owned(),
                grant_reference: "conduit.grant-binding/copy-source-read".to_owned(),
                resource_kind: "conduit.resource/filesystem-file".to_owned(),
                required_profile: "conduit.filesystem/read-file".to_owned(),
                principal,
                allowed_selection: vec![ResourceSelectionOperation::Choose],
                selection_access: vec![ResourceSelectionAccessProfile {
                    operation: ResourceSelectionOperation::Choose,
                    access: vec![ResourceAccessScope::Read],
                }],
                disallow_same_resource_as: vec![DESTINATION_SLOT.to_owned()],
            },
            ResourceBindingSlot {
                id: DESTINATION_SLOT.to_owned(),
                resource_reference: "conduit.resource-binding/copy-destination".to_owned(),
                grant_reference: "conduit.grant-binding/copy-destination-write".to_owned(),
                resource_kind: "conduit.resource/filesystem-file".to_owned(),
                required_profile: "conduit.filesystem/write-file".to_owned(),
                principal,
                allowed_selection: vec![
                    ResourceSelectionOperation::CreateNew,
                    ResourceSelectionOperation::ReplaceExisting,
                ],
                selection_access: vec![
                    ResourceSelectionAccessProfile {
                        operation: ResourceSelectionOperation::CreateNew,
                        access: vec![ResourceAccessScope::Write, ResourceAccessScope::Create],
                    },
                    ResourceSelectionAccessProfile {
                        operation: ResourceSelectionOperation::ReplaceExisting,
                        access: vec![ResourceAccessScope::Write, ResourceAccessScope::Replace],
                    },
                ],
                disallow_same_resource_as: vec![SOURCE_SLOT.to_owned()],
            },
        ],
    )
    .expect("valid Copy binding profile")
}

#[test]
fn same_source_has_distinct_user_and_site_binding_profile_identities() {
    let source = "panel 0\nreader: fixture/source\n";
    let user_workspace = Workspace::new("copy-user", source).unwrap();
    let site_workspace = Workspace::new("copy-site", source).unwrap();
    let user = copy_profile_for(
        "conduit.binding-profile/copy-files-user",
        BindingPrincipal::User,
    );
    let site = copy_profile_for(
        "conduit.binding-profile/copy-files-site",
        BindingPrincipal::Site,
    );
    assert_eq!(
        user_workspace.semantic().source_semantic_hash,
        site_workspace.semantic().source_semantic_hash
    );
    assert_ne!(user.projection().identity, site.projection().identity);
    assert_eq!(user.projection().slots[0].principal, BindingPrincipal::User);
    assert_eq!(site.projection().slots[0].principal, BindingPrincipal::Site);
}

fn request(
    request_id: &str,
    operation_id: &str,
    slot_id: &str,
    revision: u64,
    provider: &SelectionProviderObservation,
    action: ResourceBindingRequestAction,
    access: Vec<ResourceAccessScope>,
) -> ResourceBindingRequestEnvelope {
    ResourceBindingRequestEnvelope {
        protocol_version: PATCHBAY_PROTOCOL_VERSION,
        request_id: request_id.to_owned(),
        operation_id: operation_id.to_owned(),
        slot_id: slot_id.to_owned(),
        expected_binding_revision: revision,
        provider_id: provider.id.clone(),
        provider_generation: provider.generation,
        action,
        requested_access: access,
        selection_request_id: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn select(
    profile: &mut ProtectedBindingProfile,
    provider: &SelectionProviderObservation,
    request_id: &str,
    operation_id: &str,
    slot_id: &str,
    revision: u64,
    action: ResourceBindingRequestAction,
    access: Vec<ResourceAccessScope>,
    opaque_handle: &str,
    safe_label: &str,
) -> u64 {
    let request = request(
        request_id,
        operation_id,
        slot_id,
        revision,
        provider,
        action,
        access,
    );
    profile
        .begin_selection(&request, provider, 10)
        .expect("selection begins");
    let receipt = profile
        .complete_selection(
            ProtectedSelectionCompletion {
                request_id: request.request_id,
                operation_id: request.operation_id,
                provider_id: provider.id.clone(),
                provider_observation_id: provider.observation_id.clone(),
                provider_generation: provider.generation,
                completed_at_tick: 10,
                outcome: SelectionCompletionOutcome::Selected,
                opaque_handle: Some(ProtectedValue::new(opaque_handle).expect("opaque handle")),
                safe_label: Some(safe_label.to_owned()),
            },
            provider,
        )
        .expect("selection completes");
    receipt.binding_revision
}

fn grant(
    profile: &mut ProtectedBindingProfile,
    provider: &SelectionProviderObservation,
    request_id: &str,
    slot_id: &str,
    revision: u64,
    access: Vec<ResourceAccessScope>,
    grant: &str,
) -> u64 {
    let request = request(
        request_id,
        "conduit.operation/copy-bindings",
        slot_id,
        revision,
        provider,
        ResourceBindingRequestAction::ConfirmGrant,
        access.clone(),
    );
    profile
        .confirm_grant(
            &request,
            ProtectedGrantConfirmation {
                slot_id: slot_id.to_owned(),
                expected_binding_revision: revision,
                authority_observation_id: "conduit.authority-observation/site".to_owned(),
                authority_generation: 1,
                outcome: GrantConfirmationOutcome::Granted,
                grant: Some(ProtectedValue::new(grant).expect("opaque grant")),
                access,
            },
        )
        .expect("grant confirmation")
        .binding_revision
}

#[test]
fn copy_bindings_require_separate_selection_and_exact_scope_grant() {
    let provider = SelectionProviderObservation::deterministic_files(10);
    let mut profile = copy_profile();
    let selected_revision = select(
        &mut profile,
        &provider,
        "conduit.request/select-source",
        "conduit.operation/copy-bindings",
        SOURCE_SLOT,
        0,
        ResourceBindingRequestAction::Choose,
        vec![ResourceAccessScope::Read],
        "conduit.opaque-resource/source-canary",
        "source.txt",
    );

    assert_eq!(
        profile.resolve(SOURCE_SLOT).unwrap_err(),
        ResourceBindingError::GrantRequired
    );
    let json = serde_json::to_string(&profile.projection()).expect("safe projection JSON");
    assert!(json.contains("source.txt"));
    assert!(!json.contains("source-canary"));
    assert!(!json.contains("copy-source-read"));

    let ready_revision = grant(
        &mut profile,
        &provider,
        "conduit.request/grant-source",
        SOURCE_SLOT,
        selected_revision,
        vec![ResourceAccessScope::Read],
        "conduit.opaque-grant/source-read-canary",
    );
    let exact = profile
        .resolve(SOURCE_SLOT)
        .expect("ready exact resolution");
    assert_eq!(exact.binding_revision, ready_revision);
    assert_eq!(exact.resource(), "conduit.opaque-resource/source-canary");
    assert_eq!(exact.grant(), "conduit.opaque-grant/source-read-canary");
    assert_eq!(exact.access, &[ResourceAccessScope::Read]);
    let debug = format!("{exact:?}");
    assert!(!debug.contains("source-canary"));
    assert!(!debug.contains("source-read-canary"));
}

#[test]
fn provider_profiles_are_honest_about_enumeration_replace_and_export() {
    let deterministic = SelectionProviderObservation::deterministic_files(10);
    let browser = SelectionProviderObservation::browser_files(10);
    let hosted = SelectionProviderObservation::hosted_local_files(10);
    let unsupported = SelectionProviderObservation::unsupported_files(10);

    assert!(deterministic.enumeration_authorized);
    assert!(
        deterministic
            .supported_operations
            .contains(&ResourceSelectionOperation::ReplaceExisting)
    );
    assert!(!browser.enumeration_authorized);
    assert!(
        browser
            .supported_operations
            .contains(&ResourceSelectionOperation::ReplaceExisting)
    );
    assert!(
        browser
            .supported_operations
            .contains(&ResourceSelectionOperation::DownloadExport)
    );
    assert!(!hosted.enumeration_authorized);
    assert!(
        hosted
            .supported_operations
            .contains(&ResourceSelectionOperation::ReplaceExisting)
    );
    assert_eq!(unsupported.state, SelectionProviderState::Unsupported);

    let mut wrong_kind = deterministic.clone();
    wrong_kind.resource_kind = "conduit.resource/credential-reference".to_owned();
    let wrong_kind_request = request(
        "conduit.request/wrong-resource-kind",
        "conduit.operation/copy-bindings",
        SOURCE_SLOT,
        0,
        &wrong_kind,
        ResourceBindingRequestAction::Choose,
        vec![ResourceAccessScope::Read],
    );
    assert_eq!(
        copy_profile().begin_selection(&wrong_kind_request, &wrong_kind, 10),
        Err(ResourceBindingError::Unsupported)
    );

    let mut profile = copy_profile();
    let browser_replace = request(
        "conduit.request/browser-replace",
        "conduit.operation/copy-bindings",
        DESTINATION_SLOT,
        0,
        &browser,
        ResourceBindingRequestAction::ReplaceExisting,
        vec![ResourceAccessScope::Write, ResourceAccessScope::Create],
    );
    assert_eq!(
        profile.begin_selection(&browser_replace, &browser, 10),
        Err(ResourceBindingError::AccessDenied)
    );
    let browser_replace = request(
        "conduit.request/browser-replace-exact",
        "conduit.operation/copy-bindings",
        DESTINATION_SLOT,
        0,
        &browser,
        ResourceBindingRequestAction::ReplaceExisting,
        vec![ResourceAccessScope::Write, ResourceAccessScope::Replace],
    );
    assert!(
        profile
            .begin_selection(&browser_replace, &browser, 10)
            .is_ok()
    );

    let mut create_profile = copy_profile();
    let wrong_create_scope = request(
        "conduit.request/create-with-replace-scope",
        "conduit.operation/copy-bindings",
        DESTINATION_SLOT,
        0,
        &deterministic,
        ResourceBindingRequestAction::CreateNew,
        vec![ResourceAccessScope::Write, ResourceAccessScope::Replace],
    );
    assert_eq!(
        create_profile.begin_selection(&wrong_create_scope, &deterministic, 10),
        Err(ResourceBindingError::AccessDenied)
    );
    let exact_create_scope = request(
        "conduit.request/create-with-create-scope",
        "conduit.operation/copy-bindings",
        DESTINATION_SLOT,
        0,
        &deterministic,
        ResourceBindingRequestAction::CreateNew,
        vec![ResourceAccessScope::Write, ResourceAccessScope::Create],
    );
    assert!(
        create_profile
            .begin_selection(&exact_create_scope, &deterministic, 10)
            .is_ok()
    );
}

#[test]
fn cancellation_duplicate_requests_and_late_prior_revision_callbacks_fail_closed() {
    let provider = SelectionProviderObservation::deterministic_files(10);
    let mut profile = copy_profile();
    let pending = request(
        "conduit.request/source-old",
        "conduit.operation/copy-bindings",
        SOURCE_SLOT,
        0,
        &provider,
        ResourceBindingRequestAction::Choose,
        vec![ResourceAccessScope::Read],
    );
    profile
        .begin_selection(&pending, &provider, 10)
        .expect("pending old chooser");
    assert_eq!(
        profile.begin_selection(&pending, &provider, 10),
        Err(ResourceBindingError::DuplicateRequest)
    );
    let mut cancel = request(
        "conduit.request/cancel-source-old",
        "conduit.operation/copy-bindings",
        SOURCE_SLOT,
        0,
        &provider,
        ResourceBindingRequestAction::Cancel,
        Vec::new(),
    );
    cancel.selection_request_id = Some(pending.request_id.clone());
    profile
        .cancel_selection(&cancel)
        .expect("cancel pending chooser");
    let late = ProtectedSelectionCompletion {
        request_id: pending.request_id,
        operation_id: pending.operation_id,
        provider_id: provider.id.clone(),
        provider_observation_id: provider.observation_id.clone(),
        provider_generation: provider.generation,
        completed_at_tick: 10,
        outcome: SelectionCompletionOutcome::Selected,
        opaque_handle: Some(ProtectedValue::new("conduit.opaque-resource/late").unwrap()),
        safe_label: Some("late.txt".to_owned()),
    };
    assert_eq!(
        profile.complete_selection(late, &provider),
        Err(ResourceBindingError::CancelledRequest)
    );

    let old = request(
        "conduit.request/destination-old",
        "conduit.operation/copy-bindings",
        DESTINATION_SLOT,
        0,
        &provider,
        ResourceBindingRequestAction::CreateNew,
        vec![ResourceAccessScope::Write, ResourceAccessScope::Create],
    );
    profile.begin_selection(&old, &provider, 10).unwrap();
    select(
        &mut profile,
        &provider,
        "conduit.request/destination-new",
        "conduit.operation/copy-bindings",
        DESTINATION_SLOT,
        0,
        ResourceBindingRequestAction::CreateNew,
        vec![ResourceAccessScope::Write, ResourceAccessScope::Create],
        "conduit.opaque-resource/destination-new",
        "new.txt",
    );
    let late_prior_revision = ProtectedSelectionCompletion {
        request_id: old.request_id,
        operation_id: old.operation_id,
        provider_id: provider.id.clone(),
        provider_observation_id: provider.observation_id.clone(),
        provider_generation: provider.generation,
        completed_at_tick: 10,
        outcome: SelectionCompletionOutcome::Selected,
        opaque_handle: Some(
            ProtectedValue::new("conduit.opaque-resource/destination-old").unwrap(),
        ),
        safe_label: Some("old.txt".to_owned()),
    };
    assert_eq!(
        profile.complete_selection(late_prior_revision, &provider),
        Err(ResourceBindingError::WrongRevision)
    );
}

#[test]
fn permission_grant_provider_and_resource_failures_remain_distinct() {
    let provider = SelectionProviderObservation::deterministic_files(10);
    for (outcome, expected) in [
        (
            SelectionCompletionOutcome::PermissionDenied,
            ResourceBindingError::PermissionDenied,
        ),
        (
            SelectionCompletionOutcome::ResourceDisappeared,
            ResourceBindingError::ResourceDisappeared,
        ),
        (
            SelectionCompletionOutcome::ProviderDisappeared,
            ResourceBindingError::ProviderDisappeared,
        ),
    ] {
        let mut profile = copy_profile();
        let pending = request(
            "conduit.request/denied-source",
            "conduit.operation/copy-bindings",
            SOURCE_SLOT,
            0,
            &provider,
            ResourceBindingRequestAction::Choose,
            vec![ResourceAccessScope::Read],
        );
        profile.begin_selection(&pending, &provider, 10).unwrap();
        assert_eq!(
            profile.complete_selection(
                ProtectedSelectionCompletion {
                    request_id: pending.request_id,
                    operation_id: pending.operation_id,
                    provider_id: provider.id.clone(),
                    provider_observation_id: provider.observation_id.clone(),
                    provider_generation: provider.generation,
                    completed_at_tick: 10,
                    outcome,
                    opaque_handle: None,
                    safe_label: None,
                },
                &provider,
            ),
            Err(expected)
        );
    }

    let mut profile = copy_profile();
    let revision = select(
        &mut profile,
        &provider,
        "conduit.request/source-selected",
        "conduit.operation/copy-bindings",
        SOURCE_SLOT,
        0,
        ResourceBindingRequestAction::Choose,
        vec![ResourceAccessScope::Read],
        "conduit.opaque-resource/source",
        "source.txt",
    );
    let denial = request(
        "conduit.request/source-grant-denied",
        "conduit.operation/copy-bindings",
        SOURCE_SLOT,
        revision,
        &provider,
        ResourceBindingRequestAction::ConfirmGrant,
        vec![ResourceAccessScope::Read],
    );
    profile
        .confirm_grant(
            &denial,
            ProtectedGrantConfirmation {
                slot_id: SOURCE_SLOT.to_owned(),
                expected_binding_revision: revision,
                authority_observation_id: "conduit.authority-observation/site".to_owned(),
                authority_generation: 1,
                outcome: GrantConfirmationOutcome::Denied,
                grant: None,
                access: vec![ResourceAccessScope::Read],
            },
        )
        .unwrap();
    assert_eq!(
        profile.resolve(SOURCE_SLOT).unwrap_err(),
        ResourceBindingError::GrantDenied
    );
}

#[test]
fn copy_rejects_source_as_destination_and_does_not_compare_safe_labels() {
    let provider = SelectionProviderObservation::deterministic_files(10);
    let mut profile = copy_profile();
    select(
        &mut profile,
        &provider,
        "conduit.request/source",
        "conduit.operation/copy-bindings",
        SOURCE_SLOT,
        0,
        ResourceBindingRequestAction::Choose,
        vec![ResourceAccessScope::Read],
        "conduit.opaque-resource/same",
        "file.txt",
    );
    let destination = request(
        "conduit.request/destination-same",
        "conduit.operation/copy-bindings",
        DESTINATION_SLOT,
        0,
        &provider,
        ResourceBindingRequestAction::ReplaceExisting,
        vec![ResourceAccessScope::Write, ResourceAccessScope::Replace],
    );
    profile
        .begin_selection(&destination, &provider, 10)
        .unwrap();
    assert_eq!(
        profile.complete_selection(
            ProtectedSelectionCompletion {
                request_id: destination.request_id,
                operation_id: destination.operation_id,
                provider_id: provider.id.clone(),
                provider_observation_id: provider.observation_id.clone(),
                provider_generation: provider.generation,
                completed_at_tick: 10,
                outcome: SelectionCompletionOutcome::Selected,
                opaque_handle: Some(ProtectedValue::new("conduit.opaque-resource/same").unwrap()),
                safe_label: Some("another-label.txt".to_owned()),
            },
            &provider,
        ),
        Err(ResourceBindingError::AccessDenied)
    );

    select(
        &mut profile,
        &provider,
        "conduit.request/destination-distinct",
        "conduit.operation/copy-bindings",
        DESTINATION_SLOT,
        0,
        ResourceBindingRequestAction::ReplaceExisting,
        vec![ResourceAccessScope::Write, ResourceAccessScope::Replace],
        "conduit.opaque-resource/distinct",
        "file.txt",
    );
    let projection = profile.projection();
    assert_eq!(
        projection.slots[0].safe_label,
        projection.slots[1].safe_label
    );
}

#[test]
fn provider_generation_change_invalidates_candidate_resolution_without_leaking_binding() {
    let provider = SelectionProviderObservation::deterministic_files(10);
    let mut stale_profile = copy_profile();
    let stale_request = request(
        "conduit.request/source-stale-time",
        "conduit.operation/copy-bindings",
        SOURCE_SLOT,
        0,
        &provider,
        ResourceBindingRequestAction::Choose,
        vec![ResourceAccessScope::Read],
    );
    stale_profile
        .begin_selection(&stale_request, &provider, 10)
        .unwrap();
    assert_eq!(
        stale_profile.complete_selection(
            ProtectedSelectionCompletion {
                request_id: stale_request.request_id,
                operation_id: stale_request.operation_id,
                provider_id: provider.id.clone(),
                provider_observation_id: provider.observation_id.clone(),
                provider_generation: provider.generation,
                completed_at_tick: provider.valid_until_tick + 1,
                outcome: SelectionCompletionOutcome::Selected,
                opaque_handle: Some(
                    ProtectedValue::new("conduit.opaque-resource/stale-time").unwrap(),
                ),
                safe_label: Some("stale.txt".to_owned()),
            },
            &provider,
        ),
        Err(ResourceBindingError::StaleProvider)
    );

    let mut profile = copy_profile();
    let selected = select(
        &mut profile,
        &provider,
        "conduit.request/source-provider",
        "conduit.operation/copy-bindings",
        SOURCE_SLOT,
        0,
        ResourceBindingRequestAction::Choose,
        vec![ResourceAccessScope::Read],
        "conduit.opaque-resource/provider-canary",
        "safe.txt",
    );
    grant(
        &mut profile,
        &provider,
        "conduit.request/source-provider-grant",
        SOURCE_SLOT,
        selected,
        vec![ResourceAccessScope::Read],
        "conduit.opaque-grant/provider-canary",
    );
    assert!(profile.resolve(SOURCE_SLOT).is_ok());

    let mut changed = provider.clone();
    changed.generation += 1;
    changed.observation_id = "conduit.selector-observation/deterministic-files-new".to_owned();
    profile.reconcile_provider(&changed, 11);
    assert_eq!(
        profile.resolve(SOURCE_SLOT).unwrap_err(),
        ResourceBindingError::ProviderDisappeared
    );
    let json = serde_json::to_string(&profile.projection()).unwrap();
    assert!(json.contains("provider-disappeared"));
    assert!(!json.contains("provider-canary"));
}

#[test]
fn protected_export_policy_redacts_or_refuses_without_transferring_authority() {
    let provider = SelectionProviderObservation::deterministic_files(10);
    let mut profile = copy_profile();
    let selected = select(
        &mut profile,
        &provider,
        "conduit.request/export-source",
        "conduit.operation/copy-bindings",
        SOURCE_SLOT,
        0,
        ResourceBindingRequestAction::Choose,
        vec![ResourceAccessScope::Read],
        "conduit.opaque-resource/export-canary",
        "safe-export.txt",
    );
    grant(
        &mut profile,
        &provider,
        "conduit.request/export-source-grant",
        SOURCE_SLOT,
        selected,
        vec![ResourceAccessScope::Read],
        "conduit.opaque-grant/export-canary",
    );

    assert_eq!(
        profile.export_projection(ProtectedBindingExportPolicy::Refuse),
        Err(ResourceBindingError::ProtectedExportRefused)
    );
    let redacted = profile
        .export_projection(ProtectedBindingExportPolicy::RedactedSafeMetadata)
        .expect("safe metadata export");
    let json = serde_json::to_string(&redacted).unwrap();
    assert!(json.contains("safe-export.txt"));
    assert!(!json.contains("export-canary"));
    assert_eq!(
        profile.resolve(SOURCE_SLOT).unwrap().access,
        &[ResourceAccessScope::Read]
    );
}

#[test]
fn inspect_revoke_and_forget_preserve_separate_authority_and_binding_states() {
    let provider = SelectionProviderObservation::deterministic_files(10);
    let mut profile = copy_profile();
    let selected = select(
        &mut profile,
        &provider,
        "conduit.request/lifecycle-select",
        "conduit.operation/copy-bindings",
        SOURCE_SLOT,
        0,
        ResourceBindingRequestAction::Choose,
        vec![ResourceAccessScope::Read],
        "conduit.opaque-resource/lifecycle-canary",
        "lifecycle.txt",
    );
    let ready = grant(
        &mut profile,
        &provider,
        "conduit.request/lifecycle-grant",
        SOURCE_SLOT,
        selected,
        vec![ResourceAccessScope::Read],
        "conduit.opaque-grant/lifecycle-canary",
    );
    let inspected = profile
        .inspect(&request(
            "conduit.request/lifecycle-inspect",
            "conduit.operation/copy-bindings",
            SOURCE_SLOT,
            ready,
            &provider,
            ResourceBindingRequestAction::Inspect,
            Vec::new(),
        ))
        .unwrap();
    assert_eq!(inspected.disposition, "inspected");

    let revoked = profile
        .revoke(&request(
            "conduit.request/lifecycle-revoke",
            "conduit.operation/copy-bindings",
            SOURCE_SLOT,
            ready,
            &provider,
            ResourceBindingRequestAction::Revoke,
            vec![ResourceAccessScope::Read],
        ))
        .unwrap();
    assert_eq!(profile.projection().slots[0].state, "revoked");
    assert_eq!(
        profile.resolve(SOURCE_SLOT).unwrap_err(),
        ResourceBindingError::GrantRequired
    );
    profile
        .forget(&request(
            "conduit.request/lifecycle-forget",
            "conduit.operation/copy-bindings",
            SOURCE_SLOT,
            revoked.binding_revision,
            &provider,
            ResourceBindingRequestAction::Forget,
            Vec::new(),
        ))
        .unwrap();
    let projection = profile.projection();
    assert_eq!(projection.slots[0].state, "selection-required");
    assert!(projection.slots[0].safe_label.is_none());
    assert!(
        !serde_json::to_string(&projection)
            .unwrap()
            .contains("lifecycle-canary")
    );
}
