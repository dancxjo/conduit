"""Maintenance-only atlas generation; product builds consume the pinned bytes.

Inputs: the three exact DejaVu 2.37 TTF files and the reviewed Unifont corpus.
Output: sorted fixed records (codepoint, advance, x bearing, width, height,
coverage offset), followed by 8-bit coverage. No font engine ships at runtime.
"""
import hashlib
import json
from pathlib import Path
import struct
import sys
from PIL import Image, ImageDraw, ImageFont, __version__

root = Path(__file__).resolve().parent
repo = root.parents[3]
font_dir = Path(sys.argv[1])
expected = {
    'DejaVuSans.ttf': 'ae7b7855e115a5966d8b1b3f80f254ccc117ec86f9965e202ee2940453837280',
    'DejaVuSans-Bold.ttf': '5c1247acef7f2b8522a31742c76d6adcb5569bacc0be7ceaa4dc39dd252ce895',
    'DejaVuSansMono.ttf': 'c805f9436dbc268644c1d9584f01a601a653e028e08fd74b9b949f6cf8304d88',
}
for source, digest in expected.items():
    assert hashlib.sha256((font_dir / source).read_bytes()).hexdigest() == digest, source
corpus = repo / 'products/patchbay/native/assets/unifont/unifont-17.0.04-patchbay.hex'
points = sorted({int(line.split(':')[0], 16) for line in corpus.read_text().splitlines()})
manifest = {'pillow': __version__, 'corpus_sha256': hashlib.sha256(corpus.read_bytes()).hexdigest(), 'atlases': []}
for name, source, size, height in [('body', 'DejaVuSans.ttf', 14, 18), ('heading', 'DejaVuSans-Bold.ttf', 18, 24), ('title', 'DejaVuSans.ttf', 26, 34), ('code', 'DejaVuSansMono.ttf', 13, 18)]:
    path = font_dir / source
    font = ImageFont.truetype(str(path), size, layout_engine=ImageFont.Layout.BASIC)
    missing = bytes(font.getmask(chr(0x10ffff)))
    records, pixels = [], bytearray()
    for cp in points:
        ch = chr(cp)
        if bytes(font.getmask(ch)) == missing:
            continue
        left, top, right, bottom = font.getbbox(ch)
        width = max(1, right - left)
        assert bottom <= height and -128 <= left <= 127
        image = Image.new('L', (width, height))
        ImageDraw.Draw(image).text((-left, 0), ch, font=font, fill=255)
        advance = round(font.getlength(ch))
        if 0x300 <= cp <= 0x36f:
            advance = 0
        records.append(struct.pack('<IBbBBI', cp, advance, left, width, height, len(pixels)))
        pixels.extend(image.tobytes())
    data = struct.pack('<I', len(records)) + b''.join(records) + pixels
    (root / (name + '.atlas')).write_bytes(data)
    manifest['atlases'].append({'name': name, 'source': source, 'source_sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'size': size, 'height': height, 'glyphs': len(records), 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
(root / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
