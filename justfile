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
    @if rg -i 'playwright' -g 'Cargo.toml' -g 'package.json' -g 'package-lock.json' .; then echo 'Playwright dependency is forbidden before browser host work'; exit 1; fi
