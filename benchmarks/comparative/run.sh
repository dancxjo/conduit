#!/usr/bin/env bash
set -euo pipefail

workspace_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
output_dir=${1:-"$workspace_root/target/comparative-benchmark"}
javascript_runtime="$output_dir.runtime/javascript"
manifest="$workspace_root/benchmarks/comparative/manifest.json"
regression_policy="$workspace_root/benchmarks/comparative/regression-policy.json"
raw="$output_dir/raw.ndjson"
summary="$output_dir/summary.json"
metadata="$output_dir/metadata.json"
report="$output_dir/report.md"
regression_evaluation="$output_dir/regressions.json"
regression_report="$output_dir/regression-report.md"
commands="$output_dir/commands.txt"
machine_class=${CONDUIT_BENCHMARK_MACHINE_CLASS:-local-unclassified}
values=${CONDUIT_BENCHMARK_VALUES:-$(jq -r .values "$manifest")}
warmups=${CONDUIT_BENCHMARK_WARMUPS:-$(jq -r .warmup_trials "$manifest")}
trials=${CONDUIT_BENCHMARK_TRIALS:-$(jq -r .measured_trials "$manifest")}
stride=$(jq -r .latency_sample_stride "$manifest")
queue=$(jq -r .queue_capacity_items "$manifest")
overload_slow_yields=$(jq -r .overload.slow_consumer_yields "$manifest")
fanout_slow_yields=$(jq -r .fanout.slow_consumer_yields "$manifest")
cancellation_capacity=$(jq -r .cancellation.queue_capacity_items "$manifest")
cancellation_pressure=$(jq -r .cancellation.pressure_policy "$manifest")
cancellation_after=$(jq -r .cancellation.cancel_after_offers "$manifest")
bursty_capacity=$(jq -r .bursty_consumers.queue_capacity_items "$manifest")
bursty_items=$(jq -r .bursty_consumers.consumer_burst_items "$manifest")
bursty_pause_yields=$(jq -r .bursty_consumers.consumer_pause_yields "$manifest")
persistent_capacity=$(jq -r .persistent_sessions.queue_capacity_items "$manifest")
persistent_quantum=$(jq -r .persistent_sessions.session_pump_quantum "$manifest")
persistent_wakes=$(jq -r .persistent_wake_residency.host_wakes "$manifest")
persistent_wake_plateau=$(jq -r .persistent_wake_residency.residency_plateau_after_wakes "$manifest")
persistent_wake_capacity=$(jq -r .persistent_wake_residency.queue_capacity_items "$manifest")
persistent_wake_quantum=$(jq -r .persistent_wake_residency.session_pump_quantum "$manifest")
persistent_timer_wakes=$(jq -r .persistent_timer_residency.timer_wakes "$manifest")
persistent_timer_plateau=$(jq -r .persistent_timer_residency.residency_plateau_after_wakes "$manifest")
persistent_timer_capacity=$(jq -r .persistent_timer_residency.queue_capacity_items "$manifest")
persistent_timer_quantum=$(jq -r .persistent_timer_residency.session_pump_quantum "$manifest")
persistent_timer_advance_ticks=$(jq -r .persistent_timer_residency.timer_advance_ticks "$manifest")
shared_payload_capacity=$(jq -r .shared_payload_fanout.queue_capacity_items "$manifest")
shared_watch_preview_bytes=$(jq -r .shared_payload_fanout.watch_preview_bytes "$manifest")
copy_payload_capacity=$(jq -r .copy_required_payload_fanout.queue_capacity_items "$manifest")

mkdir -p "$output_dir"
for artifact in "$raw" "$summary" "$metadata" "$report" "$regression_evaluation" "$regression_report" "$commands"; do
  if [[ -e "$artifact" ]]; then
    echo "refusing to overwrite existing benchmark artifact: $artifact" >&2
    exit 1
  fi
done

record_raw() {
  printf '%q ' "$@" >> "$commands"
  printf '\n' >> "$commands"
  "$@" >> "$raw"
}

