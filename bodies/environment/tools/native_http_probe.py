"""Finite supplemental HTTP evidence; never a Body owner or UART consumer."""
import hashlib
import http.client
import json
import secrets
import threading
import time
import urllib.parse


class HttpProbe:
    def __init__(self, capture):
        self.capture = capture
        self.stopped = threading.Event()
        self.thread = threading.Thread(target=self.observe, name='native-http-proof')
        self.failure = None
        self.rows = []
        self.submitted = False

    def start(self):
        self.thread.start()

    def stop(self):
        self.stopped.set()
        self.thread.join(timeout=12)
        if self.thread.is_alive():
            raise RuntimeError('HTTP observer did not retire')

    @staticmethod
    def exchange(method, path, body=None):
        connection = http.client.HTTPConnection('192.168.4.1', 80, timeout=3)
        try:
            headers = {} if body is None else {'Content-Type': 'application/x-www-form-urlencoded'}
            connection.request(method, path, body, headers)
            response = connection.getresponse()
            data = response.read(4097)
            if len(data) > 4096:
                raise RuntimeError('HTTP proof response bound')
            return response.status, data
        finally:
            connection.close()

    def submit(self):
        try:
            release = json.loads((self.capture / 'owner-release.json').read_text())
        except (FileNotFoundError, json.JSONDecodeError):
            return  # The owner may still be writing its release receipt.
        assert release['control_applied'] and release['acknowledgement']['state'] == 'Released'
        assert not (self.capture / 'owner-quiesced.json').exists()
        status, page = self.exchange('GET', '/network')
        assert status == 200 and b'method=post action=/network' in page
        password = ('fixture-only-' + secrets.token_hex(8)).encode()
        body = urllib.parse.urlencode({'ssid': 'conduit-absent-' + secrets.token_hex(8),
                                       'password': password.decode()}).encode()
        # Exactly one POST, no retry. Never retain or print credential bytes.
        self.submitted = True
        status, response = self.exchange('POST', '/network', body)
        assert status == 202 and password not in response
        report = {'proof_class': 'concurrent-private-network-submission',
                  'completed': True, 'real_network_credentials_used': False,
                  'submitted': True, 'setup_page_status': 200, 'post_status': status,
                  'setup_page_sha256': hashlib.sha256(page).hexdigest(),
                  'fixture_sha256': hashlib.sha256(body).hexdigest(),
                  'reservation_offer_generation': release['reservation']['offer_generation'],
                  'boot_id': release['reservation']['active']['boot_id']}
        (self.capture / 'live-network-submission.json').write_text(json.dumps(report, indent=2) + '\n')

    def observe(self):
        deadline = time.monotonic() + 360
        try:
            with (self.capture / 'http-observations.jsonl').open('w') as log:
                for _ in range(180):
                    if self.stopped.is_set() or time.monotonic() >= deadline:
                        return
                    if not self.submitted and (self.capture / 'owner-release.json').exists():
                        self.submit()
                    row = {'host_time_ns': time.time_ns()}
                    try:
                        status, data = self.exchange('GET', '/reading')
                        row['status'] = status
                        if data:
                            row['body'] = json.loads(data)
                    except (OSError, http.client.HTTPException, ValueError) as error:
                        row['error'] = type(error).__name__
                    self.rows.append(row)
                    log.write(json.dumps(row) + '\n')
                    log.flush()
                    self.stopped.wait(2)
        except Exception as error:
            # Class only: transport exceptions can contain request material.
            self.failure = type(error).__name__

    def finish(self):
        self.stop()
        assert self.failure is None and self.submitted, 'HTTP/fixture observer failed'
        faces = {}
        for path in self.capture.glob('native-source-face-*.json'):
            face = json.loads(path.read_text())
            faces[face['identity']] = face
        matched = {}
        successes = []
        for row in self.rows:
            if row.get('status') != 200:
                continue
            body = row['body']
            face = faces[body['face_identity']]
            assert face['revision'] == body['face_revision'] == body['sequence']
            assert body['sampled_at_boot_ms'] is None and body['observed_at_boot_ms'] is not None
            assert all(body[key] for key in ('fresh', 'current', 'ap_ready'))
            assert body['status'] == 'available'
            matched[body['sequence']] = body['face_identity']
            successes.append(row)
        assert len(faces) == 3 and set(matched) == {1, 2, 3}, 'HTTP missed a native revision'
        first = min(row['host_time_ns'] for row in successes)
        assert any(row.get('status') == 503 and row['host_time_ns'] < first for row in self.rows)
        final = json.loads((self.capture / 'final-root-offer.json').read_text())
        assert final['network_configuration'] == 'access-point-restored'
        assert final['network_control_dispatches'] == [1, 1, 0]
        assert final['network_control_failure_reason'] == 'rejected'
        assert final['network_control_failure_phase'] == 'associate'
        fixture = json.loads((self.capture / 'live-network-submission.json').read_text())
        assert final['advertisement']['boot_id'] == fixture['boot_id'] == final['network_control_binding']['boot_id']
        for sequence in range(1, 4):
            sample = json.loads((self.capture / f'physical-observation-{sequence}.json').read_text())
            assert sample['setup_status'] == 'awaiting-native-retirement'
        assert not final['network_control_receipt']
        report = {'proof_class': 'finite native Face HTTP correlation and rejected fixture fallback',
                  'completed': True, 'matched_native_face_identities': matched,
                  'http_successes': len(successes), 'unavailable_before_publication': True,
                  'successful_station_join_proven': False, 'browser_body_membership': False}
        (self.capture / 'http-correlation.json').write_text(json.dumps(report, indent=2) + '\n')
