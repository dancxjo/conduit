#!/usr/bin/env bash
set -euo pipefail
cd /home/dancxjo/conduit-4907-scorer
unset CONDUIT_PARSER_NATIVE_LIMIT
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export CARGO_TARGET_DIR=/home/dancxjo/conduit-4907-scorer/target
export CONDUIT_PARSER_NATIVE_METRICS_OUTPUT=/home/dancxjo/conduit-4907-scorer/work/native_metrics.json
cargo +stable test \
  --config 'profile.test.package.conduit-core.opt-level=2' \
  --config 'profile.test.package.conduit-plot.opt-level=2' \
  --config 'profile.test.package.conduit-kernel.opt-level=2' \
  --config 'profile.test.package.conduit-planner.opt-level=2' \
  --config 'profile.test.package.conduit-composite.opt-level=2' \
  --config 'profile.test.package.conduitos.opt-level=2' \
  --config 'profile.test.package.conduit-plan-lowering.opt-level=2' \
  --config 'profile.test.package.conduit-language.opt-level=1' \
  -p conduit-language --test parser_learned -- --ignored --nocapture --test-threads=1 \
  2>&1 | tee work/proof/native-heldout.log
