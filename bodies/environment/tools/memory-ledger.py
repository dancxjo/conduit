"""Inventory existing profiles; never infer combined admission from their sums."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess


ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = ROOT / "bodies/environment/evidence"


def read_json(path):
    return json.loads(path.read_text())


def elf_sections(path):
    """Read section allocation extents, not allocator use or stack high water."""
    data = path.read_bytes()
    if data[:7] != b"\x7fELF\x01\x01\x01":
        raise ValueError("expected little-endian ELF32")
    if struct.unpack_from("<H", data, 18)[0] != 94:
        raise ValueError("address classification requires Xtensa ELF")
    offset = struct.unpack_from("<I", data, 32)[0]
    stride, count, names_index = struct.unpack_from("<HHH", data, 46)
    if stride != 40 or not 0 < names_index < count or offset + count * stride > len(data):
        raise ValueError("invalid ELF section table")
    sections = [struct.unpack_from("<10I", data, offset + i * stride) for i in range(count)]
    names = sections[names_index]
    strings = data[names[4]:names[4] + names[5]]
    result = []
    for name, kind, flags, address, _, size, *_ in sections:
        if not flags & 2 or not size:
            continue
        end = strings.find(b"\0", name)
        if end < name:
            raise ValueError("invalid ELF section name")
        flash = 0x3F400000 <= address < 0x3F800000 or 0x400D0000 <= address < 0x40400000
        result.append({
            "name": strings[name:end].decode("ascii"), "address": hex(address),
            "bytes": size, "writable": bool(flags & 1), "nobits": kind == 8,
            "flash_mapped": flash, "executable": bool(flags & 4),
        })
    return {
        "path": str(path), "sha256": hashlib.sha256(data).hexdigest(),
        "flash_mapped_bytes": sum(s["bytes"] for s in result if s["flash_mapped"]),
        "ram_section_extent_sum_bytes": sum(s["bytes"] for s in result if not s["flash_mapped"]),
        "ram_resident_code_extent_bytes": sum(s["bytes"] for s in result if not s["flash_mapped"] and s["executable"]),
        "sections": result,
        "accounting": "RAM section extents include static heap arenas and .stack reservations; do not add heap capacities or linked stacks again. This is not a bank/alias/lifetime proof or allocator/stack high-water measurement.",
    }


def ledger(sampling_image=None, output_image=None):
    inputs = {
        "sampling": EVIDENCE / "2026-10-24-constructor-tape-hardware/sampling.json",
        "output": EVIDENCE / "2026-10-24-constructor-tape-hardware/output.json",
        "mask_cpu": EVIDENCE / "2026-10-24-inline-frame-storage/measurement.json",
        "expression_cpu": EVIDENCE / "2026-10-24-tape-join-repair/measurement.json",
        "sign_budget": EVIDENCE / "2026-10-24-derived-output-signs/measurement.json",
    }
    sampling, output, mask = (read_json(inputs[k]) for k in ["sampling", "output", "mask_cpu"])
    expression_cpu = read_json(inputs["expression_cpu"])
    sign_budget = read_json(inputs["sign_budget"])
    for relative, digest in sign_budget["code_sha256"].items():
        if hashlib.sha256((ROOT / relative).read_bytes()).hexdigest() != digest:
            raise ValueError("output sign-bound code changed; renew finite-profile proof")
    storage = ROOT / "bodies/environment/runtime/src/appliance/value_storage.rs"
    source = storage.read_text()
    capacities = re.search(r"const CAPACITIES: \[u32; \d+\] = \[([^]]+)\]", source)
    if capacities is None:
        raise ValueError("sampling storage profile changed; renew ledger parser")
    capacities = [int(n.strip()) for n in capacities[1].split(",")]
    output_storage = ROOT / "targets/esp32/proof/tm1637/src/native_display_kernel.rs"
    shape = re.search(r"HostedValueStore::new\(\s*(\d+),\s*profile.maximum_value_bytes,\s*(\d+) \* profile.maximum_value_bytes", output_storage.read_text())
    linked_layout = ROOT / "targets/esp32/proof/tm1637/src/display_layout.rs"
    extent_match = re.search(r"pub const MAXIMUM_VALUE_BYTES: u32 = (\d+);", linked_layout.read_text())
    if shape is None or extent_match is None or "maximum_value_bytes:MAXIMUM_VALUE_BYTES" not in linked_layout.read_text():
        raise ValueError("output storage profile changed; renew ledger parser")
    items, quota_items = map(int, shape.groups())
    extent = int(extent_match[1])
    if items != quota_items or not 0 < extent <= 1024:
        raise ValueError("output backing and live quota differ; account separately")
    if "DisplaySignBudget::derive(maximum_frames,profile.sign_items,profile.sign_bytes)" not in re.sub(r"\s+", "", output_storage.read_text()):
        raise ValueError("output sign preparation changed; renew ledger parser")
    output_construction = ROOT / "targets/esp32/proof/tm1637/src/native_display.rs"
    frame_limit = re.search(r"pub const CONSTRUCTION_FRAMES: u32 = (\d+);", output_construction.read_text())
    minimum_items = re.search(r"pub const MINIMUM_SIGN_ITEMS: u16 = (\d+);", linked_layout.read_text())
    minimum_bytes = re.search(r"pub const MINIMUM_SIGN_BYTES: u32 = (\d+);", linked_layout.read_text())
    if frame_limit is None or minimum_items is None or minimum_bytes is None or (int(frame_limit[1]), int(minimum_items[1]), int(minimum_bytes[1])) != (sign_budget["maximum_input_frames"], sign_budget["minimum_sign_items"], sign_budget["minimum_sign_bytes"]):
        raise ValueError("output work/minimum sign contract changed; renew finite-profile proof")
    forms = ROOT / "plots/device-protocols/dht11.conduit"
    temperature = re.search(r"^form device/dht11-temperature = Temperature as i(\d+)le", forms.read_text(), re.M)
    humidity = re.search(r"^form device/dht11-humidity = Ratio as u(\d+)le", forms.read_text(), re.M)
    if temperature is None or humidity is None or (int(temperature[1]) + int(humidity[1])) % 8:
        raise ValueError("quantity Forms changed; renew representation inventory")
    result = {
        "schema": "conduit.environment/combined-memory-ledger@1",
        "source_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "source_dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT)),
        "combined_admitted": False,
        "captured_profiles": {
            "sampling_face": {
                "source": sampling["source_commit"], "root_capacity_bytes": sampling["root_heap_bytes"],
                "preparation_and_run_peak_bytes": sampling["root_heap_peak_bytes"],
                "steady_state_bytes": None, "observed_sampling_used_bytes": sampling["observed_sampling_root_used_bytes"],
                "heap_checkpoints": sampling["heap_checkpoints"],
                "observed_sdk_peak_bytes": sampling["observed_sdk_peak_bytes"],
                "after_retirement_bytes": sampling["root_heap_after_retirement_bytes"],
                "sdk_capacity_bytes": sampling["sdk_heap_bytes"],
                "linked_stack_bytes": sampling["linked_stack_bytes"], "stack_high_water_bytes": None,
            },
            "output_held": {
                "source": output["source"], "root_capacity_bytes": output["root_capacity_bytes"],
                "preparation_peak_bytes": output["root_peak_bytes"],
                "dormant_retained_bytes": output["root_used_bytes"],
                "execution_peak_bytes": None, "linked_stack_bytes": output["linked_stack_bytes"],
                "captured_value_slot_capacities": output["value_slot_capacities"],
                "stack_high_water_bytes": None,
            },
        },
        "inventory": {
            "sampling_value_payload": {"bytes": sum(capacities), "slot_capacities": capacities, "class": "persistent mutable", "metadata_and_allocator_overhead_included": False},
            "output_value_payload": {"bytes": items * extent, "slots": items, "per_slot_bytes": extent, "extent_basis": "Make ceiling from all independently checked payload Types; slot count unchanged", "class": "persistent mutable", "metadata_and_allocator_overhead_included": False},
            "output_sign_records": {"ordinary_items": None, "remote_items": sign_budget["remote_items"], "cpu_observed_ordinary_items": sign_budget["cpu_ordinary_items"], "cpu_observed_storage_bytes": sign_budget["cpu_storage_bytes"], "mcu_storage_bytes": None, "ordinary_items_formula": sign_budget["ordinary_items_formula"], "native_construction_diagnostic_maximum_frames": int(frame_limit[1]), "class": "persistent mutable", "derived_minimum": None, "derived_upper_bound_proven_for_finite_profile": True, "continuing_retention_or_handoff_proven": False},
            "mask_cpu": {"bytes": mask["after_mask_owner_bytes"], "heap_bytes": mask["after_mask_requested_heap_peak_bytes"], "pointer_bits": mask["pointer_bits"], "mcu_size_bytes": None},
            "packet_expression_cpu": {"before_bytes": expression_cpu["before_packet_source_bytes"], "after_bytes": expression_cpu["after_packet_source_bytes"], "pointer_bits": expression_cpu["pointer_bits"], "mcu_size_bytes": None, "compact_structured_tape_implemented": expression_cpu["compact_structured_tape_implemented"], "profile": expression_cpu["tape_profile"]},
            "quantity_pair_form": {"bytes": (int(temperature[1]) + int(humidity[1])) // 8, "types": ["Temperature", "Ratio"], "includes_status_clock_or_provenance": False, "selected_for_local_cords": False},
            "root_control_reserve": {"bytes": 8192, "class": "admission headroom", "already_allocated_object": False},
        },
        "combined": {
            "immutable_flash_bytes": None, "persistent_mutable_bytes": None,
            "reusable_scratch_bytes": None, "preparation_peak_bytes": None,
            "stack_high_water_bytes": None, "wireless_worst_case_bytes": None,
            "unshared_value_payload_bytes": sum(capacities) + items * extent,
            "sharing_proven": False,
            "missing_proofs": [
                "One combined realization with exact initialized Mask/clock/output custody",
                "Type-derived value extents and overlap including fan-out, pending calls, refusals and cancellation",
                "Preparation liveness and last-use release for descriptions and temporary schemas",
                "Continuing Mask mandatory-record retention or explicit handoff over its admitted lifetime",
                "Linked combined immutable material, static RAM and measured stack bound",
                "Concurrent sensing/rendering and AP/join/fallback SDK high water",
            ],
        },
        "inputs": {name: {"path": str(path.relative_to(ROOT)), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()} for name, path in inputs.items()},
    }
    for name, path in [("sampling_storage_source", storage), ("output_storage_source", output_storage), ("output_linked_layout", linked_layout), ("output_construction_source", output_construction), ("quantity_form_source", forms)]:
        result["inputs"][name] = {"path": str(path.relative_to(ROOT)), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
    for name, image in [("sampling_face", sampling_image), ("output_held", output_image)]:
        if image is not None:
            receipt_path = image.parent / ("image-receipt.json" if name == "sampling_face" else "image.json")
            receipt = read_json(receipt_path)
            image_sections = elf_sections(image)
            if receipt["source_head"] != result["captured_profiles"][name]["source"] or receipt["source_dirty"] or receipt["elf_sha256"] != image_sections["sha256"]:
                raise ValueError("image does not match the captured clean profile receipt")
            image_sections["receipt_sha256"] = hashlib.sha256(receipt_path.read_bytes()).hexdigest()
            result["captured_profiles"][name]["linked_image"] = image_sections
    return result


def markdown(value):
    profiles = value["captured_profiles"]
    sampling, output = profiles["sampling_face"], profiles["output_held"]
    lines = [
        "# Combined appliance memory ledger", "",
        "Status: combined admission is unproven. These profiles were captured separately; their peaks must not be added or subtracted to claim a fit.", "",
        "| Category | Current evidence | Combined obligation |", "|---|---|---|",
        f"| Immutable material | ELF section inventories in JSON when images are supplied | Link one combined image; separate flash material from RAM-resident code/data |",
        f"| Persistent mutable values | Sampling payload {value['inventory']['sampling_value_payload']['bytes']:,} B; output payload {value['inventory']['output_value_payload']['bytes']:,} B, plus metadata/allocator costs | Exact Types and simultaneous lifetimes; no reuse credited |",
        f"| Mask | {value['inventory']['mask_cpu']['bytes']} inline bytes, {value['inventory']['mask_cpu']['heap_bytes']} heap bytes in the captured {value['inventory']['mask_cpu']['pointer_bits']}-bit CPU measurement | Obtain MCU representation and admitted runtime custody |",
        f"| Packet expression | CPU retained model {value['inventory']['packet_expression_cpu']['before_bytes']:,} → {value['inventory']['packet_expression_cpu']['after_bytes']:,} B with constructor tape and fixed input locations | Dynamic leaf evaluators remain; flash material, lifetime allocation and MCU measurement still required |",
        f"| Local representations | Existing quantity-pair Forms total {value['inventory']['quantity_pair_form']['bytes']} B; semantic Types remain Temperature and Ratio | Admit compact local cord representations; separately retain status, clocks and provenance |",
        "| Scratch | No combined lifetime allocation exists | Include fan-out, pending calls, refusal and cancellation |",
        "| Construction | Output descriptors retire after transfer; schemas and expression trees remain in other paths | Bound transient peaks and every last-use release |",
        f"| Execution records | Derived for {value['inventory']['output_sign_records']['native_construction_diagnostic_maximum_frames']} inputs: {value['inventory']['output_sign_records']['remote_items']} remote records; ordinary count follows target record size and Plan minima ({value['inventory']['output_sign_records']['cpu_observed_ordinary_items']} in CPU64) | Admit continuing retention or explicit handoff; never discard required evidence |",
        f"| Root peaks | Sampling/Face {sampling['preparation_and_run_peak_bytes']:,} B; dormant output preparation {output['preparation_peak_bytes']:,} B | Measure combined preparation and execution separately |",
        f"| Retained Root | Dormant output {output['dormant_retained_bytes']:,} B; sampler observed at {min(sampling['observed_sampling_used_bytes']):,}–{max(sampling['observed_sampling_used_bytes']):,} B across three publications | Do not label post-retirement memory as steady state |",
        f"| Stack | Linked reservations: sampling {sampling['linked_stack_bytes']:,} B, output {output['linked_stack_bytes']:,} B | Reservations are not measured high-water bounds |",
        f"| Wireless | Separate SDK arena {sampling['sdk_capacity_bytes']:,} B; captured sampling peak {sampling['observed_sdk_peak_bytes']:,} B | Concurrent AP/client/fallback peak remains unmeasured |", "",
        f"The current stores require {value['combined']['unshared_value_payload_bytes']:,} payload bytes together. That is only an unshared value-backing subtotal, not a combined appliance total or proof of simultaneous feasibility.", "",
        f"The output capture still used {sum(output['captured_value_slot_capacities']):,} payload bytes. The current Type-derived profile uses {value['inventory']['output_value_payload']['bytes']:,}; its MCU retained/peak effect has not been captured. The capture also predates sign-budget reduction and used 256 ordinary and 256 remote slots. These are distinct source profiles.", "",
        "The CPU Mask figure and separately captured MCU profiles are intentionally not combined numerically. Source identities and input hashes are in memory-ledger.json.", "",
    ]
    if all("linked_image" in p for p in profiles.values()):
        lines += ["| Captured image | Flash-mapped sections | RAM section extent sum | RAM-resident executable sections |", "|---|---:|---:|---:|"]
        for name, profile in profiles.items():
            image = profile["linked_image"]
            lines.append(f"| {name} | {image['flash_mapped_bytes']:,} B | {image['ram_section_extent_sum_bytes']:,} B | {image['ram_resident_code_extent_bytes']:,} B |")
        lines += ["", "RAM section extents already include heap arenas and linked stack reservations. These figures are per-image inventories, not distinct physical-bank totals or an additive combined budget. RAM-resident executable sections are not automatically movable to flash.", ""]
    return "\n".join(lines)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "target/environment-memory-ledger")
    parser.add_argument("--sampling-image", type=Path)
    parser.add_argument("--output-image", type=Path)
    args = parser.parse_args()
    value = ledger(args.sampling_image, args.output_image)
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "memory-ledger.json").write_text(json.dumps(value, indent=2) + "\n")
    (args.output / "memory-ledger.md").write_text(markdown(value))
    print(args.output / "memory-ledger.md")
