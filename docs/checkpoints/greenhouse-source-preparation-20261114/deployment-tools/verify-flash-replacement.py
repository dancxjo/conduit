"""Verify complete flash readback against explicit, sector-aligned replacements.

Read-only. Every byte outside the supplied erase ranges must remain unchanged.
Each replacement is OFFSET:ERASE_LENGTH:FILE, parsed with base-0 integers.
"""
import argparse
import hashlib
import json
from pathlib import Path


def verify(before, after, replacements, sector_size):
    if len(before) != len(after):
        raise ValueError('flash lengths differ')
    expected = bytearray(before)
    occupied = []
    for offset, erase_length, content in replacements:
        end = offset + erase_length
        if (offset < 0 or erase_length <= 0 or offset % sector_size
                or erase_length % sector_size or end > len(before)
                or len(content) > erase_length):
            raise ValueError('invalid replacement extent')
        if any(offset < right and left < end for left, right in occupied):
            raise ValueError('overlapping replacement extents')
        occupied.append((offset, end))
        expected[offset:end] = b'\xff' * erase_length
        expected[offset:offset + len(content)] = content
    if after != expected:
        raise ValueError('readback differs from exact approved replacement bytes')
    return {'schema': 'conduit.operator/exact-flash-readback@1',
            'flash_bytes': len(before),
            'before_sha256': hashlib.sha256(before).hexdigest(),
            'after_sha256': hashlib.sha256(after).hexdigest(),
            'replacement_extents': occupied,
            'all_other_bytes_unchanged': True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('before', type=Path)
    parser.add_argument('after', type=Path)
    parser.add_argument('--sector-size', type=int, default=4096)
    parser.add_argument('--replace', action='append', required=True)
    args = parser.parse_args()
    if args.sector_size <= 0:
        parser.error('sector size must be positive')
    replacements = []
    for item in args.replace:
        offset, length, path = item.split(':', 2)
        replacements.append((int(offset, 0), int(length, 0), Path(path).read_bytes()))
    print(json.dumps(verify(args.before.read_bytes(), args.after.read_bytes(),
                            replacements, args.sector_size), sort_keys=True))


if __name__ == '__main__':
    main()
