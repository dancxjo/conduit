"""Acquire a fresh complete license; never trust an unverified historical cache."""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile

URL = ("https://developer.download.nvidia.com/embedded/L4T/"
       "r32_Release_v7.1/T186/Tegra_Software_License_Agreement-Tegra-Linux.txt")
MAX_BYTES = 2 * 1024 * 1024


def acquire(output):
    output = Path(output).absolute()
    if output.exists() or output.is_symlink():
        raise ValueError("license output must not exist")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(prefix="nano-license-", dir=output.parent) as temporary:
        subprocess.run(["curl", "--fail", "--location", "--max-filesize", str(MAX_BYTES),
                        "--output", temporary.name, URL], check=True)
        if not 1 <= Path(temporary.name).stat().st_size <= MAX_BYTES:
            raise ValueError("license download must be nonempty and at most 2 MiB")
        # Only a successful complete transfer can become a reusable artifact.
        os.link(temporary.name, output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    try:
        acquire(args.output)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, str(error) + "\n")
