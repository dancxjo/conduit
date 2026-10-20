#!/usr/bin/env python3
"""Verify actual normal-owner replacement/retry receipts and immutable digests."""
import gzip
import hashlib
import json
from pathlib import Path
import sys

CASES = {'durable-normal-owner-replacement-history', 'owner-terminal-publication-retry-without-replay', 'normal-owner-custody-capacity-before-effects'}

def require(value, message):
    if not value:
        raise ValueError(message)

def verify(log):
    rows = []
    for line in log.splitlines():
        start = line.find('{')
        if start < 0:
            continue
        try:
            row, _ = json.JSONDecoder().raw_decode(line[start:])
        except ValueError:
            continue
        if isinstance(row, dict):
            rows.append(row)
    rows = [row for row in rows if row.get('case') in CASES]
    require(len(rows) == 3 and {row['case'] for row in rows} == CASES, 'missing or duplicated normal owner proof')
    for row in rows:
        if 'history' not in row:
            continue
        history = row['history']
        require(history['archive_records_committed'] is True, 'receipts were not durably archived')
        require(history['legacy_prefix_unavailable'] is False, 'proof has an unavailable historical prefix')
        require(len(history['receipts']) == len(history['receipt_digests']) <= history['capacity'] == 64, 'history capacity/index differs')
        for receipt, fingerprint in zip(history['receipts'], history['receipt_digests']):
            record = {'schema':'conduit.body/execution-custody@1', 'host_id':history['custodian_host_id'], 'body_id':history['body_id'], 'receipt':receipt}
            data = json.dumps(record, separators=(',', ':'), ensure_ascii=False).encode()
            require('sha256:' + hashlib.sha256(data).hexdigest() == fingerprint, 'immutable receipt fingerprint differs')
    rows = {row['case']: row for row in rows}
    replacement = rows['durable-normal-owner-replacement-history']
    old, new, history = replacement['first_receipt'], replacement['latest_receipt'], replacement['history']
    require(old in history['receipts'] and new in history['receipts'], 'replacement erased an old or new receipt')
    require(old != new and old['read_play']['active_play_id'] != new['read_play']['active_play_id'], 'replacement reused old Play')
    require(old['read_plan_id'] != new['read_plan_id'], 'replacement reused old Plan')
    require(old['read_terminal_sign']['sign_id'] != new['read_terminal_sign']['sign_id'], 'replacement reused terminal Sign')
    require(old['restored_fore_sha256'] != new['restored_fore_sha256'], 'distinct state outputs lost their fingerprints')
    require(old['body_id'] == new['body_id'] == history['body_id'] == replacement['body_id'], 'replacement changed Body identity')
    retry = rows['owner-terminal-publication-retry-without-replay']
    receipt = retry['receipt']
    require(retry['history']['receipts'] == [receipt], 'publication retry duplicated or lost execution')
    require(receipt['committed_fore_count'] == 1, 'checkpoint effect was duplicated or absent')
    require(receipt['terminal_sign']['active_play_id'] == receipt['play']['active_play_id'], 'terminal custody rebound to another Play')
    require(len(retry['checkpoint_files']) == 2 and all(file['sha256'].startswith('sha256:') and len(file['sha256']) == 71 for file in retry['checkpoint_files']), 'checkpoint file evidence differs')
    capacity = rows['normal-owner-custody-capacity-before-effects']
    require(capacity['fixture_receipts'] is True, 'capacity metadata fixture was misrepresented as execution')
    require(capacity['capacity'] == capacity['retained_count'] == 64, 'declared profile did not fill')
    require(capacity['host_started'] is False and capacity['checkpoint_files'] == 0, 'capacity refusal allowed effects')
    require('before Play' in capacity['refusal'], 'capacity refusal lost its admission boundary')
    return list(rows.values())

if __name__ == '__main__':
    path = Path(sys.argv[1])
    raw = gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes()
    try:
        result = verify(raw.decode())
    except (ValueError, KeyError, IndexError, TypeError) as error:
        sys.exit(f'FAIL: {error}')
    print(f'PASS: {len(result)} normal owner custody/recovery cases and immutable receipt digests')
