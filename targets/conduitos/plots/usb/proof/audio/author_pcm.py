#!/usr/bin/env python3
"""Author finite independent PCM input; never derive expected samples from USB output."""
import argparse
import json
import struct


def author(blocks=1, clock_id=73, start_frame=0):
    if not 1 <= blocks <= 16 or not 1 <= clock_id <= (1 << 64)-1:
        raise ValueError('finite block count or clock identity outside bounds')
    if not 0 <= start_frame <= (1 << 64)-1-blocks*384:
        raise ValueError('sample frame extent overflows')
    authored, original_samples, original = [], [], bytearray()
    for block in range(blocks):
        samples = []
        for frame in range(384):
            left = ((block*384+frame)*73) % 40001-20000
            samples.append([left, -left])
        pcm = b''.join(struct.pack('<hh', left, right) for left, right in samples)
        first = start_frame+block*384
        segments = [pcm[position*192:(position+1)*192] for position in range(8)]
        assert all(len(segment) == 192 for segment in segments)
        assert b''.join(segments) == pcm
        authored.append({
            'header': {'representation': 'signed16-little-endian', 'sample-rate-hz': 48000,
                       'layout': 'stereo-left-right', 'frame-count': 384, 'clock-id': clock_id,
                       'start-frame': first, 'discontinuity': False, 'payload-bytes': 1536},
            'cadence': {'phase': (first//48) % 1000, 'sample_rate': 48000,
                        'channels': 2, 'sample_bytes': 2},
            'segments': [{'wire_hex': segment.hex()} for segment in segments],
        })
        original_samples.extend(samples)
        original.extend(pcm)
    return {'authored_blocks': authored, 'original_samples': original_samples,
            'original_pcm_hex': original.hex(), 'clock_id': clock_id,
            'start_frame': start_frame, 'sample_rate_hz': 48000, 'channels': 2,
            'representation': 'signed16-little-endian'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    parser.add_argument('--blocks', type=int, default=1, choices=range(1,17))
    parser.add_argument('--clock-id', type=int, default=73)
    parser.add_argument('--start-frame', type=int, default=0)
    args = parser.parse_args()
    value = author(args.blocks, args.clock_id, args.start_frame)
    with open(args.output, 'x', encoding='utf8') as output:
        json.dump(value, output, separators=(',', ':'))
        output.write('\n')


if __name__ == '__main__':
    main()
