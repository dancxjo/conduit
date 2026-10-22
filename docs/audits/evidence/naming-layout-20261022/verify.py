"""Standalone retained-byte verifier; no Cargo, guest or source mutation."""
from pathlib import Path
import gzip
import hashlib
import json
import re

root = Path(__file__).resolve().parent
sha = lambda value: hashlib.sha256(value).hexdigest()
for line in (root / 'SHA256SUMS').read_text().splitlines():
    digest, name = line.split('  ', 1)
    assert sha((root / name).read_bytes()) == digest, name
for row in json.loads((root / 'raw-log-custody.json').read_text()):
    packed = (root / row['file']).read_bytes()
    raw = gzip.decompress(packed)
    assert len(packed) == row['gzip_bytes'] and sha(packed) == row['gzip_sha256']
    assert len(raw) == row['raw_bytes'] and sha(raw) == row['raw_sha256']
records = []
for prefix in ['9900', 'ad089']:
    summary = json.loads((root / (prefix + '-page-costs.json')).read_text())
    raw = gzip.decompress((root / summary['raw_log']).read_bytes())
    assert sha(raw) == summary['raw_log_sha256']
    rows = []
    for line in raw.decode().splitlines():
        match = re.search(r'naming_(row|page_cost) ', line)
        if match:
            rows.append(line[match.start():])
    actual = ('\n'.join(rows) + '\n').encode()
    assert len(rows) == summary['extracted_row_and_cost_records']
    assert sha(actual) == summary['extracted_records_sha256']
    assert actual == gzip.decompress((root / (prefix + '-row-cost-records.txt.gz')).read_bytes())
    records.append(actual)
assert records[0] == records[1]
print('All retained hashes, original gzip bytes and 704 per-source row/cost records verified.')
