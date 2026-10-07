#!/usr/bin/env python3
"""Serve one actual parser event file for contemporaneous local inspection."""
import argparse
import hashlib
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import time

from revision_inspector import HTML

MAXIMUM_BYTES = 32 * 1024 * 1024


def producer_state(pid, started=None):
    try:
        # A retained PID or process name alone is not completion evidence.
        fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
        if started is not None and fields[19] != started:
            return 'stopped'
        return 'stopped' if fields[0] in ('Z', 'X') else 'running'
    except FileNotFoundError:
        return 'stopped'
    except (OSError, IndexError):
        return 'unknown'


def snapshot(trace, pid, started=None):
    try:
        raw = trace.read_bytes()
    except FileNotFoundError:
        raw = b''
    if len(raw) > MAXIMUM_BYTES:
        raise ValueError('Event file exceeds 32 MiB')
    # An incomplete append is not an event. A malformed complete append refuses
    # the observation; the browser retains its last valid evidence.
    complete = raw[:raw.rfind(b'\n') + 1]
    complete.decode('utf-8')
    events, prefixes = [], []
    prefix = hashlib.sha256()
    for line in complete.splitlines(keepends=True):
        prefix.update(line)
        if line.strip():
            event = json.loads(line)
            if not isinstance(event, dict) or not isinstance(event.get('event'), str):
                raise ValueError('Expected JSONL event records')
            events.append(event)
            prefixes.append(prefix.hexdigest())
    return {
        'source': trace.name,
        'sha256': hashlib.sha256(raw).hexdigest(),
        'events': events,
        'prefix_sha256': prefixes,
        'producer_state': producer_state(pid, started),
        'observed_at_ms': int(time.time() * 1000),
    }


FOLLOW = r'''<script>
// This endpoint reads the real producer file. It does not execute a parser,
// append events, or infer linguistic truth from process liveness.
let liveReading=false,livePreviousCount=0;
async function observeProducer(){
 if(liveReading)return;liveReading=true;
 try{
  const response=await fetch('/events',{cache:'no-store'});
  if(!response.ok)throw Error(await response.text());
  const next=await response.json();
  const previousCount=evidence.events.length;
  if(next.events.length<previousCount || (previousCount && next.prefix_sha256[previousCount-1]!==evidence.prefix_sha256[previousCount-1]))throw Error('Producer history changed; retaining previous evidence');
  const atEnd=Number($('event').value)===Math.max(0,evidence.events.length-1);
  const oldIndex=Number($('event').value);
  evidence=next;
  if(next.events.length){
   $('event').max=next.events.length-1;
   $('event').value=atEnd?next.events.length-1:Math.min(oldIndex,next.events.length-1);
   render();
  }
  $('status').textContent=`Producer ${next.producer_state} · ${next.events.length} retained events · ${next.events.length>livePreviousCount?'new events observed':'waiting for new events'}`;
  $('error').textContent='';
  livePreviousCount=next.events.length;
  window.liveObservation={count:next.events.length,sha256:next.sha256,producer_state:next.producer_state,observed_at_ms:next.observed_at_ms};
 }catch(error){$('error').textContent=error.message}
 finally{liveReading=false}
}
// The local producer controls this view. Recorded-file controls remain in the
// standalone recorded inspector, where they cannot imply live execution.
for(const id of ['reload','follow','stop'])$(id).hidden=true;
observeProducer();setInterval(observeProducer,1000);
</script>'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('trace', type=Path)
    parser.add_argument('--producer-pid', type=int, required=True)
    parser.add_argument('--port', type=int, default=0)
    args = parser.parse_args()
    if args.producer_pid <= 0:
        parser.error('producer PID must be positive')
    try:
        started = Path(f'/proc/{args.producer_pid}/stat').read_text().rsplit(')', 1)[1].split()[19]
    except (OSError, IndexError):
        parser.error('producer process must be observable at startup')
    initial = {'source': args.trace.name, 'sha256': '', 'events': [], 'prefix_sha256': []}
    html = HTML.replace('__EVIDENCE__', json.dumps(initial).replace('<', '\\u003c')).replace(
        'This is a recorded execution trace. Playback controls change the inspected event; they do not run the parser or reproduce its timing.',
        'This view observes events flushed by the selected local parser producer. Process status describes liveness; only the retained events provide linguistic evidence.',
    ).replace('Recorded event <span', 'Producer event <span').replace(
        '<th>Score</th><th>Choices</th>', '<th>Score</th><th>Selected prefix</th><th>Choices</th>',
    ).replace(
        '[c.identity,c.active,c.score,c.choices', '[c.identity,c.active,c.score,c.selected,c.choices',
    ).replace(
        'Choices and heads are native profile ordinals.',
        'Choices and heads are native profile ordinals. Choices beyond the selected prefix do not establish lexical selections.',
    ).replace(
        "'Recorded trace'}", "(evidence.producer_state?'Producer '+evidence.producer_state:'Recorded trace')}",
    ).replace('Elapsed in original run (ms)', 'Elapsed in producer run (ms)').replace(
        'Recorded file:', 'Producer file:',
    ).replace(
        'x=evidence.events[i];if(!x)return;',
        'original=evidence.events[i];if(!original)return;const x={...original,...(original.receipt??{})};',
    ).replace(
        "['Stable',x.stable]",
        "['Syntax stable',x.stable],['Text stable prefix',x.stable_source_prefix]",
    ).replace(
        "install();\n</script>", "$('status').textContent='Waiting for producer events';\n</script>"
    ).replace('</html>', FOLLOW + '</html>').encode()

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path == '/':
                status, body, content_type = 200, html, 'text/html; charset=utf-8'
            elif self.path == '/events':
                try:
                    body = json.dumps(snapshot(args.trace, args.producer_pid, started)).encode()
                    status, content_type = 200, 'application/json'
                except (OSError, ValueError, UnicodeError) as error:
                    status, body, content_type = 422, str(error).encode(), 'text/plain'
            else:
                status, body, content_type = 404, b'Not found', 'text/plain'
            self.send_response(status)
            self.send_header('Content-Type', content_type)
            self.send_header('Cache-Control', 'no-store')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_):
            pass

    server = ThreadingHTTPServer(('127.0.0.1', args.port), Handler)
    print(json.dumps({'url': f'http://127.0.0.1:{server.server_port}',
                      'trace': str(args.trace), 'producer_pid': args.producer_pid}), flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == '__main__':
    main()
