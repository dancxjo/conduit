#!/usr/bin/env python3
"""Extract the deterministic Patchbay glyph ranges from pinned GNU Unifont."""

import gzip
import pathlib
import sys

RANGES = (
    (0x0020, 0x007E),  # ASCII
    (0x00A0, 0x00FF),  # Latin-1 Supplement
    (0x0370, 0x03FF),  # Greek and Coptic
    (0x0400, 0x04FF),  # Cyrillic
    (0x2190, 0x21FF),  # Arrows
    (0x2500, 0x257F),  # Box Drawing
    (0x25A0, 0x25FF),  # Geometric Shapes
    (0x4E2D, 0x4E2D),  # Double-width acceptance glyph
    (0xFFFD, 0xFFFD),  # Explicit replacement glyph
)


def selected(codepoint: int) -> bool:
    return any(start <= codepoint <= end for start, end in RANGES)


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: generate_subset.py INPUT.hex.gz OUTPUT.hex")
    source, output = map(pathlib.Path, sys.argv[1:])
    lines = []
    with gzip.open(source, "rt", encoding="ascii") as glyphs:
        for line in glyphs:
            codepoint, separator, _ = line.partition(":")
            if separator and selected(int(codepoint, 16)):
                lines.append(line)
    output.write_text("".join(lines), encoding="ascii")


if __name__ == "__main__":
    main()
