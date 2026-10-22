"""Rehash original five-host media and inspect the graphical marker bytes."""
import hashlib
import json
from pathlib import Path

base = Path(__file__).resolve().parent.parent
packet = base / 'work/6179-ce688-normal-preserved'
proof = json.loads((packet / 'graphical-profile-proof.json').read_text())
source = 'ce688e5889d4ee18911fee3b5152c76bf92d9995'
assert proof['source_commit'] == source
markers = [b'CONDUIT_GRAPHICAL_PROFILE'] + [
    ('CONDUIT_GRAPHICAL_ASSET_' + kind + '_ATLAS').encode()
    for kind in ['BODY', 'HEADING', 'TITLE', 'CODE']
]
directories = {
    'conduitos/x86_64/pc': 'x86_64-pc',
    'conduitos/ia32/pc': 'ia32-pc',
    'conduitos/aarch64/virt': 'aarch64-virt',
    'conduitos/riscv64/virt': 'riscv64-virt',
    'conduitos/loongarch64/virt': 'loongarch64-virt',
}
expected = {r['host']: r['image_sha256'] for r in proof['headless_images']}
assert set(expected) == set(directories) - {'conduitos/x86_64/pc'}
expected['conduitos/x86_64/pc'] = proof['image_sha256']
rows = []
for host, directory in directories.items():
    image_dir = packet / 'live-images' / directory
    manifest = json.loads((image_dir / 'build-manifest.json').read_text())
    assert manifest['source_identity'] == source
    assert manifest['artifact_role'] == 'product-host'
    image = image_dir / manifest['image']['file']
    assert image.resolve().is_relative_to(image_dir.resolve())
    raw = image.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    assert digest == manifest['image']['sha256'] == expected[host]
    present = [marker.decode() for marker in markers if marker in raw]
    if host == 'conduitos/x86_64/pc':
        assert len(present) == len(markers)
    else:
        assert not present
    rows.append({'host': host, 'bytes': len(raw), 'image_sha256': digest,
                 'present_markers': present})
receipt = {'status': 'verified', 'source': source, 'images': rows,
           'scope': 'Original image bytes and graphical asset markers; non-x86 runtime untested.'}
(base / 'outputs/6179-ce688-root-image-assets.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps({'status': 'verified', 'images': len(rows)}))
