#!/usr/bin/env python3
"""Render actual retained parser JSONL as a self-contained recorded-trace inspector."""
import argparse
import hashlib
import json
from pathlib import Path

HTML = r'''<!doctype html><html lang="en"><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Language revision evidence</title>
<style>
body{font:16px system-ui;margin:2rem auto;padding:0 1rem;max-width:1100px;background:#f6f7fa;color:#172234}h1{font-size:1.7rem}button,input{font:inherit}button{padding:.4rem .7rem;margin-right:.4rem}section{background:white;border:1px solid #d9dfeb;border-radius:8px;padding:1rem;margin:1rem 0}label{display:block}input[type=range]{width:100%}table{border-collapse:collapse;width:100%;font-size:.9rem}th,td{padding:.6rem;text-align:left;border-bottom:1px solid #ddd}pre{white-space:pre-wrap;overflow-wrap:anywhere;font-size:.8rem}small{color:#526078}.facts{display:flex;gap:1.5rem;flex-wrap:wrap}.facts b{display:block}.warning{border-left:5px solid #c88820}#status{font-weight:600}td{overflow-wrap:anywhere}@media(max-width:650px){body{margin:1rem auto}table{display:block;overflow:auto}}
</style>
<h1>Language revision evidence</h1>
<p>This is a recorded execution trace. Playback controls change the inspected event; they do not run the parser or reproduce its timing.</p>
<section class="warning"><p id="status"></p><small>Provisional candidates are not committed facts. Agreement, stable, and committed fields are shown exactly as recorded; missing fields remain unavailable.</small></section>
<section><label for="event">Recorded event <span id="ordinal"></span></label><input id="event" type="range" min="0" step="1"><button id="previous">Previous</button><button id="next">Next</button><button id="reload">Load another recorded JSONL</button><input id="file" type="file" accept=".jsonl,.json,application/json" hidden><p id="error" role="alert"></p></section>
<section><div class="facts" id="facts"></div><p id="policy"></p></section>
<section><h2>Retained candidates</h2><p>Choices and heads are native profile ordinals. This view does not invent a token-to-POS map or a preferred candidate when the trace omits one.</p><table><thead><tr><th>Identity</th><th>Active</th><th>Score</th><th>Choices</th><th>Heads</th><th>Relations</th><th>Unread / depth</th></tr></thead><tbody id="candidates"></tbody></table></section>
<section><h2>Identity and agreement</h2><pre id="identity"></pre></section>
<section><details><summary>Complete recorded event, including native bytes and invocation evidence</summary><pre id="raw"></pre></details></section>
<footer><small id="provenance"></small></footer>
<script>
let evidence=__EVIDENCE__;
const $=id=>document.getElementById(id);
const shown=x=>x===undefined?'unavailable':typeof x==='object'?JSON.stringify(x):String(x);
function render(){const i=Number($('event').value),x=evidence.events[i];if(!x)return;$('ordinal').textContent=`${i+1} / ${evidence.events.length}`;$('status').textContent=`Recorded ${shown(x.event)} · ${shown(x.source_revision)}`;$('facts').replaceChildren();for(const [title,value] of [['Elapsed in original run (ms)',x.actual_elapsed_ms],['Available',x.available??x.outcome?.availability?.available],['Stable',x.stable],['Committed',x.committed],['Final input',x.final_input??x.outcome?.availability?.final_input],['Admitted model invocations',x.model_invocations]]){let p=document.createElement('div'),b=document.createElement('b');b.textContent=title;p.append(b,document.createTextNode(shown(value)));$('facts').append(p)}$('policy').textContent=x.frontier_policy??'No frontier policy field in this event.';$('candidates').replaceChildren();for(const c of x.candidates??[]){let row=document.createElement('tr');for(const value of [c.identity,c.active,c.score,c.choices,c.heads,c.relations,`${shown(c.unread)} / ${shown(c.depth)}`]){let cell=document.createElement('td');cell.textContent=shown(value);row.append(cell)}$('candidates').append(row)}$('identity').textContent=JSON.stringify(Object.fromEntries(['analysis_revision','source_sequence','lexical_profile_identity','model_content_identity','model_execution','retained_executions','joint_lexical_arc_agreement','flow_invocations'].map(k=>[k,x[k]??'unavailable'])),null,2);$('raw').textContent=JSON.stringify(x,null,2);$('provenance').textContent=`Recorded file: ${evidence.source} · SHA-256: ${evidence.sha256} · Generator schema 1`;$('previous').disabled=i===0;$('next').disabled=i===evidence.events.length-1}
function install(){if(!evidence.events.length)throw Error('No recorded events');$('event').max=evidence.events.length-1;$('event').value=0;render()}
$('event').oninput=render;$('previous').onclick=()=>{$('event').value=Number($('event').value)-1;render()};$('next').onclick=()=>{$('event').value=Number($('event').value)+1;render()};$('reload').onclick=()=>$('file').click();$('file').onchange=async()=>{try{const f=$('file').files[0];if(!f)return;if(f.size>32*1024*1024)throw Error('Recorded file exceeds 32 MiB');const bytes=await f.arrayBuffer(),text=new TextDecoder('utf-8',{fatal:true}).decode(bytes),events=text.split('\n').filter(l=>l.trim()).map(JSON.parse);if(!events.length||events.some(e=>!e||typeof e!=='object'||Array.isArray(e)||typeof e.event!=='string'))throw Error('Expected JSONL event records');const digest=await crypto.subtle.digest('SHA-256',bytes);evidence={source:f.name,sha256:Array.from(new Uint8Array(digest),x=>x.toString(16).padStart(2,'0')).join(''),events};$('error').textContent='';install()}catch(e){$('error').textContent=e.message}};install();
</script></html>'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('trace', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    raw = args.trace.read_bytes()
    if len(raw) > 32 * 1024 * 1024:
        parser.error('recorded trace exceeds 32 MiB')
    events = [json.loads(line) for line in raw.decode('utf-8').splitlines() if line.strip()]
    if not events or any(not isinstance(e, dict) or not isinstance(e.get('event'), str) for e in events):
        parser.error('expected nonempty JSONL event records')
    data = {'source': args.trace.name, 'sha256': hashlib.sha256(raw).hexdigest(), 'events': events}
    encoded = json.dumps(data, ensure_ascii=True).replace('<', '\\u003c')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(HTML.replace('__EVIDENCE__', encoded), encoding='utf-8')
    print(f'{len(events)} recorded events; source SHA-256 {data["sha256"]}; {args.output}')


if __name__ == '__main__':
    main()
