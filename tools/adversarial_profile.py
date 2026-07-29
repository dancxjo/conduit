#!/usr/bin/env python3
"""Report exact adversarial-conformance support for non-hosted profiles."""

from __future__ import annotations

import argparse
import json
import pathlib


ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance/c5/adversarial-containment-v1.json"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--profile",
        required=True,
        choices=["constrained", "physical-hil"],
    )
    args = parser.parse_args()
    fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
    constrained = {
        "budget-reset-across-lifecycle",
        "old-hazardous-command-after-transition",
    }
    cases = []
    for case in fixture["cases"]:
        if args.profile == "constrained" and case["id"] in constrained:
            status = "executed-by-conduit-embedded-test"
        else:
            status = "unsupported"
        cases.append(
            {
                "id": case["id"],
                "status": status,
                "reason": (
                    None
                    if status.startswith("executed")
                    else "profile has no production implementation or physical fixture for this attack"
                ),
            }
        )
    print(
        json.dumps(
            {
                "schema": "conduit.adversarial-profile-report/v1",
                "profile": args.profile,
                "fixture_seed": fixture["seed"],
                "cases": cases,
                "claim_boundary": fixture["claim_boundary"],
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
