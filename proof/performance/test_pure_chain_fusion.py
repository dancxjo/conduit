#!/usr/bin/env python3
"""Exercise the verifier against a real capture and rehashed semantic negatives.

Usage: python3 proof/performance/test_pure_chain_fusion.py CAPTURE
The original capture is read-only; all mutations use a temporary copy.
"""
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import sys
import tempfile

sys.dont_write_bytecode = True

spec = importlib.util.spec_from_file_location(
    "fusion_verifier", Path(__file__).with_name("verify-pure-chain-fusion.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
original = Path(sys.argv[1])
module.verify(original)


def rewrite(root, name, value, lines=False):
    path = root / name
    path.write_text("".join(json.dumps(v) + "\n" for v in value) if lines else json.dumps(value))
    status_path = root / "status.json"
    status = json.loads(status_path.read_text())
    status["artifact_sha256"][name] = hashlib.sha256(path.read_bytes()).hexdigest()
    status_path.write_text(json.dumps(status))


def rows(root):
    return [json.loads(line) for line in (root / "raw.jsonl").read_text().splitlines()]


def mutate_rows(root, mutate):
    data = rows(root)
    native = next(r for r in data if r.get("mode") == "native-fused")
    mutate(data, native)
    rewrite(root, "raw.jsonl", data, lines=True)


def missing_profile(root):
    mutate_rows(root, lambda data, native: data.remove(native))


def wrong_materialization(root):
    mutate_rows(root, lambda data, native: native["work_accounting"].update(intermediate_materializations=1))


def changed_sign(root):
    mutate_rows(root, lambda data, native: native["signs"][0].update(sequence=999999))


def changed_output(root):
    name = "sources/pure-chain-8-native-fused-0-outputs.json"
    data = json.loads((root / name).read_text())
    data[0]["bytes"][0] ^= 1
    rewrite(root, name, data)


def wrong_custody(root):
    path = root / "status.json"
    data = json.loads(path.read_text())
    data["source_custody_verified"] = False
    path.write_text(json.dumps(data))


for mutate in [missing_profile, wrong_materialization, changed_sign, changed_output, wrong_custody]:
    with tempfile.TemporaryDirectory(prefix="fusion-negative-") as directory:
        root = Path(directory) / "capture"
        shutil.copytree(original, root)
        mutate(root)
        try:
            module.verify(root)
        except AssertionError:
            print(f"refused {mutate.__name__}")
        else:
            raise AssertionError(f"accepted {mutate.__name__}")
print("verified real capture and five refusal controls")
