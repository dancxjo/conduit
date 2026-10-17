#!/usr/bin/env python3
"""Read-only verification of retained ordinary runtime-performance fusion rows."""
import hashlib
import json
from pathlib import Path
import statistics
import sys

root = Path(sys.argv[1])
rows = [json.loads(line) for line in (root / "raw.jsonl").read_text().splitlines()]
rows = [r for r in rows if r.get("phase") == "finite-ordinary-fusion-pipeline"]
assert rows, "no measured ordinary fused pipeline rows"
expected = hashlib.sha256(b"".join((i + 32).to_bytes(8, "little") for i in range(64))).hexdigest()
groups = {}
for row in rows:
    key = (row["gears"], row["repetition"])
    modes = groups.setdefault(key, {})
    assert row["mode"] not in modes, "duplicate trial"
    modes[row["mode"]] = row
    assert row["values"] == 64
    assert row["ordered_output_sha256"] == expected
    assert row["logical_gear_count"] == row["gears"]
    assert row["logical_internal_cord_count"] == row["gears"] - 1
    assert row["mandatory_output_sign_count"] == 64
    assert row["value_capacity_before"] == row["value_capacity_after"]
    assert row["work_accounting"]["intermediate_materializations"] == (row["gears"] - 1) * 64
    assert row["work_accounting"]["intermediate_canonical_bytes"] == (row["gears"] - 1) * 64 * 8
    assert row["work_accounting"]["internal_copy_counter"] is None
    assert row["physical_regions"] == (1 if row["mode"] == "fused" and row["gears"] > 1 else 0)
    assert row["full_host_call"]["wall_ns"] > 0
    assert row["completion_window_useful_outputs_per_second"] > 0
for (gears, repetition), modes in groups.items():
    assert set(modes) == {"local-unfused", "fused"}
    a, b = modes["local-unfused"], modes["fused"]
    assert a["source_sha256"] == b["source_sha256"]
    assert a["checked_plot_id"] == b["checked_plot_id"]
    assert a["value_capacity_before"] == b["value_capacity_before"]
    if gears > 1:
        assert b["kernel_decisions"] < a["kernel_decisions"]
    plans = []
    for mode in ["local-unfused", "fused"]:
        plan = json.loads((root / "sources" / f"pure-chain-{gears}-{mode}-{repetition}-plan.json").read_text())
        fragment = plan["fragments"][0]
        logical = {}
        for gear in fragment["placements"]:
            logical[gear["gear_id"]] = {k: gear.get(k) for k in ["kind_id", "kind_contract_revision", "inputs", "outputs", "configuration", "semantic_contract", "source_span"]}
        ids = {p["placement_id"]: p["gear_id"] for p in fragment["placements"]}
        cords = sorted(json.dumps({**{k: cord.get(k) for k in ["source_port_id", "sink_port_id", "value_kind", "track", "temporal", "item_capacity", "byte_capacity", "pressure_policy"]}, "source_gear": ids[cord["source_placement_id"]], "sink_gear": ids[cord["sink_placement_id"]]}, sort_keys=True) for cord in fragment["connections"])
        plans.append((logical, cords))
    assert plans[0] == plans[1], "logical graph changed under physical fusion"
for gears in [1, 8, 32]:
    assert len([key for key in groups if key[0] == gears]) >= 3, "at least three matched repetitions required"
summary = {}
for gears in [1, 8, 32]:
    selected = [r for r in rows if r["gears"] == gears]
    summary[gears] = {mode: {
        "decisions": sorted({r["kernel_decisions"] for r in selected if r["mode"] == mode}),
        "full_call_wall_ns_median": statistics.median(r["full_host_call"]["wall_ns"] for r in selected if r["mode"] == mode),
        "full_call_cpu_ns_median": statistics.median(r["full_host_call"]["process_cpu_ns"] for r in selected if r["mode"] == mode and r["full_host_call"]["process_cpu_ns"] is not None),
        "completion_outputs_per_second_median": statistics.median(r["completion_window_useful_outputs_per_second"] for r in selected if r["mode"] == mode),
    } for mode in ["local-unfused", "fused"]}
print(json.dumps({"verified_pairs": len(groups), "profiles": summary}, indent=2))
