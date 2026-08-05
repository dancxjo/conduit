use crate::process::Step;

/// `check-kernel-s1` suite steps.
pub static KERNEL_S1: &[Step] = &[
    Step::new(
        "kernel-s1-test-alloc",
        "Test conduit-kernel with alloc feature",
        "cargo",
        &["test", "-p", "conduit-kernel", "--features", "alloc"],
    ),
    Step::new(
        "kernel-s1-check-thumb",
        "Check conduit-kernel for thumbv6m-none-eabi",
        "cargo",
        &["check", "-p", "conduit-kernel", "--target", "thumbv6m-none-eabi"],
    ),
];

/// `check-kernel-takeover` suite steps.
pub static KERNEL_TAKEOVER: &[Step] = &[
    Step::new(
        "kernel-takeover-check-std-host-no-default",
        "Check conduit-std-host with no default features",
        "cargo",
        &["check", "-p", "conduit-std-host", "--no-default-features"],
    ),
    Step::new(
        "kernel-takeover-exact-signal-fragment",
        "Test exact_signal_fragment_lowers_to_numeric_kernel_tables",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "exact_signal_fragment_lowers_to_numeric_kernel_tables",
        ],
    ),
    Step::new(
        "kernel-takeover-streamed-output",
        "Test streamed_output_uses_a_virtual_clock_and_retains_terminal_evidence",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "streamed_output_uses_a_virtual_clock_and_retains_terminal_evidence",
        ],
    ),
    Step::new(
        "kernel-takeover-three-sink-fanout",
        "Test local_three_sink_signal_fanout_uses_only_the_sealed_kernel_profile",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "local_three_sink_signal_fanout_uses_only_the_sealed_kernel_profile",
        ],
    ),
    Step::new(
        "kernel-takeover-unsupported-form",
        "Test unsupported_production_std_form_fails_closed_without_a_legacy_pump",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "unsupported_production_std_form_fails_closed_without_a_legacy_pump",
        ],
    ),
    Step::new(
        "kernel-takeover-multivalue",
        "Test kernel_multivalue",
        "cargo",
        &["test", "-p", "conduit-std-host", "kernel_multivalue"],
    ),
    Step::new(
        "kernel-takeover-preparation",
        "Test kernel_preparation",
        "cargo",
        &["test", "-p", "conduit-std-host", "kernel_preparation"],
    ),
    Step::new(
        "kernel-takeover-browser-readiness-drawbridge",
        "Test dependency_and_profile_installation_drawbridge_is_explicit",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-runtime",
            "--test",
            "browser_readiness",
            "dependency_and_profile_installation_drawbridge_is_explicit",
        ],
    ),
    Step::new(
        "kernel-takeover-typed-multi-value",
        "Test typed_multi_value_form_runs_through_the_std_kernel",
        "cargo",
        &["test", "-p", "conduit", "--test", "hello", "typed_multi_value_form_runs_through_the_std_kernel"],
    ),
    Step::new(
        "kernel-takeover-admitted-sink-no-payload",
        "Test admitted_sink_host_operation_may_have_no_output_payload",
        "cargo",
        &["test", "-p", "conduit-kernel", "admitted_sink_host_operation_may_have_no_output_payload"],
    ),
    Step::new(
        "kernel-takeover-kernel-thumb",
        "Check conduit-kernel for thumbv6m-none-eabi",
        "cargo",
        &["check", "-p", "conduit-kernel", "--target", "thumbv6m-none-eabi"],
    ),
    Step::new(
        "kernel-takeover-core-thumb",
        "Check conduit-core for thumbv6m-none-eabi",
        "cargo",
        &["check", "-p", "conduit-core", "--target", "thumbv6m-none-eabi"],
    ),
];
