#!/usr/bin/env python3
"""Check external UAC PCM and decoded native ISO evidence; never manufacture it."""
import argparse
import json
import hashlib
from pathlib import Path
import struct

def check(transcript, receipt, authored, authored_sha256):
    packets, controls = [], []
    for entry in transcript:
        raw = bytes.fromhex(entry['wire_hex'])
        header = 12 if entry['type'] == 0 else 16
        if len(raw) < header:
            raise ValueError('truncated usbredir record')
        kind, count = struct.unpack_from('<II', raw)
        if kind != entry['type'] or count != len(raw)-header:
            raise ValueError('usbredir extent mismatch')
        body = raw[header:]
        if entry['direction'] == 'qemu-to-device' and kind == 100:
            if len(body) < 10: raise ValueError('short control')
            endpoint, request, request_type, _, value, index, length = struct.unpack_from('<BBBBHHH', body)
            controls.append((request_type, request, value, index, length, body[10:]))
        if entry['direction'] == 'qemu-to-device' and kind == 102:
            if len(body) != 196 or body[:1] != b'\x01' or int.from_bytes(body[2:4],'little') != 192:
                raise ValueError('wrong ISO endpoint or exact PCM extent')
            packets.append(body[4:])
    if not 72 <= len(packets) <= 128:
        raise ValueError('at least72 actual packets required to cross64TRB ring reuse')
    if (0x22,1,0x100,1,3,bytes.fromhex('80bb00')) not in controls:
        raise ValueError('missing exact endpoint SET_CUR48000')
    if (0xa2,0x81,0x100,1,3,b'') not in controls:
        raise ValueError('missing endpoint GET_CUR48000')
    if receipt['sample_rate_hz'] != 48000 or receipt['channels'] != 2 or receipt['representation'] != 'signed16-little-endian':
        raise ValueError('PCM profile mismatch')
    if receipt['configuration'] != 1 or receipt['interface'] != 1 or receipt['alternate'] != 1 or receipt['endpoint'] != 1:
        raise ValueError('selected topology mismatch')
    if receipt['clock_id'] <= 0 or receipt['start_frame'] < 0:
        raise ValueError('missing original PCM clock/start')
    if receipt['authored_input_sha256'] != authored_sha256:
        raise ValueError('authored input digest mismatch')
    for field in ['clock_id', 'start_frame', 'sample_rate_hz', 'channels', 'representation']:
        if receipt[field] != authored[field]:
            raise ValueError('original PCM metadata differs from authored input')
    expected = bytes.fromhex(authored['original_pcm_hex'])
    samples = authored['original_samples']
    if len(samples) != len(packets)*48 or any(len(pair) != 2 or any(not isinstance(sample, int) or not -32768 <= sample <= 32767 for sample in pair) for pair in samples):
        raise ValueError('original signed16 stereo sample extent mismatch')
    if b''.join(struct.pack('<hh', *pair) for pair in samples) != expected:
        raise ValueError('source PCM little endian byte order mismatch')
    if bytes.fromhex(receipt['original_pcm_hex']) != expected:
        raise ValueError('receipt substituted original PCM')
    segments = [bytes.fromhex(segment['wire_hex']) for block in authored['authored_blocks'] for segment in block['segments']]
    if any(len(segment) != 192 for segment in segments) or b''.join(segments) != expected:
        raise ValueError('authored PCM projections omit duplicate or alter original bytes')
    if len(expected) != len(packets)*192 or b''.join(packets) != expected or not any(expected):
        raise ValueError('external bytes differ from original nontrivial PCM projection')
    batches=receipt['native_batches']
    if len(batches)*8 != len(packets):raise ValueError('native completion count mismatch')
    prior=None
    for batch in batches:
        if batch['outcome'] != 'completed':
            raise ValueError('native outcome remains '+batch['outcome'])
        completions=batch['packets']
        if len(completions)!=8:raise ValueError('incomplete batch is not completion')
        for completion in completions:
            frame=completion['frame']
            if not 0<=frame<2048 or completion['requested']!=192 or completion['actual']!=192:
                raise ValueError('native exact packet completion mismatch')
            if prior is not None and frame != (prior+1)%2048:
                raise ValueError('service gap between explicit native frames')
            prior=frame
    if receipt['terminal'] != 'drained' or not receipt['original_stop_acknowledged'] or not receipt['possessions_retired']:
        raise ValueError('original drain/stop/possession retirement missing')
    return {'packets':len(packets),'pcm_bytes':len(expected),'ring_reuse':True,
            'native_terminal':'drained','audible_output_claimed':False}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--transcript',required=True)
    parser.add_argument('--receipt',required=True)
    parser.add_argument('--authored-input',required=True)
    args=parser.parse_args()
    for name, maximum in [(args.transcript, 1048576), (args.receipt, 262144), (args.authored_input, 262144)]:
        if Path(name).stat().st_size > maximum:
            raise ValueError('finite evidence file extent exceeded')
    authored_bytes = Path(args.authored_input).read_bytes()
    entries=[json.loads(line) for line in Path(args.transcript).read_text().splitlines() if line]
    print(json.dumps(check(entries,json.loads(Path(args.receipt).read_text()),json.loads(authored_bytes),hashlib.sha256(authored_bytes).hexdigest()),sort_keys=True))

if __name__=='__main__':main()
