"""One bounded local TCP peer for the ECM emulator proof; no USB/frame code."""
import argparse
import json
import socket
import time
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--ready', required=True)
parser.add_argument('--receipt', required=True)
parser.add_argument('--full-mtu', action='store_true')
args = parser.parse_args()
request = bytes(index % 256 for index in range(458))
response = bytes((255-index) % 256 for index in range(74))
if args.full_mtu:
    request = bytes(i % 256 for i in range(1460))
    response = bytes((255-i) % 256 for i in range(1460))
deadline = time.monotonic() + 120
with socket.socket() as listener:
    listener.bind(('127.0.0.1', 0))
    listener.listen(1)
    listener.settimeout(max(.001, deadline-time.monotonic()))
    ready = {'host':'127.0.0.1', 'port':listener.getsockname()[1],
             'request':list(request), 'response':list(response)}
    ready_path = Path(args.ready)
    temporary_ready = ready_path.with_suffix('.pending')
    with temporary_ready.open('x') as output:
        json.dump(ready, output)
    temporary_ready.replace(ready_path)
    connection, address = listener.accept()
    with connection:
        received = bytearray()
        while len(received) < len(request):
            connection.settimeout(max(.001, deadline-time.monotonic()))
            chunk = connection.recv(len(request)-len(received))
            if not chunk:
                raise ValueError('TCP peer closed before exact request')
            received.extend(chunk)
        if bytes(received) != request:
            raise ValueError('TCP request differs from admitted bytes')
        connection.sendall(response)
        connection.settimeout(max(.001, deadline-time.monotonic()))
        if connection.recv(1):
            raise ValueError("unexpected request bytes after the exact bounded transaction")
        with open(args.receipt, 'x') as output:
            json.dump({'outcome':'exact-request-received-response-sent-client-eof-observed',
                       'request_hex':received.hex(), 'response_hex':response.hex(),
                       'local_port':listener.getsockname()[1],
                       'scope':'host TCP peer only; no guest receipt claim'}, output)
