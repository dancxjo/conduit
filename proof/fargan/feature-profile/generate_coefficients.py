#!/usr/bin/env python3
"""Public deterministic analysis coefficients; no trained model or downloads.

The Source feature policy owns band endpoints, normalization and DCT profile.
This development exporter materializes their ordinary immutable tensor inputs.
"""
import hashlib
import json
import math
import pathlib
import struct
import sys

out = pathlib.Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
bands = [0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 16, 20, 24, 28, 34, 40]
weights = [[0.0] * 18 for _ in range(161)]
for band in range(17):
    width = 4 * (bands[band + 1] - bands[band])
    for j in range(width):
        index = 4 * bands[band] + j
        weights[index][band] += 1 - j / width
        weights[index][band + 1] += j / width
# Analysis input is normalized I16 PCM; generic DFT is unnormalized. The
# upstream amplitude basis uses raw I16-valued floats and FFT scale1/320.
for row in weights:
    row[0] *= 2
    row[-1] *= 2
    for i in range(18):
        row[i] *= (32768 / 320) ** 2
arrays = {
    "bands161x18": ([161, 18], [v for row in weights for v in row]),
    "band_bias18": ([18], [0.01] * 18),
    "dct18x18": ([18, 18], [math.cos((i + 0.5) * j * math.pi / 18) * math.sqrt(2 / 18) * (math.sqrt(0.5) if j == 0 else 1) for i in range(18) for j in range(18)]),
}
manifest = {"profile": "speech/fargan-formant-spectral-approximation@1", "element": "f32-le", "matrix_order": "input-major", "arrays": {}}
expected = {
    "bands161x18": "f3e181ecfdf3a579b76699a0924efebbb2ba6e0a9837cb265a87e929c18da1c3",
    "band_bias18": "ccc8ef501158d683a793b5de106853c5d7a556205bc98003893c0c9d5c12f123",
    "dct18x18": "0ee4290f4ec2b3408882aefe5d379cd9a8e0050b319a0e67d297d22442b928a5",
}
for name, (shape, values) in arrays.items():
    data = struct.pack("<" + "f" * len(values), *values)
    if hashlib.sha256(data).hexdigest() != expected[name]:
        raise SystemExit("coefficient digest mismatch: " + name)
    (out / (name + ".bin")).write_bytes(data)
    manifest["arrays"][name] = {"shape": shape, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
(out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
