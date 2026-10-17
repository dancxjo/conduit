from pathlib import Path
import hashlib
import json
import statistics
import argparse

parser = argparse.ArgumentParser(description="Verify raw source-pinned quiescence before/after captures")
parser.add_argument("--before", type=Path, required=True)
parser.add_argument("--after", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
assert not args.output.exists(), "preserve previous proof outputs"
captures = {}
for label in ["before", "after"]:
    folder = getattr(args, label).resolve()
    status = json.loads((folder / "status.json").read_text())
    assert status["success"] and len(status["artifacts_sha256"]) == 18
    for name, digest in status["artifacts_sha256"].items():
        assert hashlib.sha256((folder / name).read_bytes()).hexdigest() == digest
    metadata = json.loads((folder / "metadata.json").read_text())
    assert metadata["source"] == status["source"]
    rows = json.loads((folder / "raw.json").read_text())
    assert len(rows) == 144
    expected = {(process, active, sparse, trial) for process in range(3)
                for active in [1, 8, 32] for sparse in [False, True] for trial in range(8)}
    assert {(row["process"], row["scan"]["active_nodes"], row["scan"]["sparse"], row["scan"]["trial"])
            for row in rows} == expected
    replays = []
    for process in range(3):
        replay = json.loads((folder / f"process-{process}.replays.json").read_text())
        assert len(replay) == 48
        replays.extend({"process": process, **row} for row in replay)
        assert "Exit status: 0" in (folder / f"process-{process}.resource.log").read_text()
    captures[label] = {"metadata": metadata, "raw": rows, "replays": replays,
                       "artifact_hashes": status["artifacts_sha256"]}
before, after = captures["before"], captures["after"]
assert before["metadata"]["source"] != after["metadata"]["source"]
for key in ["profile", "rustc", "cpu_model", "machine", "build_command"]:
    assert before["metadata"][key] == after["metadata"][key]
for old, new in zip(before["raw"], after["raw"]):
    assert old["process"] == new["process"]
    a, b = old["scan"], new["scan"]
    assert a["cords_scanned"] == a["active_cords"] * (a["ready_inputs"] + 1)
    assert b["cords_scanned"] == 0
    assert {k: v for k, v in a.items() if k not in ["cords_scanned", "prepare_ns", "step_ns"]} == {
        k: v for k, v in b.items() if k not in ["cords_scanned", "prepare_ns", "step_ns"]}
    assert old["overhead"] == new["overhead"]
    assert a["steps"] == b["steps"] == 2 * a["ready_inputs"]
    for key in ["ingress_allocations", "ingress_reallocations", "step_allocations",
                "step_reallocations", "step_value_store_copy_bytes"]:
        assert old["overhead"][key] == new["overhead"][key] == 0
for old, new in zip(before["replays"], after["replays"]):
    assert old["capacity"] == old["active"] == new["capacity"] == new["active"]
    assert {k: v for k, v in old.items() if k != "kernel_bytes"} == {
        k: v for k, v in new.items() if k != "kernel_bytes"}
summary = []
for active in [1, 8, 32]:
    for sparse in [False, True]:
        item = {"active": active, "capacity": active, "sparse": sparse}
        for label, capture in captures.items():
            matching = [row for row in capture["raw"]
                        if row["scan"]["active_nodes"] == active and row["scan"]["sparse"] == sparse]
            item[label] = {}
            for phase in ["prepare_ns", "step_ns"]:
                means = [statistics.mean(row["scan"][phase] for row in matching if row["process"] == process)
                         for process in range(3)]
                item[label][phase] = {"process_means": means, "mean": statistics.mean(means),
                                     "process_mean_sample_sd": statistics.stdev(means)}
            replay = next(row for row in capture["replays"] if row["active"] == active and row["sparse"] == sparse)
            item[label]["instrumented_scheduler_bytes"] = replay["kernel_bytes"]
            item[label]["cord_probes"] = matching[0]["scan"]["cords_scanned"]
        summary.append(item)
proof = {"before_source": before["metadata"]["source"], "after_source": after["metadata"]["source"],
         "verified_artifact_hashes": 36, "cases_per_side": 144,
         "all_replayed_signs_remote_lifecycle_outputs_readiness_and_values_identical": True,
         "all_other_work_and_allocation_counters_identical": True,
         "payload_only_quiescence_probes_after": 0, "summary": summary, "captures": captures,
         "limits": ["instrumented native scheduler fixture, not whole hosted Play or FARGAN",
                    "observed wall times include counters/harness and shared-host variation; no calibrated threshold",
                    "object bytes include cfg(test) counters; production object footprint requires separate proof",
                    "graphs containing quiescence connections retain their scan path; no speedup claimed for them",
                    "parent5231 retains original large-graph, fairness/transition and production benchmark acceptance"]}
output = args.output.resolve()
output.write_text(json.dumps(proof, indent=2) + "\n")
for row in summary:
    print(json.dumps(row))
print(output)
