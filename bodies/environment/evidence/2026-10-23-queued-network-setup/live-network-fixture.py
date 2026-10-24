"""Internal private fixture during the supported native owner-run proof.
Requires the host already connected to the MCU AP. Does not use UART or retry POST.
"""
from pathlib import Path
import datetime
import hashlib
import http.client
import json
import secrets
import sys
import time
import urllib.parse
capture = Path(sys.argv[1])
release = json.loads((capture / 'owner-release.json').read_text())
assert release['control_applied'] and release['acknowledgement']['state'] == 'Released'
assert not (capture / 'owner-quiesced.json').exists(), 'requires live worker'
report = {'proof_class': 'concurrent-private-network-submission', 'completed': False,
          'real_network_credentials_used': False, 'submitted': False,
          'reservation_offer_generation': release['reservation']['offer_generation'],
          'boot_id': release['reservation']['active']['boot_id'],
          'started_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat()}
started = time.monotonic()
try:
    conn = http.client.HTTPConnection('192.168.4.1', 80, timeout=5)
    try:
        conn.request('GET', '/network')
        reply = conn.getresponse()
        page = reply.read()
        assert reply.status == 200 and b'method=post action=/network' in page
        report['setup_page_status'] = reply.status
        report['setup_page_sha256'] = hashlib.sha256(page).hexdigest()
    finally:
        conn.close()
    password = ('fixture-only-' + secrets.token_hex(8)).encode()
    body = urllib.parse.urlencode({'ssid': 'conduit-absent-' + secrets.token_hex(8),
                                 'password': password.decode()}).encode()
    report['fixture_sha256'] = hashlib.sha256(body).hexdigest()
    assert not (capture / 'owner-quiesced.json').exists(), 'worker already quiesced'
    conn = http.client.HTTPConnection('192.168.4.1', 80, timeout=5)
    try:
        conn.request('POST', '/network', body, {'Content-Type': 'application/x-www-form-urlencoded'})
        reply = conn.getresponse()
        response = reply.read()
        assert password not in response, 'fixture credential echoed in response'
        report['post_status'] = reply.status
        assert reply.status == 202, 'private submission not acknowledged'
        report['submitted'] = True
        report['completed'] = True
    finally:
        conn.close()
except Exception as error:
    report['failure'] = str(error)
finally:
    report['elapsed_seconds'] = time.monotonic() - started
    (capture / 'live-network-submission.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))
raise SystemExit(0 if report['completed'] else 1)
