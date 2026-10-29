"""One-shot fault selection and verification of an actually lost acceptance."""
import hashlib
import json
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

class SingleAcceptanceLoss:
    def __init__(self):
        self.armed = False
        self.triggered = False

    def should_drop(self, reply):
        acknowledgement = reply.get("acknowledgement") if isinstance(reply, dict) else None
        if self.armed and not self.triggered and isinstance(reply, dict) and reply.get("event") == "native-display-check-control" and isinstance(acknowledgement, dict) and acknowledgement.get("state") == "EvidenceAccepted":
            self.armed = False
            self.triggered = True
            return True
        return False

def request_digest(request):
    operations = ["Inspect", "Reserve", "Prepare", "Start", "Release", "Cancel", "Retire", "AcceptEvidence"]
    assert request["protocol"] == 3
    material = b"conduit.environment/root-owner-control@3" + (3).to_bytes(2, "little") + bytes([operations.index(request["operation"])])
    material += request["sequence"].to_bytes(8, "little") + request["offer_generation"].to_bytes(8, "little")
    for name in ["host", "boot", "body", "wake", "body_plan", "body_play", "plan", "fragment", "fragment_play"]:
        encoded = request[name].encode()
        material += len(encoded).to_bytes(2, "little") + encoded
    material += bytes(request["assignment_seal"])
    if request.get("evidence"):
        material += request["evidence"]["invocation"].to_bytes(8, "little") + bytes(request["evidence"]["sha256"])
    return hashlib.sha256(material).digest()

def verify_lost_receipt(run, reply):
    # Verify that the dropped receipt is for the exact page already synced by
    # this owner. Reuse the published wire domains/field order, never a secret.
    files = list((run / "display-evidence").glob(f"*-q{reply['sequence'] - 1}.json"))
    assert len(files) == 1
    retained = json.loads(files[0].read_bytes())
    request = retained["request"]
    page = retained["offer"]["page"]
    request.update(operation="AcceptEvidence", sequence=reply["sequence"], evidence=page)
    command = request_digest(request)
    receipt = reply["acknowledgement"]
    assert receipt["state"] == "EvidenceAccepted" and receipt["snapshot"] is None
    assert bytes(receipt["command"]) == command
    key = bytes(json.loads((run / "mcu-admission.json").read_bytes())["advertisement"]["verifying_key"])
    transcript = hashlib.sha256(b"conduit.environment/root-owner-acknowledgement@2" + command + bytes([5, 0])).digest()
    Ed25519PublicKey.from_public_bytes(key).verify(bytes(reply["acknowledgement_signature"]), transcript)
    return page["invocation"]
