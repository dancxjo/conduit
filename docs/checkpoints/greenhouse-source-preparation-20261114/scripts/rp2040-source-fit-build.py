"""Internal uninstalled ARM qualification, not Make/Seed or permission to flash."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import argparse

task = Path(__file__).resolve().parent.parent
repo = task / "work/environment-extraction-integration-20261111"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--candidate", default="work/state-profile-candidate-20261113")
parser.add_argument("--log", default="outputs/rp2040-full-source-link-20261113.log")
args = parser.parse_args()
candidate = task / args.candidate
target = task / "work/cache-backing/rp2040-source-fit-20261113"
description = (candidate / "compile-image.json").read_bytes()
image = json.loads(description)["manifest"]
env = os.environ.copy()
env.update(CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS="4", CARGO_INCREMENTAL="0",
           CARGO_PROFILE_DEV_DEBUG="0", CARGO_PROFILE_TEST_DEBUG="0",
           CONDUIT_HOST_CONFIGURATION=str(candidate / "green-neutral-candidate.host.conduit"),
           CONDUIT_SOURCE_IDENTITY=image["source_identity"], CONDUIT_HOST_IMAGE_ID=image["image_id"],
           CONDUIT_HOST_IMAGE_DESCRIPTION=str(candidate / "compile-image.json"),
           CONDUIT_HOST_IMAGE_DESCRIPTION_SHA256=hashlib.sha256(description).hexdigest(),
           CONDUIT_WORKSET_ENTRIES='["shared-environment"]')
for field, filename in [("SOURCE", "workset.conduit"), ("SPECIALIZATIONS", "specializations.json")]:
    path = candidate / filename
    env["CONDUIT_WORKSET_" + field] = str(path)
    env["CONDUIT_WORKSET_" + field + "_SHA256"] = hashlib.sha256(path.read_bytes()).hexdigest()
with (task / args.log).open("w") as log:
    result = subprocess.run(["cargo", "+1.98.1", "build", "--manifest-path",
                             "targets/rp2040/firmware/host/Cargo.toml", "--locked", "--offline",
                             "--release", "--target", "thumbv6m-none-eabi"],
                            cwd=repo, env=env, stdout=log, stderr=subprocess.STDOUT)
print("Qualification build exit:", result.returncode, flush=True)
raise SystemExit(result.returncode)