cargo build --release -p conduit-benchmark --manifest-path "$workspace_root/Cargo.toml"
mkdir -p "$javascript_runtime"
cp "$workspace_root/benchmarks/comparative/javascript/package.json" "$javascript_runtime/package.json"
cp "$workspace_root/benchmarks/comparative/javascript/package-lock.json" "$javascript_runtime/package-lock.json"
cp "$workspace_root/benchmarks/comparative/javascript/run.mjs" "$javascript_runtime/run.mjs"
npm ci --ignore-scripts --prefix "$javascript_runtime"
bash "$workspace_root/benchmarks/comparative/reactor/fetch-dependencies.sh" "$output_dir/dependencies"
mkdir -p "$output_dir/classes"
javac \
  -cp "$output_dir/dependencies/*" \
  -d "$output_dir/classes" \
  "$workspace_root/benchmarks/comparative/reactor/ComparativeBenchmark.java"

fixture_sha256=$(sha256sum "$manifest" | cut -d' ' -f1)
commit=$(git -C "$workspace_root" rev-parse HEAD)
worktree_status=$(git -C "$workspace_root" status --porcelain=v1)
cpu=$(sed -n 's/^model name[[:space:]]*: //p' /proc/cpuinfo 2>/dev/null | head -n 1 || true)
jq -n \
  --arg commit "$commit" \
  --arg workspace_root "$workspace_root" \
  --arg worktree_status "$worktree_status" \
  --arg fixture_sha256 "$fixture_sha256" \
  --arg machine "$(uname -m)" \
  --arg machine_class "$machine_class" \
  --arg kernel "$(uname -sr)" \
  --arg cpu "${cpu:-unknown}" \
  --arg rustc "$(rustc -Vv)" \
  --arg node "$(node --version)" \
  --arg java "$(java -version 2>&1 | head -n 1)" \
  --arg rxjs_sha256 "$(sha256sum "$workspace_root/benchmarks/comparative/javascript/package-lock.json" | cut -d' ' -f1)" \
  --arg reactor_sha256 "$(sha256sum "$output_dir/dependencies/reactor-core-3.8.6.jar" | cut -d' ' -f1)" \
  --arg reactive_streams_sha256 "$(sha256sum "$output_dir/dependencies/reactive-streams-1.0.4.jar" | cut -d' ' -f1)" \
  --arg runner_sha256 "$(sha256sum "$workspace_root/benchmarks/comparative/run.sh" | cut -d' ' -f1)" \
  --arg conduit_binary_sha256 "$(sha256sum "$workspace_root/target/release/conduit-benchmark" | cut -d' ' -f1)" \
  --arg javascript_runner_sha256 "$(sha256sum "$workspace_root/benchmarks/comparative/javascript/run.mjs" | cut -d' ' -f1)" \
  --arg java_runner_sha256 "$(sha256sum "$workspace_root/benchmarks/comparative/reactor/ComparativeBenchmark.java" | cut -d' ' -f1)" \
  --argjson values "$values" \
  --argjson warmups "$warmups" \
  --argjson trials "$trials" \
  '{schema:"conduit.comparative-benchmark-metadata",schema_version:0,commit:$commit,workspace_root:$workspace_root,worktree:{clean:($worktree_status == ""),status:$worktree_status},fixture_sha256:$fixture_sha256,runner_sha256:$runner_sha256,binaries:{conduit_benchmark_sha256:$conduit_binary_sha256,javascript_runner_sha256:$javascript_runner_sha256,java_runner_sha256:$java_runner_sha256},machine:$machine,execution_environment:{machine_class:$machine_class},kernel:$kernel,cpu:$cpu,toolchains:{rustc:$rustc,node:$node,java:$java},dependencies:{rxjs_lock_sha256:$rxjs_sha256,reactor_core_sha256:$reactor_sha256,reactive_streams_sha256:$reactive_streams_sha256},run:{values:$values,warmup_trials:$warmups,measured_trials:$trials,exact_commands:"commands.txt"}}' \
  > "$metadata"
cp "$manifest" "$output_dir/manifest.json"
cp "$workspace_root/benchmarks/comparative/raw-sample.schema.json" "$output_dir/raw-sample.schema.json"
cp "$regression_policy" "$output_dir/regression-policy.json"

for workload in $(jq -r '.workloads[]' "$manifest"); do
  for operators in $(jq -r '.operator_depths[]' "$manifest"); do
    common=(--workload "$workload" --operators "$operators" --values "$values" --queue-items "$queue" --latency-sample-stride "$stride" --warmup-trials "$warmups" --measured-trials "$trials")
    if [[ "$workload" == "bounded-async" ]]; then
      record_raw java -cp "$output_dir/classes:$output_dir/dependencies/*" ComparativeBenchmark "${common[@]}"
    else
      record_raw "$workspace_root/target/release/conduit-benchmark" "${common[@]}"
      record_raw node "$javascript_runtime/run.mjs" "${common[@]}"
      record_raw java -cp "$output_dir/classes:$output_dir/dependencies/*" ComparativeBenchmark "${common[@]}"
      record_raw "$workspace_root/target/release/conduit-benchmark" "${common[@]}" --identity-loop
      record_raw node "$javascript_runtime/run.mjs" "${common[@]}" --identity-loop
      record_raw java -cp "$output_dir/classes:$output_dir/dependencies/*" ComparativeBenchmark "${common[@]}" --identity-loop
    fi
  done
