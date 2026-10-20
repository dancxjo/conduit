#!/usr/bin/env python3
"""Inspect retained actual Body/kernel replacement records; no timing or release claim."""
import gzip
import hashlib
import json
from pathlib import Path
import sys

CASES = {
    "uncertain-old-Fore-new-Plan-and-Play",
    "accepted-output-drained-before-cancelled-replacement",
    "selected-provider-loss-independent-live-Play",
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify(log):
    records = [json.loads(line) for line in log.splitlines() if line.startswith('{')]
    records = [row for row in records if "case" in row]
    require(len(records) == 3 and {r['case'] for r in records} == CASES, "missing or duplicate lifecycle case")
    require(any(line.startswith('test result: ok. 3 passed; 0 failed;') for line in log.splitlines()), "missing successful test result")
    for row in records:
        old, new = row['old_body_plan'], row['replacement_body_plan']
        require(old['plan_id'] == row['old_plan'], "old Body Plan custody differs")
        require(new['plan_id'] == row['replacement_plan'], "replacement Body Plan custody differs")
        require(old['body_id'] == new['body_id'] and old['wake_id'] == new['wake_id'], "Body/Wake identity changed")
        require(old['plan_id'] != new['plan_id'] and row['old_play'] != row['replacement_play'], "replacement reused Plan/Play")
        require(row['old_terminal_sign'] != row['replacement_terminal_sign'], "replacement reused terminal Sign")
        require(len(old['plots']) == len(new['plots']) == 1, "unexpected product workset")
        prior, candidate = old['plots'][0]['plan'], new['plots'][0]['plan']
        require(prior['plan_id'] != candidate['plan_id'], "ordinary Plan did not change")
        for key in ['source_document_id', 'checked_plot_id', 'expanded_plot_id']:
            require(prior[key] == candidate[key], f"logical Source changed: {key}")
        require(prior['fragments'][0]['boot_id'] != candidate['fragments'][0]['boot_id'], "replacement did not bind its fresh Boot")
        for prefix in ['old', 'replacement']:
            report = row[f'{prefix}_report']
            for key in ['plan', 'play', 'terminal_sign']:
                require(row[f'{prefix}_{key}'] in report, f"{prefix} report lost {key}")
            require('cleanup_failure: None' in report, f"{prefix} cleanup failed")
    by_case = {r['case']: r for r in records}
    unknown = by_case['uncertain-old-Fore-new-Plan-and-Play']
    pending, payload = unknown['old_pending_output'], bytes(unknown['old_output_bytes'])
    require(pending['stage'] == 'adapter-outcome-unknown', "external uncertainty was erased")
    require(pending['byte_length'] == len(payload) and pending['sha256'] == hashlib.sha256(payload).hexdigest(), "pending output fingerprint differs")
    require(pending['sequence'] == unknown['old_output_sequence'], "old output sequence changed")
    require(unknown['external_old_effects'] == unknown['replacement_outputs'] == 1, "output replay or missing effect")
    require(unknown['replacement_items'] == ['Buy milk', 'Buy bread'], "old command replayed or state lost")
    require(unknown['cache_old_result'] == unknown['old_body_plan']['plots'][0]['plan']['plan_id'], "old oracle result does not bind executed Plan")
    require(unknown['cache_replacement_result'] == unknown['replacement_body_plan']['plots'][0]['plan']['plan_id'], "new oracle result does not bind executed Plan")
    require(unknown['cache_replacement_invalidations'] == 1, "changed binding was not invalidated")
    require(unknown['session']['historical_plan_ids'] == [unknown['old_plan'], unknown['replacement_plan']], "planning history lost old Plan")
    claims = unknown['session']['execution_claims']
    require(len(claims) == 1 and claims[0]['play']['active_play_id'] == unknown['old_play'], "old execution claim lost")
    require(claims[0]['phase']['Terminal']['sign_id'] == unknown['old_terminal_sign'], "old claim terminal custody lost")
    cancelled = by_case['accepted-output-drained-before-cancelled-replacement']
    require(cancelled['offered_event'] < cancelled['delivered_event'] < cancelled['cancelled_event'], "delivery/cancellation order differs")
    require(cancelled['old_output_count'] == cancelled['replacement_output_count'] == 1, "cancelled output replayed or lost")
    require(cancelled['replacement_items'] == ['Buy milk', 'After cancellation'], "cancelled command replayed")
    lost = by_case['selected-provider-loss-independent-live-Play']
    require(lost['unaffected_body_id'] != lost['old_body_plan']['body_id'], "independent work reused the same Body birth")
    require(lost['unaffected_play'] not in [lost['old_play'], lost['replacement_play']], "independent Play identity conflated")
    require('HostCallFailed' in lost['old_refusal'], "actual selected provider loss not retained")
    require(lost['unaffected_outputs'] == lost['replacement_outputs'] == 1, "independent/replacement output custody differs")
    require(lost['replacement_items'] == ['New authorized work'], "failed old command replayed")
    return records


if __name__ == '__main__':
    path = Path(sys.argv[1])
    raw = gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes()
    try:
        result = verify(raw.decode())
    except (ValueError, KeyError, TypeError, IndexError) as error:
        sys.exit(f'FAIL: {error}')
    print(f'PASS: {len(result)} actual Body replacement custody cases; no physical-network, speedup or release claim')
