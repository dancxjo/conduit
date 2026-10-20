# Readonly CI feature coverage audit

Audited clean repair69f488258; no builds or source edits. Genuine gap confirmed: the fullworkspace local run enabled ConduitOS hosted-tools,native-compositor,native-owner-network (recorded Cargo fingerprint), whereas delegated CI protocol tests request only package conduitos defaults.

## Existing paths

- .github/workflows/integration.yml delegates ci.yml exhaustive. ci.yml unit invokes `cargo xtask ci pipeline unit <shard>`. tools/xtask-dispatch/src/ci_dispatch/pipeline.rs runs package step/owned WORKSPACE_STEPS, then host_protocol_shards proof commands.
- tools/xtask/src/suites/host_protocol_shards.rs arguments() is `cargo test --locked --no-fail-fast -p conduitos`, with no features. Tests are split by automatic_body/automatic_clock; remaining library tests belong hosts-conduitos. validate_library_partition uses the identical default-only command, so cannot discover missing gated tests.
- workspace_shards.rs HOST_CONDUITOS_TEST_PACKAGES contains only conduit-conduitos-mask-offer. workspace_shard_features_tests.rs checks only packages extracted from package_test_step, so delegated conduitos is excluded despite its features being workspace-enabled.
- targets/conduitos/src/lib.rs gates native_owner_document and native_owner_admission on native-owner-network; native_owner_return additionally x86_64. All14 native_owner_ cases are absent from default library inventory.
- tools/xtask/src/suites/input_semantics_check.rs native-editor explicitly enables native-compositor but filters `text_editor`; native-navigation enables it but filters `native_face`. Neither filter includes native_owner_. Other native filters keyboard_bridge/keyboard_text_plan likewise do not. The InputSemantics suite is only selected by explicit CheckSuite::InputSemantics in commands/check.rs; it is not chained into WORKSPACE_STEPS or current pipeline targets. Adding a step there alone would not close CI gap.
- tools/ci/pipeline/targets.mjs conduitos targets build actual host product and verify boot/journey/product. These compile feature-enabled product but do not execute library unit tests. A passing real guest journey does not exercise truncated/tampered document fixtures.
- tools/ci/pipeline/admission.mjs narrow admission invokes selected package roots together. A conduitos-only delta gets default features, also misses14. Broader roots can accidentally unify features but that is not an explicit guaranteed gate.

## Smallest robust correction

Prefer restore delegated protocol proof feature context, not add a disconnected input-semantics step. Add explicit `--features hosted-tools,native-compositor` to host_protocol_shards arguments() (native-owner-network transitively enabled), retaining same four-way partition and current bounds. Then default library inventory validation discovers all gated cases and assigns native_owner_ to hosts-conduitos; this restores more than the14 discovered failures and matches actual fullworkspace semantics. Integration-target/default-shape checks can remain about integration targets/defaults; explain that library test feature context is now deliberate. Add a focused assertion that protocol arguments retain both features and extend feature-coverage test to audit delegated PACKAGE with these exact args against workspace features. Do not silently add duplicated conduitos root to HOST_CONDUITOS step: it would undermine partition/scheduling.

Alternative narrowly targeted14-test step must be wired into an already executed WORKSPACE_STEPS host ownership contract AND narrow native admission; adding to input_semantics_check only is insufficient. It would leave broader feature drift undetected, so less sound than restoring protocol feature context. Candidate narrow admission also needs explicit native-owner-network context for conduitos-selected roots, or an explicit14-test feature step when native risk is true; this is distinct from exhaustive protocol restoration.

Evidence is source registration, not fresh CI execution. Root owns implementation and validation; no green CI acceptance claim.
