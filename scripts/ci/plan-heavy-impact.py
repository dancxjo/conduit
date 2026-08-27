#!/usr/bin/env python3
"""Plan heavyweight CI obligations from an exact Git diff.

This first CI-planning slice is deliberately conservative:
- Cargo path dependencies are discovered from every package in the checkout,
  including standalone firmware workspaces.
- A changed package selects a heavyweight suite only when it is in that
  suite's dependency closure.
- Non-Cargo assets use small, explicit ownership rules.
- Unknown/global changes select every heavyweight suite.

The GitHub workflow can keep exact-main/merge-queue runs exhaustive while using
these outputs to make pull-request validation selective.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from typing import Iterable

ROOT = Path(__file__).resolve().parents[2]

SKIP_DIR_NAMES = {".git", "target", "node_modules"}

SUITE_ROOTS = {
    "esp32": {
        "conduit-esp32-c3-signal",
        "conduit-esp32-s3-signal",
        "conduit-esp32-wroom-signal",
        "conduit-host-esp32-fabrication",
    },
    "browser": {
        "conduit-browser-host",
        "conduit-browser-runtime",
        "patchbay-html",
        "patchbay-hosted",
        "patchbay-model",
        "patchbay-native",
    },
    "conduitos": {
        "conduitos",
        "conduit-host-conduitos-fabrication",
        "conduit-workspace-fabrication",
    },
}

DIRECT_PREFIXES = {
    "esp32": (
        "firmware/conduit-esp32-",
        "targets/esp32/",
    ),
    "browser": (
        "hosts/browser-",
        "apps/patchbay/",
        "proof/browser/",
        "assets/",
    ),
    "conduitos": (
        "hosts/conduitos/",
        "profiles/hosts/conduitos",
    ),
}

# These files/directories can alter build/proof orchestration across ownership
# boundaries. The safe response is a complete heavyweight PR plan.
GLOBAL_PREFIXES = (
    ".github/",
    ".cargo/",
    "xtask/",
    "scripts/ci/",
)
GLOBAL_FILES = {
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain",
    "rust-toolchain.toml",
}

# Non-package paths that are deliberately irrelevant to heavyweight target
# selection. Workspace checks still run for non-doc PRs in this first slice.
HARMLESS_PREFIXES = (
    "docs/",
    "examples/",
)
HARMLESS_FILES = {
    "README.md",
    "STATUS.md",
    "AGENTS.md",
    "LICENSE",
    "justfile",
}


@dataclass(frozen=True)
class Package:
    name: str
    directory: Path
    dependencies: frozenset[str]


def _dependency_tables(data: dict) -> Iterable[dict]:
    for key in ("dependencies", "dev-dependencies", "build-dependencies"):
        table = data.get(key)
        if isinstance(table, dict):
            yield table

    target = data.get("target")
    if isinstance(target, dict):
        for target_table in target.values():
            if not isinstance(target_table, dict):
                continue
            for key in ("dependencies", "dev-dependencies", "build-dependencies"):
                table = target_table.get(key)
                if isinstance(table, dict):
                    yield table


def discover_packages(root: Path = ROOT) -> dict[str, Package]:
    manifests: list[tuple[Path, dict]] = []
    for current, dirs, files in os.walk(root):
        dirs[:] = [d for d in dirs if d not in SKIP_DIR_NAMES]
        if "Cargo.toml" not in files:
            continue
        manifest = Path(current) / "Cargo.toml"
        try:
            data = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError):
            # A malformed manifest will fail normal CI; impact planning must not
            # silently classify it as harmless.
            continue
        package = data.get("package")
        if isinstance(package, dict) and isinstance(package.get("name"), str):
            manifests.append((manifest, data))

    name_by_dir: dict[Path, str] = {}
    for manifest, data in manifests:
        name_by_dir[manifest.parent.resolve()] = data["package"]["name"]

    packages: dict[str, Package] = {}
    for manifest, data in manifests:
        directory = manifest.parent.resolve()
        dependencies: set[str] = set()
        for table in _dependency_tables(data):
            for spec in table.values():
                if not isinstance(spec, dict):
                    continue
                rel = spec.get("path")
                if not isinstance(rel, str):
                    continue
                dep_dir = (directory / rel).resolve()
                dep_name = name_by_dir.get(dep_dir)
                if dep_name is not None:
                    dependencies.add(dep_name)
        name = data["package"]["name"]
        packages[name] = Package(name, directory, frozenset(dependencies))
    return packages


def dependency_closure(packages: dict[str, Package], roots: set[str]) -> set[str]:
    missing = sorted(root for root in roots if root not in packages)
    if missing:
        raise KeyError(f"configured CI root package(s) missing: {', '.join(missing)}")

    seen: set[str] = set()
    stack = list(roots)
    while stack:
        name = stack.pop()
        if name in seen:
            continue
        seen.add(name)
        package = packages.get(name)
        if package is not None:
            stack.extend(package.dependencies - seen)
    return seen


def package_for_path(path: str, packages: dict[str, Package]) -> str | None:
    absolute = (ROOT / path).resolve()
    best: tuple[int, str] | None = None
    for package in packages.values():
        try:
            absolute.relative_to(package.directory)
        except ValueError:
            continue
        score = len(package.directory.parts)
        if best is None or score > best[0]:
            best = (score, package.name)
    return None if best is None else best[1]


def _starts_with_any(path: str, prefixes: Iterable[str]) -> bool:
    return any(path.startswith(prefix) for prefix in prefixes)


def full_plan(reason: str, paths: list[str]) -> dict:
    return {
        "esp32_required": True,
        "browser_required": True,
        "conduitos_required": True,
        "full_fallback": True,
        "reason": reason,
        "changed_paths": paths,
        "changed_packages": [],
        "suite_reasons": {
            "esp32": [reason],
            "browser": [reason],
            "conduitos": [reason],
        },
    }


def plan_for_paths(paths: list[str], packages: dict[str, Package] | None = None) -> dict:
    packages = packages or discover_packages()
    try:
        closures = {
            suite: dependency_closure(packages, roots)
            for suite, roots in SUITE_ROOTS.items()
        }
    except KeyError as exc:
        return full_plan(f"safe-fallback:{exc}", paths)

    selected = {suite: False for suite in SUITE_ROOTS}
    reasons: dict[str, list[str]] = {suite: [] for suite in SUITE_ROOTS}
    changed_packages: set[str] = set()

    # Markdown-only changes are already handled by the existing docs classifier,
    # but keeping them cheap here makes the planner independently sensible.
    substantive = [path for path in paths if not path.endswith(".md")]
    if not substantive:
        return {
            "esp32_required": False,
            "browser_required": False,
            "conduitos_required": False,
            "full_fallback": False,
            "reason": "markdown-only",
            "changed_paths": paths,
            "changed_packages": [],
            "suite_reasons": reasons,
        }

    for path in substantive:
        if path in GLOBAL_FILES or _starts_with_any(path, GLOBAL_PREFIXES):
            return full_plan(f"global-change:{path}", paths)

        direct = False
        for suite, prefixes in DIRECT_PREFIXES.items():
            if _starts_with_any(path, prefixes):
                selected[suite] = True
                reasons[suite].append(f"owned-path:{path}")
                direct = True

        package_name = package_for_path(path, packages)
        if package_name is not None:
            changed_packages.add(package_name)
            for suite, closure in closures.items():
                if package_name in closure:
                    selected[suite] = True
                    reasons[suite].append(f"package-dependency:{package_name}")
            continue

        if direct:
            continue
        if path in HARMLESS_FILES or _starts_with_any(path, HARMLESS_PREFIXES):
            continue

        # Do not guess about a non-package source/artifact path. A new ownership
        # class must be taught to this planner explicitly before it can be
        # skipped safely.
        return full_plan(f"unclassified-path:{path}", paths)

    selected_names = [suite for suite, value in selected.items() if value]
    reason = (
        "no-heavyweight-obligations"
        if not selected_names
        else "selected:" + ",".join(sorted(selected_names))
    )
    return {
        "esp32_required": selected["esp32"],
        "browser_required": selected["browser"],
        "conduitos_required": selected["conduitos"],
        "full_fallback": False,
        "reason": reason,
        "changed_paths": paths,
        "changed_packages": sorted(changed_packages),
        "suite_reasons": reasons,
    }


def changed_paths(base: str, head: str) -> list[str]:
    for value, label in ((base, "base"), (head, "head")):
        if len(value) != 40 or any(ch not in "0123456789abcdefABCDEF" for ch in value):
            raise ValueError(f"invalid {label} SHA")
        subprocess.run(
            ["git", "cat-file", "-e", f"{value}^{{commit}}"],
            cwd=ROOT,
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    output = subprocess.check_output(
        [
            "git",
            "diff",
            "--name-only",
            "-z",
            "--diff-filter=ACDMRTUXB",
            base,
            head,
        ],
        cwd=ROOT,
    )
    return [entry.decode("utf-8") for entry in output.split(b"\0") if entry]


def github_bool(value: bool) -> str:
    return "true" if value else "false"


def write_outputs(plan: dict) -> None:
    print(f"esp32_required={github_bool(plan['esp32_required'])}")
    print(f"browser_required={github_bool(plan['browser_required'])}")
    print(f"conduitos_required={github_bool(plan['conduitos_required'])}")
    print(f"full_fallback={github_bool(plan['full_fallback'])}")
    print(f"impact_reason={plan['reason']}")


def write_summary(plan: dict, destination: Path) -> None:
    rows = []
    for suite in ("esp32", "browser", "conduitos"):
        required = plan[f"{suite}_required"]
        why = ", ".join(plan["suite_reasons"][suite]) or "no dependency/ownership path"
        rows.append(f"| {suite} | {'run' if required else 'skip on PR'} | {why} |")
    changed_packages = ", ".join(plan["changed_packages"]) or "(none)"
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(
        "\n".join(
            [
                "## CI impact plan",
                "",
                f"Reason: `{plan['reason']}`",
                "",
                f"Changed packages: {changed_packages}",
                "",
                "| heavyweight suite | decision | reason |",
                "| --- | --- | --- |",
                *rows,
                "",
                "> Pull requests may use this plan selectively. Main and merge-queue runs remain exhaustive in this slice.",
                "",
            ]
        ),
        encoding="utf-8",
    )


def self_test() -> None:
    packages = discover_packages()
    cases = [
        (
            ["README.md", "docs/architecture/foo.md"],
            (False, False, False),
            "markdown-only",
        ),
        (
            ["apps/pete/src/lib.rs"],
            (False, False, False),
            "application-only",
        ),
        (
            ["firmware/conduit-esp32-c3-signal/src/main.rs"],
            (True, False, False),
            "esp32-owned path",
        ),
        (
            ["proof/browser/pointer.spec.mjs"],
            (False, True, False),
            "browser proof",
        ),
        (
            ["hosts/conduitos/src/main.rs"],
            (False, False, True),
            "ConduitOS",
        ),
        (
            ["crates/conduit-kernel/src/lib.rs"],
            (True, True, True),
            "shared kernel",
        ),
        (
            [".github/workflows/check.yml"],
            (True, True, True),
            "CI global fallback",
        ),
    ]
    for paths, expected, label in cases:
        plan = plan_for_paths(paths, packages)
        actual = (
            plan["esp32_required"],
            plan["browser_required"],
            plan["conduitos_required"],
        )
        if actual != expected:
            raise AssertionError(
                f"{label}: expected {expected}, got {actual}; plan={json.dumps(plan, sort_keys=True)}"
            )
    print("impact-planner-self-test: ok", file=sys.stderr)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("base", nargs="?")
    parser.add_argument("head", nargs="?")
    parser.add_argument("--json-out", type=Path)
    parser.add_argument("--summary-out", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return 0

    if not args.base or not args.head:
        parser.error("base and head SHAs are required unless --self-test is used")

    try:
        paths = changed_paths(args.base, args.head)
        plan = plan_for_paths(paths)
    except Exception as exc:  # Safe failure mode: run all heavyweight suites.
        plan = full_plan(f"planner-error:{type(exc).__name__}:{exc}", [])

    write_outputs(plan)
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(plan, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if args.summary_out:
        write_summary(plan, args.summary_out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