done

for capacity in $(jq -r '.overload.queue_capacity_items[]' "$manifest"); do
  for pressure in $(jq -r '.overload.pressure_policies[]' "$manifest"); do
    record_raw "$workspace_root/target/release/conduit-benchmark" \
      --workload overload \
      --operators 1 \
      --values "$values" \
      --queue-items "$capacity" \
      --latency-sample-stride "$stride" \
      --warmup-trials "$warmups" \
      --measured-trials "$trials" \
      --pressure-policy "$pressure" \
      --slow-consumer-yields "$overload_slow_yields"
  done
done

for stop in $(jq -r '.cancellation.stop_policies[]' "$manifest"); do
  record_raw "$workspace_root/target/release/conduit-benchmark" \
    --workload overload \
    --operators 1 \
    --values "$values" \
    --queue-items "$cancellation_capacity" \
    --latency-sample-stride "$stride" \
    --warmup-trials "$warmups" \
    --measured-trials "$trials" \
    --pressure-policy "$cancellation_pressure" \
    --slow-consumer-yields "$overload_slow_yields" \
    --termination-request "$stop" \
    --cancel-after-offers "$cancellation_after"
done

for pressure in $(jq -r '.persistent_sessions.pressure_policies[]' "$manifest"); do
  for stop in $(jq -r '.persistent_sessions.stop_policies[]' "$manifest"); do
    record_raw "$workspace_root/target/release/conduit-benchmark" \
      --workload overload \
      --operators 1 \
      --values "$values" \
      --queue-items "$persistent_capacity" \
      --latency-sample-stride "$stride" \
      --warmup-trials "$warmups" \
      --measured-trials "$trials" \
      --pressure-policy "$pressure" \
      --slow-consumer-yields "$overload_slow_yields" \
      --termination-request "$stop" \
      --cancel-after-offers "$values" \
      --session-mode persistent \
      --session-pump-quantum "$persistent_quantum"
  done
done

record_raw "$workspace_root/target/release/conduit-benchmark" \
  --workload persistent-wake \
  --operators 1 \
  --values "$persistent_wakes" \
  --queue-items "$persistent_wake_capacity" \
  --latency-sample-stride "$stride" \
  --warmup-trials "$warmups" \
  --measured-trials "$trials" \
  --slow-consumer-yields 0 \
  --termination-request drain \
  --cancel-after-offers "$persistent_wakes" \
  --session-mode persistent \
  --session-pump-quantum "$persistent_wake_quantum" \
  --residency-plateau-after-wakes "$persistent_wake_plateau"

record_raw "$workspace_root/target/release/conduit-benchmark" \
  --workload persistent-timer \
  --operators 1 \
  --values "$persistent_timer_wakes" \
  --queue-items "$persistent_timer_capacity" \
  --latency-sample-stride "$stride" \
  --warmup-trials "$warmups" \
  --measured-trials "$trials" \
  --slow-consumer-yields 0 \
  --termination-request drain \
  --cancel-after-offers "$persistent_timer_wakes" \
  --session-mode persistent \
  --session-pump-quantum "$persistent_timer_quantum" \
  --timer-advance-ticks "$persistent_timer_advance_ticks" \
  --residency-plateau-after-wakes "$persistent_timer_plateau"

for pressure in $(jq -r '.bursty_consumers.pressure_policies[]' "$manifest"); do
  record_raw "$workspace_root/target/release/conduit-benchmark" \
    --workload overload \
    --operators 1 \
    --values "$values" \
    --queue-items "$bursty_capacity" \
    --latency-sample-stride "$stride" \
    --warmup-trials "$warmups" \
    --measured-trials "$trials" \
    --pressure-policy "$pressure" \
    --slow-consumer-yields "$bursty_pause_yields" \
    --consumer-pattern bursty \
    --consumer-burst-items "$bursty_items"
done

