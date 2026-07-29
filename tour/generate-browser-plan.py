#!/usr/bin/env python3
"""Generate the exact static browser-host plan consumed by the Tour."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "tour/public/browser-plan.json"
ARTIFACTS = (
    ("browser-host-adapter", ROOT / "browser/conduit-browser-host.mjs", "../../browser/conduit-browser-host.mjs"),
    ("tour-worker", ROOT / "tour/public/tour-worker.mjs", "./tour-worker.mjs"),
    ("wasm-bindgen-loader", ROOT / "tour/public/conduit_web.js", "./conduit_web.js"),
    ("conduit-web-wasm", ROOT / "tour/public/conduit_web_bg.wasm", "./conduit_web_bg.wasm"),
)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


artifacts = [
    {
        "id": artifact_id,
        "path": public_path,
        "sha256": digest(path),
        "bytes": path.stat().st_size,
    }
    for artifact_id, path, public_path in ARTIFACTS
]
identity_input = {
    "schema": "conduit.tour-browser-plan/v1",
    "implementation_id": "conduit/tour-production-wasm-worker",
    "semantic_contract": "conduit/tour-panel-run",
    "placement": "dedicated-worker",
    "artifacts": artifacts,
}
identity_bytes = json.dumps(
    identity_input, sort_keys=True, separators=(",", ":")
).encode()
plan = {
    **identity_input,
    "plan_identity": hashlib.sha256(identity_bytes).hexdigest(),
    "observation_id": "conduit/tour-static-browser-observation-v1",
    "bounds": {
        "maximum_pending": 1,
        "maximum_message_bytes": 131072,
        "response_timeout_ms": 5000,
        "maximum_evidence_events": 32,
    },
}
rendered = json.dumps(plan, indent=2) + "\n"
if sys.argv[1:] == ["--check"]:
    if not OUTPUT.exists() or OUTPUT.read_text(encoding="utf-8") != rendered:
        raise SystemExit("tour/public/browser-plan.json is stale; run tour/build-wasm.sh")
elif sys.argv[1:]:
    raise SystemExit("usage: generate-browser-plan.py [--check]")
else:
    OUTPUT.write_text(rendered, encoding="utf-8")
