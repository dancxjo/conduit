#!/usr/bin/env python3
"""Derive the tiny, reviewable PB2007 capstone slice from the exact deposit."""

import argparse
import hashlib
import io
import json
import math
import struct
import wave
import zipfile

ARCHIVE_SHA256 = "123d3fc2f114ab37724c7f05e00a03ff21d7e815f7f957987e8255f56d73f243"
ITEMS = [f"item_{index:04d}" for index in range(1, 13)]
BINS = 16


def digest(data):
    return hashlib.sha256(data).hexdigest()


def means(values, bins=BINS):
    result = []
    for index in range(bins):
        start = index * len(values) // bins
        end = (index + 1) * len(values) // bins
        bucket = values[start:end]
        result.append(sum(bucket) / len(bucket))
    return result


def acoustic_frames(data):
    with wave.open(io.BytesIO(data), "rb") as source:
        if source.getnchannels() != 1 or source.getsampwidth() != 2 or source.getframerate() != 16000:
            raise ValueError("PB2007 slice requires mono 16-bit 16 kHz PCM")
        samples = struct.unpack(f"<{source.getnframes()}h", source.readframes(source.getnframes()))
    frames = []
    for index in range(BINS):
        start = index * len(samples) // BINS
        end = (index + 1) * len(samples) // BINS
        bucket = samples[start:end]
        rms = math.sqrt(sum(value * value for value in bucket) / len(bucket))
        absolute = sum(abs(value) for value in bucket) / len(bucket)
        crossings = sum((left < 0) != (right < 0) for left, right in zip(bucket, bucket[1:]))
        frames.append([
            round(rms),
            round(absolute),
            round(crossings * 1_000_000 / max(1, len(bucket) - 1)),
            max(abs(value) for value in bucket),
        ])
    return frames, len(samples)


def ema_frames(data):
    rows = []
    for line in data.decode("ascii").splitlines()[1:]:
        fields = line.split()
        if not fields or fields[0] != "#" or len(fields) != 13:
            raise ValueError("unexpected PB2007 EMA row")
        rows.append([float(value) for value in fields[1:]])
    columns = list(zip(*rows))
    reduced = [means(column) for column in columns]
    # Tongue tip/body/back x/z. Preserve physical values as microunits.
    selected = [1, 2, 3, 7, 8, 9]
    frames = [
        [round(reduced[coordinate][frame] * 1_000_000) for coordinate in selected]
        for frame in range(BINS)
    ]
    return frames, len(rows)


def labels(data, sample_count):
    result = []
    duration_100ns = sample_count * 10_000_000 // 16_000
    for line in data.decode("ascii").splitlines():
        start, end, label = line.split()
        result.append({
            "start_bin": min(BINS - 1, int(start) * BINS // duration_100ns),
            "end_bin": min(BINS, max(1, int(end) * BINS // duration_100ns)),
            "label": label,
        })
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("archive")
    parser.add_argument("output")
    args = parser.parse_args()
    archive = open(args.archive, "rb").read()
    if digest(archive) != ARCHIVE_SHA256:
        raise SystemExit("PB2007 archive identity mismatch")
    utterances = []
    resources = []
    with zipfile.ZipFile(io.BytesIO(archive)) as source:
        for position, item in enumerate(ITEMS):
            wav = source.read(f"_wav16/{item}.wav")
            ema = source.read(f"_seq/{item}.seq")
            lab = source.read(f"_lab/{item}.lab")
            acoustic, sample_count = acoustic_frames(wav)
            articulation, ema_count = ema_frames(ema)
            split = "train" if position < 8 else "validation" if position < 10 else "test"
            utterances.append({
                "identity": item,
                "split": split,
                "speaker_context": 0,
                "audio_sample_count": sample_count,
                "ema_sample_count": ema_count,
                "missing_mask": [False] * BINS,
                "acoustic": acoustic,
                "articulation": articulation,
                # Quarantined: the trainer's input type does not contain this field.
                "post_freeze_probe_labels": labels(lab, sample_count),
            })
            for role, data in (("audio", wav), ("ema", ema), ("labels", lab)):
                resources.append({"identity": f"{item}/{role}", "sha256": digest(data), "bytes": len(data)})
    document = {
        "schema": "conduit.tongues/pb2007-derived-slice@1",
        "source": {
            "doi": "10.5281/zenodo.6390598",
            "archive_sha256": ARCHIVE_SHA256,
            "archive_bytes": len(archive),
            "license_deposit_readme": "CC-BY-SA",
            "license_zenodo_metadata": "CC-BY-4.0",
            "citation": "Badin, Bailly, Ben Youssef, Elisei, Savariaux, Hueber; PB2007",
        },
        "derivation": {
            "identity": "sha256:pending",
            "bins_per_utterance": BINS,
            "audio_clock_hz": 16000,
            "ema_clock_hz": 100,
            "acoustic_features": ["rms", "mean-absolute", "zero-crossing-millionths", "peak"],
            "articulatory_coordinates": ["tip-x", "mid-x", "back-x", "tip-z", "mid-z", "back-z"],
            "coordinate_unit": "source-centimetre-micro-units",
            "head_correction": "source-preprocessed-unspecified",
        },
        "resources": resources,
        "utterances": utterances,
    }
    canonical = json.dumps(document, sort_keys=True, separators=(",", ":")).encode()
    document["derivation"]["identity"] = f"sha256:{digest(canonical)}"
    with open(args.output, "w", encoding="utf-8") as target:
        json.dump(document, target, indent=2, sort_keys=True)
        target.write("\n")


if __name__ == "__main__":
    main()
