"""Read-only verification of a retained owner after a lost acceptance reply.

Usable after repairing a faulty relay without resetting the MCU or restarting
its owner. Verifies the actual dropped signature, immutable history, continued
signed sampler publications and complete initial display backlog. No lifecycle
or network operation is sent to the device.
"""
import hashlib
import json
import os
from pathlib import Path
import sys
sys.dont_write_bytecode = True
from acceptance_loss import request_digest, verify_lost_receipt
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

pid, run, log, dropped = sys.argv[1:]
pid = int(pid)
run = Path(run).resolve()
log = Path(log)
args = Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0")
assert Path(os.fsdecode(args[0])).name == "conduit-environment-host"
assert args[1].startswith(b"tcp://")
assert Path(os.fsdecode(args[2])).resolve() == run.parents[1]
open_paths = []
for fd in Path(f"/proc/{pid}/fd").iterdir():
    try:
        open_paths.append(fd.readlink())
    except FileNotFoundError:
        pass
assert run / "native-observations.ndjson" in open_paths
assert not (run / "body-lulled.json").exists()

def rows(path):
    data = path.read_bytes()
    data = data[:data.rfind(b"\n") + 1]
    return [json.loads(line) for line in data.splitlines() if line]

reply = json.loads(Path(dropped).read_bytes())
invocation = verify_lost_receipt(run, reply)
admission = json.loads((run / "mcu-admission.json").read_bytes())
key = Ed25519PublicKey.from_public_bytes(bytes(admission["advertisement"]["verifying_key"]))
boot = admission["reply"]["boot_id"]
recovered = [r for r in rows(log) if r.get("event") == "owner-link-recovered"]
assert recovered and all(r["boot"] == boot for r in recovered)
control = rows(run / "native-control.ndjson")
assert not any(r["request"]["sequence"] > reply["sequence"] for r in control)
context = control[0]["request"]
scope = b"".join(hashlib.sha256(context[k].encode()).digest() for k in ["body", "boot", "fragment_play"])
forms = json.loads(next(run.glob("observed-history-0000-*.json")).read_bytes())["forms"]
confirmed = max(r["through_sequence"] for r in rows(log) if r.get("event") == "verified-observation-history")
observations = {}
for path in run.glob("observed-history-*.json"):
    if int(path.stem.rsplit("-", 1)[1]) > confirmed:
        continue  # A newly created file is not published until owner fsync.
    batch = json.loads(path.read_bytes())
    data = bytes.fromhex(batch["records"])
    assert bytes.fromhex(batch["scope"]) == scope and batch["forms"] == forms
    assert len(data) == min(4, batch["committed"] - batch["after"]) * 21
    digest = hashlib.sha256(b"conduit.environment/observed-history@1\0" + scope + bytes.fromhex(forms) + batch["after"].to_bytes(4,"little") + batch["committed"].to_bytes(4,"little") + len(data).to_bytes(8,"little") + data).digest()
    key.verify(bytes.fromhex(batch["signature"]),digest)
    for index in range(0, len(data), 21):
        record = data[index:index + 21]
        sequence = int.from_bytes(record[:8],"little")
        assert sequence == batch["after"] + index // 21 + 1
        assert observations.get(sequence,record) == record
        observations[sequence] = record
assert sorted(observations) == list(range(1, max(observations) + 1))
assert max(observations) >= 4
assert any(record[16] == 0 for sequence,record in observations.items() if sequence > 1)
latest = rows(run / "native-observations.ndjson")[-1]
request = latest["request"]
receipt = latest["receipt"]
snapshot = receipt["snapshot"]
assert request["boot"] == boot
assert all(request[name] == context[name] for name in ["host", "boot", "body", "wake", "body_plan", "body_play", "plan", "fragment", "fragment_play", "offer_generation", "assignment_seal"])
assert request["sequence"] >= recovered[-1]["next_sequence"] and receipt["state"] == "Observed"
assert bytes(receipt["command"]) == request_digest(request)
canonical = bytes(latest["canonical_bytes"])
assert len(canonical) == snapshot["bytes"] and hashlib.sha256(canonical).digest() == bytes(snapshot["sha256"])
material = b"conduit.environment/root-owner-acknowledgement@2" + bytes(receipt["command"]) + bytes([3,1])
for name in ["sequence", "committed_millis", "observed_millis"]:
    material += snapshot[name].to_bytes(8,"little")
material += bytes([snapshot["channel"]]) + snapshot["bytes"].to_bytes(2,"little") + bytes(snapshot["sha256"])
key.verify(bytes(latest["root_signature"]), hashlib.sha256(material).digest())
display = rows(run / "display-control.ndjson")
assert not any(r["request"]["sequence"] > reply["sequence"] and r["request"]["operation"] in ["Reserve", "Prepare", "Start", "Release"] for r in display)
following = [r for r in display if r["request"]["sequence"] >= recovered[-1]["next_sequence"]]
first = min(following, key=lambda r:r["request"]["sequence"])["reply"]
target = first["local_evidence"]["committed"]
current = max(display,key=lambda r:r["request"]["sequence"])["reply"]
assert current["worker"]["failure"] is None and not current["local_evidence"]["failed"]
assert current["local_evidence"]["accepted"] >= target > reply["local_evidence"]["accepted"] + 20
for number in range(target):
    files = list((run / "display-evidence").glob(f"{number:020d}-*.bin"))
    assert len(files) == 1
    data = files[0].read_bytes()
    metadata = json.loads(next(files[0].parent.glob(files[0].stem + "-q*.json")).read_bytes())
    offer = metadata["offer"]
    assert bytes(offer["command"]) == request_digest(metadata["request"])
    assert hashlib.sha256(data).digest() == bytes(offer["page"]["sha256"]) and len(data) == offer["bytes"]
    digest = hashlib.sha256(b"conduit.environment/display-evidence-offer@1" + bytes(offer["command"]) + number.to_bytes(8,"little") + bytes(offer["page"]["sha256"]) + len(data).to_bytes(2,"little")).digest()
    key.verify(bytes(metadata["root_signature"]),digest)
print(json.dumps({"event":"retained-carrier-recovery-verified","pid":pid,"boot":boot,"reconnections":len(recovered),"dropped_invocation":invocation,"history_through":max(observations),"recovered_display_prefix":target,"display":current["local_evidence"],"job_restarted":False}),flush=True)
