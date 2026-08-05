std-host:
    cargo run -p conduit -- examples/signal-demo.form --placements examples/std-local.placements

demo-std:
    cargo run -p conduit -- examples/signal-demo.form --placements examples/std-local.placements

demo-triple-local:
    cargo run -p conduit -- examples/triple-signal.form --placements examples/triple-local.placements

browser-sim:
    cargo test -p conduit-browser-sim

browser-frame-fixture:
    cargo test -p conduit-browser-sim std_host_sends_signal_to_browser_through_bounded_frame_fixture

triple-sim-proof:
    cargo test -p conduit-browser-sim triple_signal_form_fans_out_to_std_and_simulated_receipts

browser-sim-wasm-check:
    cargo check -p conduit-browser-sim --target wasm32-unknown-unknown

pico-sim:
    cargo test -p conduit-pico-sim

pico-datagram-fixture:
    cargo test -p conduit-pico-sim std_host_sends_signal_to_pico_through_bounded_datagram_fixture

pico-sim-thumb-check:
    cargo check -p conduit-pico-sim --no-default-features --target thumbv6m-none-eabi

realm:
    cargo test -p conduit-realm

realm-thumb-check:
    cargo check -p conduit-realm --target thumbv6m-none-eabi

observatory:
    cargo test -p conduit-observatory

observatory-thumb-check:
    cargo check -p conduit-observatory --target thumbv6m-none-eabi

std-catalog:
    cargo test -p conduit-std-catalog

std-catalog-thumb-check:
    cargo check -p conduit-std-catalog --no-default-features --target thumbv6m-none-eabi

check:
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

check-kernel-s1:
    cargo test -p conduit-kernel --features alloc
    cargo check -p conduit-kernel --target thumbv6m-none-eabi

check-kernel-takeover:
    cargo test -p conduit-std-host exact_signal_fragment_lowers_to_numeric_kernel_tables
    cargo test -p conduit-std-host streamed_output_uses_a_virtual_clock_and_retains_terminal_evidence
    cargo test -p conduit-std-host kernel_multivalue
    cargo test -p conduit --test hello typed_multi_value_form_runs_through_the_std_kernel
    cargo test -p conduit-kernel admitted_sink_host_operation_may_have_no_output_payload
    cargo check -p conduit-kernel --target thumbv6m-none-eabi
    cargo check -p conduit-core --target thumbv6m-none-eabi

check-planning-s2:
    cargo test -p conduit-form checked_form_identity_binds_contract_revision_and_ports
    cargo test -p conduit-form source_checked_and_expanded_form_identities_stay_distinct
    cargo test -p conduit-planner planning_
    cargo test -p conduit-runtime --test host_contract preparation_rejects_mutation_of_every_executable_identity_field_group
    cargo test -p conduit-runtime --test host_contract preparation_rejects_resealed_contract_profile_and_port_lies
    cargo test -p conduit-runtime --test host_contract preparation_rejects_resealed_policy_dependency_and_budget_lies
    cargo test -p conduit-runtime --test host_contract runtime_rejects_an_implementation_that_requests_an_unplanned_host_operation
    cargo test -p conduit-runtime --test host_contract runtime_rejects_a_host_operation_input_above_its_planned_bound
    cargo test -p conduit-runtime --test host_contract fake_browser_style_adapter_drives_effects_delay_disconnect_and_inspection
    cargo test -p conduit-runtime --test host_contract preparation_reserves_resource_pool_capacity_until_release
    cargo test -p conduit-runtime --test host_contract authority_binding_mutations_change_fragment_identity
    cargo test -p conduit-runtime --test host_contract preparation_and_effect_admission_require_the_exact_current_authority_grant
    cargo test -p conduit-runtime --test host_contract effect_admission_rejects_a_planned_host_operation_outside_the_bound_grant_subject
    cargo test -p conduit-runtime --test host_contract preparation_requires_the_exact_current_boot_scoped_link_observation
    cargo test -p conduit-runtime --test host_contract planned_evidence_storage_survives_observation_overflow
    cargo check -p conduit-core --target thumbv6m-none-eabi

check-form-s3:
    cargo test -p conduit-form lossless_document_
    cargo test -p conduit-form missing_close_is_diagnosed_at_eof_without_losing_source
    cargo test -p conduit-form source_checked_and_expanded_form_identities_stay_distinct
    cargo test -p conduit-form checked_export_is_the_only_source_of_a_parent_kind_boundary
    cargo test -p conduit-form duplicate_export_capabilities_are_rejected
    cargo test -p conduit-form inline_nested_form_uses_the_same_checked_boundary_as_a_standalone_form
    cargo test -p conduit-form nested_errors_keep_the_outer_document_and_exact_inner_span
    cargo test -p conduit-form inline_nesting_has_a_hard_depth_ceiling
    cargo test -p conduit-composite authored_parent_consumes_derived_export_through_an_ordinary_planned_cord
    cargo test -p conduit-core execution_identity_chain_keeps_plan_play_evidence_and_presentation_distinct
    cargo test -p conduit-runtime --test host_contract fake_adapter_failure_is_structured_and_terminal
    cargo test -p conduit-observatory report_separates_identity_capability_plan_connection_and_evidence_tables

check-browser-s4:
    cargo build -p conduit-browser-runtime --target wasm32-unknown-unknown --release
    npm run test:browser-host

check-realm-readiness:
    cargo test -p conduit-realm
    cargo check -p conduit-realm --target thumbv6m-none-eabi

check-observatory-readiness:
    cargo test -p conduit-observatory
    cargo check -p conduit-observatory --target thumbv6m-none-eabi

check-std-catalog-readiness:
    cargo test -p conduit-std-catalog
    cargo check -p conduit-std-catalog --no-default-features --target thumbv6m-none-eabi

check-sim-readiness:
    @if cargo tree -p conduit-runtime --edges normal --prefix none | rg -q '^conduit-signal '; then echo 'conduit-runtime must not depend on conduit-signal'; exit 1; fi
    cargo check -p conduit-signal --no-default-features
    cargo check -p conduit-wire --no-default-features
    cargo test -p conduit-wire
    cargo test -p conduit-runtime --test host_contract
    cargo test -p conduit-browser-sim
    cargo test -p conduit-browser-sim triple_signal_form_fans_out_to_std_and_simulated_receipts
    cargo check -p conduit-browser-sim --target wasm32-unknown-unknown
    cargo test -p conduit-pico-sim
    cargo test -p conduit-pico-sim std_host_sends_signal_to_pico_through_bounded_datagram_fixture
    cargo check -p conduit-pico-sim --no-default-features --target thumbv6m-none-eabi
