#!/usr/bin/env python3
"""Read-only verification of retained ordinary runtime-performance fusion rows."""
import hashlib
import json
from pathlib import Path
import statistics
import sys


def verify(root):
    status = json.loads((root / "status.json").read_text())
    assert status["worker_success"] and status["conformance_success"]
    assert status["source_custody_verified"]
    assert status["git_sha"] == status["final_git_sha"]
    for name, digest in status["artifact_sha256"].items():
        path = root / name
        assert path.resolve().is_relative_to(root.resolve()), "artifact escapes capture"
        assert hashlib.sha256(path.read_bytes()).hexdigest() == digest, name
    metadata = json.loads((root / "metadata.json").read_text())
    assert metadata["git_sha"] == status["git_sha"]
    schema = metadata.get("hosted_native_chain_schema")
    assert schema in [None, "native-u64-with-canonical-fused-reference@1"]
    native_schema = schema is not None
    rows = [json.loads(line) for line in (root / "raw.jsonl").read_text().splitlines()]
    rows = [r for r in rows if r.get("phase") == "finite-ordinary-fusion-pipeline"]
    assert rows, "no measured ordinary fused pipeline rows"
    expected = hashlib.sha256(b"".join((i + 32).to_bytes(8, "little") for i in range(64))).hexdigest()
    groups = {}
    for row in rows:
        gears, mode = row["gears"], row["mode"]
        assert gears in [1, 8, 32]
        modes = groups.setdefault((gears, row["repetition"]), {})
        assert mode not in modes, "duplicate trial"
        modes[mode] = row
        native = mode == "native-fused"
        assert row["values"] == 64
        assert row["kernel_logical_invocations"] >= 64 * gears
        assert row["ordered_output_sha256"] == expected
        assert row["logical_gear_count"] == gears
        assert row["logical_internal_cord_count"] == gears - 1
        assert row["mandatory_output_sign_count"] == 64
        assert row["value_capacity_before"] == row["value_capacity_after"]
        work = row["work_accounting"]
        assert work["logical_evaluations"] == gears * 64
        assert work["intermediate_materializations"] == (0 if native else (gears - 1) * 64)
        assert work["intermediate_canonical_bytes"] == (0 if native else (gears - 1) * 64 * 8)
        if native_schema:
            assert work["native_word_transports"] == ((gears - 1) * 64 if native else 0)
        assert work["internal_copy_counter"] is None
        assert row["physical_regions"] == int(mode != "local-unfused" and gears > 1)
        assert row["full_host_call"]["wall_ns"] > 0
        assert row["completion_window_useful_outputs_per_second"] > 0
        if native_schema:
            assert len(row["kernel_phase_wall"]["allocator_intervals"]) == 4
            stem = f"pure-chain-{gears}-{mode}-{row['repetition']}"
            outputs = json.loads((root / "sources" / f"{stem}-outputs.json").read_text())
            assert len(outputs) == 64
            for index, output in enumerate(outputs):
                assert output["sequence"] == index and output["port"] == "output"
                assert output["kind"] == "value/u64"
                assert bytes(output["bytes"]) == (index + 32).to_bytes(8, "little")
        assert sum(s["kind"] == "RemoteValueDelivered" for s in row["signs"]) == 64
    assert 3 <= status["config"]["repetitions"] <= 15
    repetitions = set(range(status["config"]["repetitions"]))
    assert set(groups) == {(gears, r) for gears in [1, 8, 32] for r in repetitions}
    for (gears, repetition), modes in groups.items():
        expected_modes = {"local-unfused", "fused"}
        if gears > 1 and native_schema:
            expected_modes.add("native-fused")
        assert set(modes) == expected_modes, "missing or unexpected physical profile"
        baseline = modes["local-unfused"]
        plans = []
        lifecycles = []
        lifecycle_kinds = {"RemoteValueOffered", "RemoteValueAccepted", "RemoteValueDelivered",
                           "RemoteOutputClosed", "RemoteInputAdmitted", "RemoteInputClosed",
                           "InputClosed", "BackCompleted"}
        for mode, row in modes.items():
            for key in ["source_sha256", "checked_plot_id", "kernel_logical_invocations"]:
                assert row[key] == baseline[key], key
            if mode != "native-fused":
                assert row["value_capacity_before"] == baseline["value_capacity_before"]
            if gears > 1 and mode != "local-unfused":
                assert row["kernel_decisions"] < baseline["kernel_decisions"]
            stem = f"pure-chain-{gears}-{mode}-{repetition}"
            plan = json.loads((root / "sources" / f"{stem}-plan.json").read_text())
            host = json.loads((root / "sources" / f"{stem}-host.json").read_text())
            play = row["active_play"]
            assert play["plan_id"] == plan["plan_id"]
            assert play["host_id"] == host["host_id"] and play["boot_id"] == host["boot_id"]
            if native_schema:
                source = json.loads((root / "sources" / f"pure-chain-{gears}-{repetition}-source.json").read_text())
            else:
                source = (root / "sources" / f"hosted-pipeline-{gears}-gears-32-adds.plot").read_text()
            assert hashlib.sha256(source.encode()).hexdigest() == row["source_sha256"]
            fragment = plan["fragments"][0]
            logical = {g["gear_id"]: {k: g.get(k) for k in ["kind_id", "kind_contract_revision", "inputs", "outputs", "configuration", "semantic_contract", "source_span"]} for g in fragment["placements"]}
            ids = {p["placement_id"]: p["gear_id"] for p in fragment["placements"]}
            cords = sorted(json.dumps({**{k: c.get(k) for k in ["source_port_id", "sink_port_id", "value_kind", "track", "temporal", "item_capacity", "byte_capacity", "pressure_policy"]}, "source_gear": ids[c["source_placement_id"]], "sink_gear": ids[c["sink_placement_id"]]}, sort_keys=True) for c in fragment["connections"])
            plans.append((logical, cords))
            lifecycle = [{k: v for k, v in s.items() if k != "sequence"} for s in row["signs"] if s["kind"] in lifecycle_kinds]
            if native_schema:
                assert len(lifecycle) == 258 + 2 * gears
                lifecycles.append(lifecycle)
        assert all(p == plans[0] for p in plans), "logical graph changed under physical fusion"
        assert all(s == lifecycles[0] for s in lifecycles), "required lifecycle changed"
        if gears > 1 and native_schema:
            assert modes["native-fused"]["kernel_decisions"] == modes["fused"]["kernel_decisions"]
            assert modes["native-fused"]["signs"] == modes["fused"]["signs"], "native/canonical Sign changed"
    summary = {}
    for gears in [1, 8, 32]:
        selected = [r for r in rows if r["gears"] == gears]
        summary[gears] = {}
        for mode in sorted({r["mode"] for r in selected}):
            trials = [r for r in selected if r["mode"] == mode]
            median = lambda f: statistics.median(f(r) for r in trials)
            summary[gears][mode] = {
                "value_capacities": sorted({tuple(r["value_capacity_before"]) for r in trials}),
                "decisions": sorted({r["kernel_decisions"] for r in trials}),
                "logical_invocations": sorted({r["kernel_logical_invocations"] for r in trials}),
                "full_call_wall_ns_median": median(lambda r: r["full_host_call"]["wall_ns"]),
                "full_call_cpu_ns_median": median(lambda r: r["full_host_call"]["process_cpu_ns"]),
                "cold_plus_call_ns_median": median(lambda r: sum(p[1]["wall_ns"] for p in r["cold_phases"]) + r["full_host_call"]["wall_ns"]),
                "first_output_ns_median": median(lambda r: r["first_acknowledgement_from_call_start_ns"]),
                "completion_outputs_per_second_median": median(lambda r: r["completion_window_useful_outputs_per_second"]),
            }
    return {"source": status["git_sha"], "verified_groups": len(groups), "profiles": summary}


if __name__ == "__main__":
    print(json.dumps(verify(Path(sys.argv[1])), indent=2))
