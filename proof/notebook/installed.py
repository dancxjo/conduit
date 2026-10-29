#!/usr/bin/env python3
"""Candidate process proof through public commands; no constructed Body state.

This seals exact candidate executable bytes for a private test installation. It
is not a clean release, browser/native Mask, QEMU or physical-device acceptance.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    args = parser.parse_args()
    checkout = Path(__file__).resolve().parents[2]
    output = checkout / "target" / "notebook-process" / f"{time.time_ns()}-{os.getpid()}"
    output.mkdir(parents=True)
    binary = output / "conduit-linux-x86_64"
    subprocess.run(["strip", "--strip-all", "-o", str(binary), str(args.binary.resolve())], check=True, timeout=120)
    binary.chmod(args.binary.stat().st_mode & 0o777)
    executable = binary.read_bytes()
    assert len(executable) <= 64 * 1024 * 1024, "candidate exceeds installed executable bound"
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=checkout, text=True).strip()
    delta = subprocess.check_output(["git", "diff", "HEAD", "--binary"], cwd=checkout)
    # Untracked sources are included in candidate custody, not hidden behind HEAD.
    untracked = subprocess.check_output(["git", "ls-files", "--others", "--exclude-standard", "-z"], cwd=checkout).split(b"\0")
    source_hash = hashlib.sha256(delta)
    for name in sorted(filter(None, untracked)):
        source_hash.update(name + b"\0" + (checkout / os.fsdecode(name)).read_bytes())
    source_identity = f"candidate:{commit}:delta-sha256:{source_hash.hexdigest()}"
    file_digest = digest(executable)
    bundle_digest = digest(f"conduit.release/host-bundle-content@1\0{binary.name}\0{file_digest}\n".encode())
    manifest = {
        "schema": "conduit.release/host-bundle@1", "source_identity": source_identity,
        "bundle_sha256": bundle_digest,
        "files": [{"path": binary.name, "bytes": len(executable), "sha256": file_digest}],
    }
    (output / "release.json").write_text(json.dumps(manifest, indent=2) + "\n")
    counter = 0

    def call(exe, arguments, *, expected=True, input_text=None):
        nonlocal counter
        counter += 1
        result = subprocess.run([str(exe), *map(str, arguments)], input=input_text, text=True,
                                capture_output=True, timeout=30)
        prefix = output / f"{counter:02d}"
        prefix.with_suffix(".stdout").write_text(result.stdout)
        prefix.with_suffix(".stderr").write_text(result.stderr)
        prefix.with_suffix(".command.json").write_text(json.dumps({"arguments": list(map(str, arguments)), "returncode": result.returncode}, indent=2) + "\n")
        if expected:
            assert result.returncode == 0, f"command {counter} failed: {result.stderr}"
        else:
            assert result.returncode != 0, f"command {counter} falsely acknowledged success"
        return result

    service = None
    service_logs = []
    with tempfile.TemporaryDirectory(prefix="conduit-notebook-", dir="/tmp") as short:
        private = Path(short)
        state = private / "state"
        storage = private / "storage"
        storage.mkdir(mode=0o700)
        call(binary, ["host", "service", "install", output / "release.json", "--state-dir", state, "--no-start"])
        installation = json.loads((state / "installation.json").read_text())
        installed = Path(installation["product_executable"])
        assert digest(installed.read_bytes()) == file_digest
        # Existing public Source ownership entrance births and retains the Body.
        born = call(installed, ["body", "own", checkout / "plots/notebook/main.conduit", "--state-dir", state, "--name", "Thoughts"], input_text="")
        truths = [json.loads(line) for line in born.stdout.splitlines() if line.strip()]
        body_id = truths[0]["biography"]["body_id"]

        def start(number):
            nonlocal service
            stdout = (output / f"service-{number}.stdout").open("w")
            stderr = (output / f"service-{number}.stderr").open("w")
            service_logs.extend([stdout, stderr])
            service = subprocess.Popen([str(installed), "host", "service", "run", "--state-dir", str(state)], stdout=stdout, stderr=stderr)
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                assert service.poll() is None, f"service {number} exited; inspect {output}"
                if (state / "control.sock").exists():
                    # A socket pathname is not readiness. Authenticate through the
                    # normal status command and inspect its current runtime Boot.
                    probe = subprocess.run([str(installed), "host", "service", "status", "--state-dir", str(state), "--json"], text=True, capture_output=True, timeout=5)
                    if probe.returncode == 0:
                        value = json.loads(probe.stdout)
                        runtime = value if value.get("process_id") else None
                        if runtime and runtime.get("process_id") == service.pid:
                            (output / f"boot-{number}.json").write_text(json.dumps(value, indent=2) + "\n")
                            return runtime
                time.sleep(0.05)
            raise AssertionError("service readiness deadline expired")

        def stop():
            nonlocal service
            if service is not None:
                service.terminate()
                try:
                    service.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    service.kill()
                    service.wait(timeout=5)
                service = None

        def notebook(*arguments, expected=True):
            result = call(installed, ["body", "notebook", "--state-dir", state, *arguments], expected=expected)
            if not expected:
                return None
            response = json.loads(result.stdout)
            assert response["body_id"] == body_id
            return response

        try:
            first = start(1)
            assert notebook("new")["name"] == "Thoughts"
            edited = notebook("replace", "--revision", 0, "--text", "I was here.")
            assert (edited["revision"], edited["text"]) == (1, "I was here.")
            saved = notebook("save", "--storage-dir", storage)
            assert saved["save_committed"] is True
            selected_before = (state / "body/notebook-commit.json").read_bytes()
            notebook("replace", "--revision", 0, "--text", "stale", expected=False)
            assert notebook("show")["text"] == "I was here."
            draft = notebook("replace", "--revision", 1, "--text", "Uncommitted changes")
            assert draft["revision"] == 2
            assert (state / "body/notebook-commit.json").read_bytes() == selected_before
            stop()
            # A dead owner cannot acknowledge a write through its stale socket.
            notebook("save", "--storage-dir", storage, expected=False)
            second = start(2)
            assert first["boot_id"] != second["boot_id"]
            assert first["process_id"] != second["process_id"]
            recovered = notebook("show")  # Startup uses the retained explicit storage selection.
            assert (recovered["revision"], recovered["text"]) == (1, "I was here.")
            recovered = notebook("reopen", "--storage-dir", storage)
            assert (recovered["revision"], recovered["text"]) == (1, "I was here.")
            assert recovered["name"] == "Thoughts"
            assert (state / "body/notebook-commit.json").read_bytes() == selected_before
            # A fresh Boot can continue through new authority on the same Body.
            continued = notebook("replace", "--revision", 1, "--text", "I was here. Again.")
            assert continued["revision"] == 2
            notebook("save", "--storage-dir", storage)
            pending = notebook("replace", "--revision", 2, "--text", "Still unsaved.")
            assert pending["revision"] == 3
            selected_after = (state / "body/notebook-commit.json").read_bytes()
            unavailable = private / "storage-unavailable"
            storage.rename(unavailable)
            notebook("save", "--storage-dir", storage, expected=False)
            retained = notebook("show")
            assert (retained["revision"], retained["text"]) == (3, "Still unsaved.")
            assert (state / "body/notebook-commit.json").read_bytes() == selected_after
            stop()
            # Retain bounded evidence, never the installation's private token.
            (output / "selected-commit.json").write_bytes((state / "body/notebook-commit.json").read_bytes())
            (output / "biography.json").write_bytes((state / "body/biography.json").read_bytes())
            summary = {
                "schema": "conduit.notebook/installed-process-proof@1",
                "proof_class": "candidate-installed-linux-process",
                "source_identity": source_identity, "executable_sha256": file_digest,
                "body_id": body_id, "first_boot": first, "second_boot": second,
                "committed_revision_recovered": 1, "uncommitted_revision_not_recovered": 2,
                "continued_revision": 2, "automatic_startup_recovery": True,
                "claims": ["authorized-file-save", "exact-revision-reopen", "stale-edit-refusal",
                           "dead-owner-no-acknowledgement", "fresh-process-same-body", "automatic-startup-recovery", "no-invented-draft",
                           "unavailable-storage-preserves-unsaved-text"],
            }
            (output / "proof.json").write_text(json.dumps(summary, indent=2) + "\n")
            print(f"Verified installed notebook process proof: {output}")
        finally:
            stop()
            for handle in service_logs:
                handle.close()


if __name__ == "__main__":
    main()
