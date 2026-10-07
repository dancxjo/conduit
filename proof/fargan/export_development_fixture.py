#!/usr/bin/env python3
"""Export locally supplied pinned C arrays for development; no network access."""
import argparse
import hashlib
import json
import math
import pathlib
import re
import struct

SOURCE_SHA = "68ba6e8731007f7184be862d7d1afe4cef096a7c9bcbfa115962340f725e004c"
UPSTREAM_SHA = "503d81b138d76621aae4b12786e90de48aa8db3a"
BLOB_SHA = {
    "f32": "d35f0510da476716183a33caafe45cc095e48d496f06de2db7655e780a385e47",
    "compact": "57afaa2df88a95fb09f0649934f1558ff8c46d82fa7627895535c65ed22e5891",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=pathlib.Path, help="pinned generated fargan_data.c")
    parser.add_argument("fixture", type=pathlib.Path, help="local development destination")
    args = parser.parse_args()
    source_bytes = args.source.read_bytes()
    if hashlib.sha256(source_bytes).hexdigest() != SOURCE_SHA:
        raise ValueError("generated source digest mismatch")
    source = source_bytes.decode("utf-8")
    arrays = []
    profiles = {"f32": bytearray(), "compact": bytearray()}
    pattern = r"static const (float|opus_int8) (\w+)\[(\d+)\] = \{(.*?)\};"
    for element, name, count, contents in re.findall(pattern, source, re.S):
        values = [value.strip() for value in contents.split(",") if value.strip()]
        if len(values) != int(count):
            raise ValueError(f"array extent mismatch: {name}")
        if element == "float":
            values = [float(value.rstrip("f")) for value in values]
            if not all(map(math.isfinite, values)):
                raise ValueError(f"nonfinite array: {name}")
            data = struct.pack("<" + "f" * len(values), *values)
        else:
            data = struct.pack("<" + "b" * len(values), *map(int, values))
        entry = {
            "name": name, "type": "f32le" if element == "float" else "i8",
            "elements": len(values), "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(), "profiles": {},
        }
        for profile, blob in profiles.items():
            if profile == "f32":
                admitted = name.endswith(("weights_float", "_bias"))
            else:
                admitted = name.endswith(("weights_int8", "_scale", "_subias", "_bias")) or (
                    name.endswith("weights_float")
                    and name.replace("_float", "_int8") not in source
                )
            if admitted:
                entry["profiles"][profile] = {"offset": len(blob), "bytes": len(data)}
                blob.extend(data)
        arrays.append(entry)
    # Validate all exported blobs before writing any model data.
    for profile, data in profiles.items():
        if hashlib.sha256(data).hexdigest() != BLOB_SHA[profile]:
            raise ValueError(f"exported profile digest mismatch: {profile}")
    manifest = {
        "upstream_sha": UPSTREAM_SHA, "source_sha256": SOURCE_SHA,
        "license_status": "local development only; explicit checkpoint grant unresolved",
        "arrays": arrays, "profiles": {},
    }
    resources = args.fixture / "resources"
    resources.mkdir(parents=True, exist_ok=True)
    for profile, data in profiles.items():
        (resources / (profile + ".bin")).write_bytes(data)
        manifest["profiles"][profile] = {"bytes": len(data), "sha256": BLOB_SHA[profile]}
    (resources / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    # Synthetic inputs exercise network state, not pronunciation or voice quality.
    features = []
    for frame in range(24):
        value = [0.0] * 20
        quiet = frame < 8 or frame >= 16
        value[0] = -8.0 if quiet else -2.0
        value[18] = math.log2(256 / (80 if frame < 12 else 160)) - 1.5
        value[19] = 0.0 if quiet else 0.8
        features.extend(value)
    (args.fixture / "conditions.f32le").write_bytes(
        struct.pack("<" + "f" * len(features), *features)
    )
    print(json.dumps(manifest["profiles"], indent=2))


if __name__ == "__main__":
    main()
