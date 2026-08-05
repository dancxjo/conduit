use crate::process::Step;

pub static PLANNING_S2: &[Step] = &[
    Step::new(
        "planning-s2-form-identity",
        "Test checked_form_identity_binds_contract_revision_and_ports",
        "cargo",
        &["test", "-p", "conduit-form", "checked_form_identity_binds_contract_revision_and_ports"],
    ),
    Step::new(
        "planning-s2-source-identities-distinct",
        "Test source_checked_and_expanded_form_identities_stay_distinct",
        "cargo",
        &["test", "-p", "conduit-form", "source_checked_and_expanded_form_identities_stay_distinct"],
    ),
    Step::new(
        "planning-s2-planning",
        "Test planning_ (conduit-planner)",
        "cargo",
        &["test", "-p", "conduit-planner", "planning_"],
    ),
    Step::new(
        "planning-s2-rejects-mutation",
        "Test preparation_rejects_mutation_of_every_executable_identity_field_group",
        "cargo",
        &[
            "test", "-p", "conduit-runtime", "--test", "host_contract",
            "preparation_rejects_mutation_of_every_executable_identity_field_group",
        ],
    ),
    Step::new(
        "planning-s2-rejects-resealed-contract",
        "Test preparation_rejects_resealed_contract_profile_and_port_lies",
        "cargo",
        &[
            "test", "-p", "conduit-runtime", "--test", "host_contract",
            "preparation_rejects_resealed_contract_profile_and_port_lies",
        ],
    ),
    Step::new(
        "planning-s2-rejects-resealed-policy",
        "Test preparation_rejects_resealed_policy_dependency_and_budget_lies",
        "cargo",
        &[
            "test", "-p", "conduit-runtime", "--test", "host_contract",
            "preparation_rejects_resealed_policy_dependency_and_budget_lies",
        ],
    ),
    Step::new(
        "planning-s2-rejects-unplanned-op",
        "Test runtime_rejects_an_implementation_that_requests_an_unplanned_host_operation",
        "cargo",
        &[
            "test", "-p", "conduit-runtime", "--test", "host_contract",
            "runtime_rejects_an_implementation_that_requests_an_unplanned_host_operation",
        ],
    ),
    Step::new(
        "planning-s2-rejects-op-above-bound",
        "Test runtime_rejects_a_host_operation_input_above_its_planned_bound",
        "cargo",
        &[
            "test", "-p", "conduit-runtime", "--test", "host_contract",
            "runtime_rejects_a_host_operation_input_above_its_planned_bound",
        ],
    ),
    Step::new(
        "planning-s2-fake-browser-adapter",
        "Test fake_browser_style_adapter_drives_effects_delay_disconnect_and_inspection",
        "cargo",
        &[
            "test", "-p", "conduit-runtime", "--test", "host_contract",
            "fake_browser_style_adapter_drives_effects_delay_disconnect_and_inspection",
        ],
    ),
    Step::new(
        "planning-s2-resource-pool",
        "Test preparation_reserves_resource_pool_capacity_until_release",
        "cargo",
        &[
            "test", "-p", "conduit-runtime", "--test", "host_contract",
            "preparation_reserves_resource_pool_capacity_until_release",
        ],
    ),
    Step::new(
        "planning-s2-authority-binding",
        "Test authority_binding_mutations_change_fragment_identity",
        "cargo",
        &[
            "test", "-p", "conduit-runtime", "--test", "host_contract",
            "authority_binding_mutations_change_fragment_identity",
        ],
    ),
    Step::new(
        "planning-s2-exact-authority-grant",
        "Test preparation_and_effect_admission_require_the_exact_current_authority_grant",
        "cargo",
        &[
            "test", "-p", "conduit-runtime", "--test", "host_contract",
            "preparation_and_effect_admission_require_the_exact_current_authority_grant",
        ],
    ),
    Step::new(
        "planning-s2-effect-outside-grant",
        "Test effect_admission_rejects_a_planned_host_operation_outside_the_bound_grant_subject",
        "cargo",
        &[
            "test", "-p", "conduit-runtime", "--test", "host_contract",
            "effect_admission_rejects_a_planned_host_operation_outside_the_bound_grant_subject",
        ],
    ),
    Step::new(
        "planning-s2-boot-link",
        "Test preparation_requires_the_exact_current_boot_scoped_link_observation",
        "cargo",
        &[
            "test", "-p", "conduit-runtime", "--test", "host_contract",
            "preparation_requires_the_exact_current_boot_scoped_link_observation",
        ],
    ),
    Step::new(
        "planning-s2-evidence-overflow",
        "Test planned_evidence_storage_survives_observation_overflow",
        "cargo",
        &[
            "test", "-p", "conduit-runtime", "--test", "host_contract",
            "planned_evidence_storage_survives_observation_overflow",
        ],
    ),
    Step::new(
        "planning-s2-core-thumb",
        "Check conduit-core for thumbv6m-none-eabi",
        "cargo",
        &["check", "-p", "conduit-core", "--target", "thumbv6m-none-eabi"],
    ),
];
