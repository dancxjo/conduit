"""Verify retained paired captures without extracting or executing archive contents."""
import hashlib
import json
import pathlib
import statistics
import tarfile

ROOT = pathlib.Path(__file__).resolve().parent
FIELDS = (
    "plan_sha256 output_sha256 signs_sha256 nodes cords steps numeric_steps "
    "expression_calls signs structural_drain pending_host_calls_at_end "
    "scheduler_inline_bytes pcm_max_abs_error active_state_max_abs_error "
    "conditioning_history_max_abs_error"
).split()
PAIRS = [
    ("fargan-preparation-before", "fargan-catalog-after"),
    ("fargan-final-before-2", "fargan-final-after-2"),
    ("fargan-final-before-3", "fargan-final-after-3"),
]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def compare(before, after):
    require(len(before) == len(after) == 48, "expected 48 frames per capture")
    expected = {(rep, frame) for rep in range(2) for frame in range(24)}
    for rows in (before, after):
        require({(r["repetition"], r["frame"]) for r in rows} == expected,
                "missing or duplicate frame identity")
        for row in rows:
            require(row["structural_drain"] is True, "not drained")
            require(row["pending_host_calls_at_end"] == 0, "pending Host Calls")
            require((row["nodes"], row["cords"], row["steps"], row["signs"])
                    == (768, 1211, 3165, 6976), "unexpected work")
            for key in ("allocations", "reallocations", "requested_bytes"):
                require(row["allocator"][key] == 0, "Play allocation")
    for left, right in zip(before, after):
        for key in ["repetition", "frame", *FIELDS]:
            require(left[key] == right[key], "paired mismatch: " + key)


def verify():
    evidence = json.loads((ROOT / "combined-evidence.json").read_text())
    archive_path = ROOT / "combined-raw-captures.tar.gz"
    require(archive_path.stat().st_size == evidence["archive_bytes"], "archive size")
    require(hashlib.sha256(archive_path.read_bytes()).hexdigest() == evidence["archive_sha256"],
            "archive digest")
    with tarfile.open(ROOT / "combined-raw-captures.tar.gz", "r:gz") as archive:
        def read(name):
            member = archive.getmember(name)
            require(member.isfile() and member.size < 100_000_000, "invalid member")
            return archive.extractfile(member).read()

        def capture(name, source):
            public = name in PAIRS[0]
            status = json.loads(read(name + ("/status.json" if public else "/capture-status.json")))
            require(status["source_custody_verified"] is True, "source custody")
            require(status["worker_exit_code"] == 0, "worker failed")
            keys = ("git_sha", "final_git_sha") if public else ("source_sha", "final_source_sha")
            require(all(status[k] == source for k in keys), "source changed")
            if public:
                require(status["conformance_success"] is True and status["worker_success"] is True,
                        "public conformance failed")
            manifest = status["artifact_sha256" if public else "file_sha256"]
            require({"raw.jsonl", "metadata.json", "source.conduit", "summary.json"} <= manifest.keys(),
                    "incomplete manifest")
            for filename, digest in manifest.items():
                require(hashlib.sha256(read(name + "/" + filename)).hexdigest() == digest,
                        "artifact digest: " + filename)
            metadata = json.loads(read(name + "/metadata.json"))
            require(metadata["git_sha"] == source, "metadata source")
            return [row for line in read(name + "/raw.jsonl").splitlines()
                    if (row := json.loads(line))["phase"] == "fargan-frame-play"]

        for pair, (before_name, after_name) in enumerate(PAIRS):
            before = capture(before_name, evidence["before_source"])
            after = capture(after_name, evidence["after_source"])
            compare(before, after)
            for mode, rows in [("before", before), ("after", after)]:
                actual = statistics.mean(r["owner_preparation_ns"] for r in rows) / 1e6
                expected = evidence["pairs"][pair][mode]["mean_preparation_ms"]
                require(abs(actual - expected) < 1e-9, "reported timing mismatch")
            # Prove comparison rejects changed evidence, even if its digest were refreshed.
            for field, value in [("signs_sha256", "changed"), ("output_sha256", "changed"),
                                 ("steps", 0), ("structural_drain", False)]:
                changed = [dict(r) for r in after]
                changed[0][field] = value
                try:
                    compare(before, changed)
                except ValueError:
                    pass
                else:
                    raise ValueError("mutation accepted: " + field)
    print("Verified 3 pairs, 144 frames per side, artifact custody, exact evidence, zero Play allocation and 12 mutation refusals.")


if __name__ == "__main__":
    verify()