for branches in $(jq -r '.bursty_consumers.fanout_branches[]' "$manifest"); do
  for mode in $(jq -r '.bursty_consumers.fanout_modes[]' "$manifest"); do
    for slow in $(jq -r '.bursty_consumers.fanout_slow_branches[]' "$manifest"); do
      record_raw "$workspace_root/target/release/conduit-benchmark" \
        --workload fanout \
        --operators 1 \
        --values "$values" \
        --queue-items "$bursty_capacity" \
        --latency-sample-stride "$stride" \
        --warmup-trials "$warmups" \
        --measured-trials "$trials" \
        --fanout-branches "$branches" \
        --fanout-mode "$mode" \
        --slow-branches "$slow" \
        --slow-consumer-yields "$bursty_pause_yields" \
        --consumer-pattern bursty \
        --consumer-burst-items "$bursty_items"
    done
  done
done

for capacity in $(jq -r '.fanout.queue_capacity_items[]' "$manifest"); do
  for branches in $(jq -r '.fanout.branches[]' "$manifest"); do
    for mode in $(jq -r '.fanout.modes[]' "$manifest"); do
      for slow in $(jq -r '.fanout.slow_branches[]' "$manifest"); do
        record_raw "$workspace_root/target/release/conduit-benchmark" \
          --workload fanout \
          --operators 1 \
          --values "$values" \
          --queue-items "$capacity" \
          --latency-sample-stride "$stride" \
          --warmup-trials "$warmups" \
          --measured-trials "$trials" \
          --fanout-branches "$branches" \
          --fanout-mode "$mode" \
          --slow-branches "$slow" \
          --slow-consumer-yields "$fanout_slow_yields"
      done
    done
  done
done

for shared_payload_bytes in $(jq -r '.shared_payload_fanout.payload_bytes[]' "$manifest"); do
  for branches in $(jq -r '.shared_payload_fanout.branches[]' "$manifest"); do
    for termination in $(jq -r '.shared_payload_fanout.termination_requests[]' "$manifest"); do
      for watch_mode in $(jq -r '.shared_payload_fanout.watch_modes[]' "$manifest"); do
        case "$watch_mode" in
          none)
            watch_slots=0
            watch_preview_bytes=0
            ;;
          one)
            watch_slots=1
            watch_preview_bytes=$shared_watch_preview_bytes
            ;;
          every-branch)
            watch_slots=$branches
            watch_preview_bytes=$shared_watch_preview_bytes
            ;;
          *)
            echo "unsupported shared-payload Watch mode: $watch_mode" >&2
            exit 1
            ;;
        esac
        record_raw "$workspace_root/target/release/conduit-benchmark" \
          --workload shared-payload-fanout \
          --operators 1 \
          --values 1 \
          --queue-items "$shared_payload_capacity" \
          --latency-sample-stride 1 \
          --warmup-trials "$warmups" \
          --measured-trials "$trials" \
          --fanout-branches "$branches" \
          --fanout-mode coupled \
          --slow-consumer-yields 0 \
          --termination-request "$termination" \
          --payload-bytes "$shared_payload_bytes" \
          --watch-slots "$watch_slots" \
          --watch-preview-bytes "$watch_preview_bytes"
      done
    done
  done
done

for copy_payload_bytes in $(jq -r '.copy_required_payload_fanout.payload_bytes[]' "$manifest"); do
  for branches in $(jq -r '.copy_required_payload_fanout.branches[]' "$manifest"); do
    record_raw "$workspace_root/target/release/conduit-benchmark" \
      --workload shared-payload-fanout \
      --operators 1 \
      --values 1 \
      --queue-items "$copy_payload_capacity" \
      --latency-sample-stride 1 \
      --warmup-trials "$warmups" \
      --measured-trials "$trials" \
      --fanout-branches "$branches" \
      --fanout-mode coupled \
      --slow-consumer-yields 0 \
      --termination-request complete \
      --payload-bytes "$copy_payload_bytes" \
      --payload-binding branch-local-uppercase-copy \
      --watch-slots 0 \
      --watch-preview-bytes 0
  done
done

node "$workspace_root/benchmarks/comparative/summarize.mjs" "$raw" "$summary" "$report"
node "$workspace_root/benchmarks/comparative/evaluate-regressions.mjs" \
  "$metadata" \
  "$summary" \
  "$output_dir/regression-policy.json" \
  "$regression_evaluation" \
  "$regression_report"
printf 'comparative benchmark artifacts: %s\n' "$output_dir"
