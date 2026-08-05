use crate::process::Step;

pub static FORM_S3: &[Step] = &[
    Step::new(
        "form-s3-lossless-document",
        "Test lossless_document_",
        "cargo",
        &["test", "-p", "conduit-form", "lossless_document_"],
    ),
    Step::new(
        "form-s3-missing-close",
        "Test missing_close_is_diagnosed_at_eof_without_losing_source",
        "cargo",
        &["test", "-p", "conduit-form", "missing_close_is_diagnosed_at_eof_without_losing_source"],
    ),
    Step::new(
        "form-s3-identities-distinct",
        "Test source_checked_and_expanded_form_identities_stay_distinct",
        "cargo",
        &["test", "-p", "conduit-form", "source_checked_and_expanded_form_identities_stay_distinct"],
    ),
    Step::new(
        "form-s3-export-boundary",
        "Test checked_export_is_the_only_source_of_a_parent_kind_boundary",
        "cargo",
        &["test", "-p", "conduit-form", "checked_export_is_the_only_source_of_a_parent_kind_boundary"],
    ),
    Step::new(
        "form-s3-duplicate-export",
        "Test duplicate_export_capabilities_are_rejected",
        "cargo",
        &["test", "-p", "conduit-form", "duplicate_export_capabilities_are_rejected"],
    ),
    Step::new(
        "form-s3-multiple-faces",
        "Test multiple_typed_and_zero_sided_faces_check_as_ordinary_kinds",
        "cargo",
        &["test", "-p", "conduit-form", "multiple_typed_and_zero_sided_faces_check_as_ordinary_kinds"],
    ),
    Step::new(
        "form-s3-face-mutations",
        "Test checked_face_mutations_fail_closed",
        "cargo",
        &["test", "-p", "conduit-form", "checked_face_mutations_fail_closed"],
    ),
    Step::new(
        "form-s3-inline-nested",
        "Test inline_nested_form_uses_the_same_checked_boundary_as_a_standalone_form",
        "cargo",
        &["test", "-p", "conduit-form", "inline_nested_form_uses_the_same_checked_boundary_as_a_standalone_form"],
    ),
    Step::new(
        "form-s3-parent-expanded-identity",
        "Test parent_expanded_identity_binds_hidden_child_semantics_not_checked_boundary",
        "cargo",
        &["test", "-p", "conduit-form", "parent_expanded_identity_binds_hidden_child_semantics_not_checked_boundary"],
    ),
    Step::new(
        "form-s3-nested-expansion-paths",
        "Test nested_expansion_paths_are_canonical_and_substitution_fails_closed",
        "cargo",
        &["test", "-p", "conduit-form", "nested_expansion_paths_are_canonical_and_substitution_fails_closed"],
    ),
    Step::new(
        "form-s3-planner-nested-expansion",
        "Test planning_binds_nested_expansion_changes_beyond_the_checked_boundary",
        "cargo",
        &["test", "-p", "conduit-planner", "planning_binds_nested_expansion_changes_beyond_the_checked_boundary"],
    ),
    Step::new(
        "form-s3-nested-errors",
        "Test nested_errors_keep_the_outer_document_and_exact_inner_span",
        "cargo",
        &["test", "-p", "conduit-form", "nested_errors_keep_the_outer_document_and_exact_inner_span"],
    ),
    Step::new(
        "form-s3-depth-ceiling",
        "Test inline_nesting_has_a_hard_depth_ceiling",
        "cargo",
        &["test", "-p", "conduit-form", "inline_nesting_has_a_hard_depth_ceiling"],
    ),
    Step::new(
        "form-s3-composite-parent",
        "Test authored_parent_consumes_derived_export_through_an_ordinary_planned_cord",
        "cargo",
        &["test", "-p", "conduit-composite", "authored_parent_consumes_derived_export_through_an_ordinary_planned_cord"],
    ),
    Step::new(
        "form-s3-composite-multi-kind",
        "Test two_input_two_output_multi_kind_faces_execute_with_exact_pressure_and_closure",
        "cargo",
        &["test", "-p", "conduit-composite", "two_input_two_output_multi_kind_faces_execute_with_exact_pressure_and_closure"],
    ),
    Step::new(
        "form-s3-input-output-exports",
        "Test input_only_and_output_only_exports_plan_as_ordinary_operations",
        "cargo",
        &["test", "-p", "conduit-composite", "input_only_and_output_only_exports_plan_as_ordinary_operations"],
    ),
    Step::new(
        "form-s3-composite-definition",
        "Test composite_definition_rejects_every_face_mapping_mutation",
        "cargo",
        &["test", "-p", "conduit-composite", "composite_definition_rejects_every_face_mapping_mutation"],
    ),
    Step::new(
        "form-s3-named-face-failure",
        "Test named_face_delivery_failure_and_cancellation_are_parent_terminal_without_topology_leaks",
        "cargo",
        &["test", "-p", "conduit-composite", "named_face_delivery_failure_and_cancellation_are_parent_terminal_without_topology_leaks"],
    ),
    Step::new(
        "form-s3-execution-identity-chain",
        "Test execution_identity_chain_keeps_plan_play_evidence_and_presentation_distinct",
        "cargo",
        &["test", "-p", "conduit-core", "execution_identity_chain_keeps_plan_play_evidence_and_presentation_distinct"],
    ),
    Step::new(
        "form-s3-fake-adapter-failure",
        "Test fake_adapter_failure_is_structured_and_terminal",
        "cargo",
        &[
            "test", "-p", "conduit-runtime", "--test", "host_contract",
            "fake_adapter_failure_is_structured_and_terminal",
        ],
    ),
    Step::new(
        "form-s3-observatory-report",
        "Test report_separates_identity_capability_plan_connection_and_evidence_tables",
        "cargo",
        &["test", "-p", "conduit-observatory", "report_separates_identity_capability_plan_connection_and_evidence_tables"],
    ),
];
