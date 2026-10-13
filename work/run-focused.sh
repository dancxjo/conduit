#!/usr/bin/env bash
set -euo pipefail
cd /home/dancxjo/conduit-4907-scorer
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export CARGO_TARGET_DIR=/home/dancxjo/conduit-4907-scorer/target
export CONDUIT_PARSER_NATIVE_METRICS_OUTPUT=/home/dancxjo/conduit-4907-scorer/work/representative_metrics.json
scorer_profile_flags=(
  --config profile.test.package.conduit-core.opt-level=2
  --config profile.test.package.conduit-plot.opt-level=2
  --config profile.test.package.conduit-kernel.opt-level=2
  --config profile.test.package.conduit-planner.opt-level=2
  --config profile.test.package.conduit-composite.opt-level=2
  --config profile.test.package.conduitos.opt-level=2
  --config profile.test.package.conduit-plan-lowering.opt-level=2
  --config profile.test.package.conduit-language.opt-level=1
)
cargo +stable test "${scorer_profile_flags[@]}" -p conduit-ai --lib integer_masked_rank -- --nocapture
cargo +stable test "${scorer_profile_flags[@]}" -p conduit-language --test parser_learned --test parser_graph --test parser_beam --test parser_transition --test parser_admission --test parser_mask -- --nocapture --test-threads=1 2>&1 | tee work/proof/focused-tests.log
cargo +stable clippy "${scorer_profile_flags[@]}" -p conduit-ai -p conduit-std-host -p conduit-language --test parser_learned --test parser_graph --test parser_beam --test parser_transition --test parser_admission --test parser_mask -- -D warnings 2>&1 | tee work/proof/scoped-clippy.log
