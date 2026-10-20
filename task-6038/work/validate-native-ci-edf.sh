#!/bin/bash
set -uo pipefail
cd /home/dancxjo/Documents/Codex/2026-10-19/goal-6038/work/native-frame-tests
export CARGO_TARGET_DIR=/home/dancxjo/Documents/Codex/2026-10-19/goal-6038/work/owner-installed-release-target
export CARGO_BUILD_JOBS=4 CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=true CARGO_PROFILE_DEV_OVERFLOW_CHECKS=true CARGO_PROFILE_DEV_STRIP=debuginfo CARGO_PROFILE_DEV_INCREMENTAL=false
export CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true CARGO_PROFILE_TEST_STRIP=debuginfo CARGO_PROFILE_TEST_INCREMENTAL=false
out=/home/dancxjo/Documents/Codex/2026-10-19/goal-6038/outputs/native-ci-coverage-edf79d210
run() { local name=$1; shift; printf '%q ' "$@" > "$out/$name.command"; printf '\n' >> "$out/$name.command"; "$@" > "$out/$name.log" 2>&1; local result=$?; echo "$name $result" | tee -a "$out/status.txt"; return "$result"; }
git rev-parse HEAD > "$out/source.txt"
run native-integration cargo +stable test -p conduitos --locked --features hosted-tools,native-compositor --test native_compositor --test native_compositor_input --test native_compositor_scanout -- --test-threads=1 || exit $?
run protocol-shards cargo +stable test -p xtask --bin xtask --locked host_protocol_shards -- --test-threads=1 || exit $?
run workspace-shards cargo +stable test -p xtask --bin xtask --locked workspace_shards -- --test-threads=1 || exit $?
echo 'Stopped after focused tests at root request; formatting repair pending.'
exit 0
