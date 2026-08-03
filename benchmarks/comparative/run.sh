#!/usr/bin/env bash
set -euo pipefail

workspace_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
output_dir=${1:-"$workspace_root/target/comparative-benchmark"}
javascript_runtime="$output_dir.runtime/javascript"
manifest="$workspace_root/benchmarks/comparative/manifest.json"
raw="$output_dir/raw.ndjson"
summary="$output_dir/summary.json"
metadata="$output_dir/metadata.json"
report="$output_dir/report.md"
commands="$output_dir/commands.txt"
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

mkdir -p "$output_dir"
for artifact in "$raw" "$summary" "$metadata" "$report" "$commands"; do
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
  '{schema:"conduit.comparative-benchmark-metadata",schema_version:0,commit:$commit,workspace_root:$workspace_root,worktree:{clean:($worktree_status == ""),status:$worktree_status},fixture_sha256:$fixture_sha256,runner_sha256:$runner_sha256,binaries:{conduit_benchmark_sha256:$conduit_binary_sha256,javascript_runner_sha256:$javascript_runner_sha256,java_runner_sha256:$java_runner_sha256},machine:$machine,kernel:$kernel,cpu:$cpu,toolchains:{rustc:$rustc,node:$node,java:$java},dependencies:{rxjs_lock_sha256:$rxjs_sha256,reactor_core_sha256:$reactor_sha256,reactive_streams_sha256:$reactive_streams_sha256},run:{values:$values,warmup_trials:$warmups,measured_trials:$trials,exact_commands:"commands.txt"}}' \
  > "$metadata"
cp "$manifest" "$output_dir/manifest.json"
cp "$workspace_root/benchmarks/comparative/raw-sample.schema.json" "$output_dir/raw-sample.schema.json"

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

node "$workspace_root/benchmarks/comparative/summarize.mjs" "$raw" "$summary" "$report"
printf 'comparative benchmark artifacts: %s\n' "$output_dir"
