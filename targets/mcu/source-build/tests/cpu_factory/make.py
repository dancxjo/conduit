"""Reproduce the checked CPU factory from Source, without building firmware.

The public IMAGE is a checked Make input, never a Seed or membership grant.
The generator and CPU probe use one completed, caller-owned Cargo graph.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--configuration", type=Path, required=True)
    parser.add_argument("--image", type=Path, required=True)
    parser.add_argument("--workset", type=Path, required=True)
    parser.add_argument("--entry", action="append", required=True)
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[5]
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    configuration = args.configuration.resolve(strict=True)
    image_path = args.image.resolve(strict=True)
    workset = args.workset.resolve(strict=True)
    if image_path.stat().st_size > 1024 * 1024:
        raise ValueError("public IMAGE input exceeds its finite frame")
    if not 0 < workset.stat().st_size <= 1024 * 1024:
        raise ValueError("workset Source input extent")
    image = json.loads(image_path.read_text())
    # Same ordered-input identity as ordinary Make's WorksetSourceInput.
    identity = hashlib.sha256(b"conduit.make/workset-source@1\0")
    for text in [args.source_revision, digest(workset), *args.entry]:
        part = text.encode()
        identity.update(len(part).to_bytes(4, "little"))
        identity.update(part)
    source_identity = "sha256:" + identity.hexdigest()
    if image["manifest"]["source_identity"] != source_identity:
        raise ValueError("checked IMAGE differs from ordered Source inputs")
    env = {k: v for k, v in os.environ.items() if not k.startswith("CPU_")}
    env.update(
        CARGO_TARGET_DIR=str(args.cache.resolve()), CARGO_BUILD_JOBS="1",
        CARGO_INCREMENTAL="0", CARGO_PROFILE_DEV_DEBUG="0",
        CARGO_PROFILE_TEST_DEBUG="0", RUSTUP_TOOLCHAIN="1.98.1",
        CONDUIT_HOST_CONFIGURATION=str(configuration),
        CONDUIT_HOST_IMAGE_ID=image["manifest"]["image_id"],
        CONDUIT_SOURCE_IDENTITY=source_identity,
        CONDUIT_HOST_IMAGE_DESCRIPTION=str(image_path),
        CONDUIT_HOST_IMAGE_DESCRIPTION_SHA256=digest(image_path),
        CONDUIT_WORKSET_SOURCE=str(workset),
        CONDUIT_WORKSET_SOURCE_SHA256=digest(workset),
        CONDUIT_WORKSET_ENTRIES=json.dumps(args.entry),
    )
    # This first recipe has no compilation-selection input. Never inherit one.
    for key in ["CONDUIT_WORKSET_SPECIALIZATIONS", "CONDUIT_WORKSET_SPECIALIZATIONS_SHA256"]:
        env.pop(key, None)
    commands = []

    def run(name, command, extra=None):
        commands.append(dict(name=name, command=command))
        (output / "commands.json").write_text(json.dumps(commands, indent=2) + "\n")
        with (output / (name + ".log")).open("w") as log:
            subprocess.run(command, cwd=repo, env=dict(env, **(extra or {})),
                           stdout=log, stderr=subprocess.STDOUT, check=True)

    command = ["cargo", "+1.98.1", "build", "--locked"]
    if args.offline:
        command.append("--offline")
    command += ["-p", "conduit-host-esp32-make", "--features", "source-emission",
                "--example", "emit-source"]
    run("build", command)
    cache = args.cache.resolve() / "debug"
    generated = output / "generated"
    run("generate", [str(cache / "examples/emit-source"), str(generated)])
    fingerprints = []
    executable_digest = digest(cache / "examples/emit-source")
    for path in (cache / ".fingerprint").glob("conduit-host-esp32-make-*/example-emit-source.json"):
        # Clippy/check may retain another fingerprint with the same features.
        # Bind to the binary from the successful build, never the newest file
        # or an unrelated cached graph. Cargo hashes its Rust crate filename.
        binary = cache / "examples" / ("emit_source-" + path.parent.name.rsplit("-", 1)[1])
        if (json.loads(path.read_text())["features"] == '["source-emission"]'
                and binary.is_file() and digest(binary) == executable_digest):
            fingerprints.append(path)
    if len(fingerprints) != 1:
        raise ValueError("require one completed emitter graph; use an isolated cohort cache")
    fingerprint = fingerprints[0]
    common = ["python3", str(Path(__file__).with_name("run.py")),
              "--generated", str(generated), "--fingerprint", str(fingerprint)]
    run("factory", common + ["--output", str(output / "factory")], dict(
        CPU_EXPECT_PREPARED="1", CPU_RUN_SOURCE="1", CPU_FAIL_FORE_READ="1",
        CPU_TARGET_SAMPLES="6"))
    run("shortage", common + ["--output", str(output / "shortage")], dict(
        CPU_EXPECT_PREPARED="1", CPU_ADMISSION_SHORTAGE="1"))
    receipt = dict(
        source_commit=subprocess.check_output(["git", "rev-parse", "HEAD"],
                                               cwd=repo, text=True).strip(),
        inputs={str(path): digest(path) for path in [configuration, image_path, workset]},
        source_revision=args.source_revision, entries=args.entry,
        workset_identity=source_identity,
        generated={path.name: digest(path) for path in generated.glob("*.rs")
                   if not path.name.startswith("source_")},
        fingerprint_sha256=digest(fingerprint),
        factory=json.loads((output / "factory/receipt.json").read_text()),
        shortage=json.loads((output / "shortage/receipt.json").read_text()),
        hardware_effects=False,
    )
    (output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    for field in ["fresh_public_root_admission", "whole_factory_called", "admission",
                  "Start", "original_Source_execution_completed"]:
        if not receipt["factory"][field]:
            raise ValueError("CPU factory obligation not met: " + field)
    if not receipt["shortage"]["exact_runtime_shortage_refused_before_effects"]:
        raise ValueError("exact runtime shortage refusal not demonstrated")


if __name__ == "__main__":
    main()
