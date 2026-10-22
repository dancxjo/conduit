import hashlib, json, struct, sys, zlib
from pathlib import Path
base = Path(sys.argv[1])
manifest = json.loads((base / 'manifest.json').read_text())
for checkpoint in manifest['checkpoints']:
    frame = checkpoint['frame']
    encoded = (base / frame['png']).read_bytes()
    assert len(encoded) == frame['png_bytes']
    assert hashlib.sha256(encoded).hexdigest() == frame['png_sha256']
    offset, parts = 8, []
    while offset < len(encoded):
        length = struct.unpack('>I', encoded[offset:offset+4])[0]
        kind, payload = encoded[offset+4:offset+8], encoded[offset+8:offset+8+length]
        offset += length + 12
        if kind == b'IHDR': width, height, depth, color, *_ = struct.unpack('>IIBBBBB', payload)
        if kind == b'IDAT': parts.append(payload)
    assert depth == 8 and color == 6
    assert (width, height) == (frame['width'], frame['height'])
    raw, stride, offset = zlib.decompress(b''.join(parts)), width * 4, 0
    previous, rgba = bytearray(stride), bytearray()
    for _ in range(height):
        filtering, row = raw[offset], bytearray(raw[offset+1:offset+1+stride])
        offset += stride + 1
        for index in range(stride):
            left = row[index-4] if index >= 4 else 0
            upper_left = previous[index-4] if index >= 4 else 0
            above = previous[index]
            if filtering == 0: prediction = 0
            elif filtering == 1: prediction = left
            elif filtering == 2: prediction = above
            elif filtering == 3: prediction = (left + above) // 2
            else:
                assert filtering == 4
                estimate = left + above - upper_left
                distances = [abs(estimate-left), abs(estimate-above), abs(estimate-upper_left)]
                prediction = [left, above, upper_left][distances.index(min(distances))]
            row[index] = (row[index] + prediction) & 255
        rgba.extend(row)
        previous = row
    assert hashlib.sha256(rgba).hexdigest() == frame['pixel_sha256']
print(json.dumps({'status': 'passed', 'captures': len(manifest['checkpoints']), 'checks': ['PNG length', 'PNG SHA256', 'dimensions', 'independent stdlib PNG decode', 'RGBA SHA256']}))
