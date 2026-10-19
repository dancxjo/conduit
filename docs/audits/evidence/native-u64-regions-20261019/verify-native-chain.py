#!/usr/bin/env python3
"""Verify retained native/canonical chain measurements, independently of Rust checks."""
import argparse, collections, hashlib, json
from pathlib import Path

def digest(data): return hashlib.sha256(data).hexdigest()
def read(path): return json.loads(path.read_text())
def require(condition, message):
    if not condition: raise ValueError(message)

def verify(root, source):
    root = Path(root)
    public = (root/'status.json').exists()
    receipt = read(root/('status.json' if public else 'capture-status.json'))
    if public:
        require(receipt['git_sha']==source and receipt['final_git_sha']==source and receipt['source_custody_verified'], 'command source custody')
        require(receipt['worker_success'] and receipt['conformance_success'] and receipt['worker_exit_code']==0, 'command success')
        hashes = receipt['artifact_sha256']
        conformance = read(root/'conformance.json')
        require(conformance['success'] and conformance['git_sha']==source, 'release conformance')
        require('28 passed; 0 failed' in (root/'conformance.stdout.log').read_text(), 'complete 28-test conformance')
    else:
        require(receipt['source_before']==source==receipt['source_after'] and receipt['clean_before'] and receipt['clean_after'], 'replay source custody')
        require(receipt['exit_code']==0 and receipt['affinity']=='4', 'replay success/affinity')
        require(receipt['worker_sha256']==receipt['expected_worker_sha256'], 'frozen worker identity')
        inherited = root.parent/receipt['conformance_capture']
        require(inherited.resolve()!=root.resolve() and (inherited/'status.json').exists(), 'public conformance reference')
        verify(inherited, source)
        hashes=receipt['artifact_sha256']
    actual={str(p.relative_to(root)) for p in root.rglob('*') if p.is_file()}-{'status.json','capture-status.json'}
    require(actual==set(hashes), 'manifest member coverage')
    for name, sha in hashes.items():
        path=root/name
        require(path.resolve().is_relative_to(root.resolve()), 'manifest path containment')
        require(digest(path.read_bytes())==sha, 'artifact hash '+name)
    meta=read(root/'metadata.json')
    require(meta['git_sha']==source and meta['source_status']=='clean' and meta['profile']=='release' and meta['debug_assertions'] is False, 'worker metadata')
    require('x86_64' in meta['os'], 'native sidecar ABI scope')
    require(meta['hosted_native_chain_schema']=='native-u64-with-canonical-fused-reference@1', 'native schema')
    if not public: require(meta['affinity'].split(':',1)[1].strip()=='4', 'observed CPU4 affinity')
    allrows=[json.loads(s) for s in (root/'raw.jsonl').read_text().splitlines()]
    rows=[r for r in allrows if r.get('phase')=='finite-ordinary-fusion-pipeline']
    report=read(root/'report.json')
    require(report['raw_records']==len(allrows), 'raw record count')
    require([r for r in report['runs'] if r.get('phase')=='finite-ordinary-fusion-pipeline']==rows, 'raw/report profile equality')
    repetitions=report['config']['repetitions']
    expected={(g,k,m) for g in (1,8,32) for k in range(repetitions) for m in (('local-unfused','fused') if g==1 else ('local-unfused','fused','native-fused'))}
    keyed={(r['gears'],r['repetition'],r['mode']):r for r in rows}
    require(len(keyed)==len(rows) and set(keyed)==expected, 'complete unique physical-profile matrix')
    lifecycle_kinds={'RemoteValueOffered','RemoteValueAccepted','RemoteValueDelivered','RemoteOutputClosed','RemoteInputAdmitted','RemoteInputClosed','InputClosed','BackCompleted'}
    def lifecycle(r):return [{k:v for k,v in e.items() if k!='sequence'} for e in r['signs'] if e['kind'] in lifecycle_kinds]
    retained_outputs={}
    retained_meaning={}
    for r in rows:
        g,k,m=r['gears'],r['repetition'],r['mode'];stem=f"{r['case']}-{k}"
        authored=read(root/'sources'/f'pure-chain-{g}-{k}-source.json')
        require(digest(authored.encode())==r['source_sha256'], 'authored Source identity')
        plan=read(root/'sources'/f'{stem}-plan.json');fragment=plan['fragments'][0]
        require(plan['checked_plot_id']==r['checked_plot_id'], 'checked Plot identity')
        require(plan['plan_id']==r['active_play']['plan_id'], 'active Plan identity')
        require(len(fragment['placements'])==g and len(fragment['connections'])==g-1, 'authoritative logical graph')
        retained_meaning[g,k,m]={p['gear_id']:{field:p[field] for field in ('kind_id','inputs','outputs','configuration','semantic_contract')} for p in fragment['placements']}
        native=m=='native-fused';fused=m!='local-unfused' and g>1
        require(len(fragment['execution_fusions'])==int(fused), 'fusion selection count')
        if fused:
            f=fragment['execution_fusions'][0]
            require(f['execution_profile_id']==('std/native-u64-chain@1' if native else 'std/pure-chain-dispatch@1'), 'selected physical profile')
            require(len(f['preserved_placements'])==g and len(f['preserved_connections'])==g-1, 'preserved logical boundaries')
        outputs=read(root/'sources'/f'{stem}-outputs.json')
        retained_outputs[g,k,m]=outputs
        require(len(outputs)==64 and all(v['port']=='output' and v['sequence']==i and v['kind']=='value/u64' and bytes(v['bytes'])==(i+32).to_bytes(8,'little') for i,v in enumerate(outputs)), 'full ordered canonical outputs')
        require(digest(b''.join(bytes(v['bytes']) for v in outputs))==r['ordered_output_sha256'], 'output digest')
        require(r['value_capacity_before']==r['value_capacity_after'], 'fixed Play storage')
        signs=r['signs'];require(all(set(e)=={'sequence','node','kind','port','request'} for e in signs), 'complete retained Sign fields')
        require(all(a['sequence']<b['sequence'] for a,b in zip(signs,signs[1:])), 'retained Sign order')
        require(sum(e['kind']=='RemoteValueDelivered' for e in signs)==64==r['mandatory_output_sign_count'], 'mandatory output evidence')
        require(len(lifecycle(r))==258+2*g, 'lifecycle evidence count')
        require('bounded transient history' in r['sign_scope'], 'honest retained-history scope')
        require([p[0] for p in r['cold_phases']]==['parse','check','expand','host-offer-preparation','realization-selection','physical-fusion-selection','plan-realization-validation-and-sealing'], 'complete separate cold preparation phases')
        require(all(p[1]['wall_ns']>0 and all(v>=0 for v in p[1]['allocator'].values()) for p in r['cold_phases']), 'cold time/allocation observations')
        phase=r['kernel_phase_wall'];intervals=phase['allocator_intervals']
        require(len(intervals)==4 and all(v>=0 for interval in intervals for v in interval.values()), 'phase allocator attribution')
        require(sum(phase[x] for x in ('before_preparation_ns','back_preparation_ns','play_ns','after_play_ns'))==phase['call_wall_ns'], 'phase time partition')
        for metric in ('allocations','reallocations','deallocations','requested_bytes'):
            require(sum(i[metric] for i in intervals)==r['full_host_call']['allocator'][metric], 'phase allocation partition '+metric)
        work=r['work_accounting'];require(work['logical_evaluations']==g*64, 'useful work')
        require(work['intermediate_materializations']==(0 if native else (g-1)*64), 'canonical work accounting')
        require(work['native_word_transports']==((g-1)*64 if native else 0), 'native work accounting')
    for g in (1,8,32):
        for k in range(repetitions):
            base=keyed[g,k,'local-unfused'];canonical=keyed[g,k,'fused']
            for m in (('fused',) if g==1 else ('fused','native-fused')):
                row=keyed[g,k,m]
                require(row['checked_plot_id']==base['checked_plot_id'] and row['source_sha256']==base['source_sha256'], 'paired logical Source')
                require(row['ordered_output_sha256']==base['ordered_output_sha256'] and row['kernel_logical_invocations']==base['kernel_logical_invocations'], 'paired output/work')
                require(lifecycle(row)==lifecycle(base), 'paired lifecycle identities/order')
                require(retained_outputs[g,k,m]==retained_outputs[g,k,'local-unfused'], 'paired complete Fore outputs')
                require(retained_meaning[g,k,m]==retained_meaning[g,k,'local-unfused'], 'paired typed logical meaning')
            if g>1:
                native=keyed[g,k,'native-fused']
                require(native['signs']==canonical['signs'], 'exact native/canonical fused retained Sign equality')
                require(native['kernel_decisions']==canonical['kernel_decisions']<base['kernel_decisions'], 'physical dispatch work')
                slots,storage=native['value_capacity_before'];cslots,cstorage=canonical['value_capacity_before']
                require(slots==cslots and storage-cstorage==slots*16, 'explicit x86_64 native sidecar charge')
    return {'source':source,'capture':str(root),'verified_profile_rows':len(rows),'native_canonical_pairs':2*repetitions,'retained_fused_sign_records_compared':sum(len(r['signs']) for r in rows if r['mode']=='native-fused'),'scope':'all recorded retained Signs; bounded history, not a lifetime transient trace','verified_artifacts':len(hashes)}

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('capture');parser.add_argument('--source',required=True);args=parser.parse_args()
    print(json.dumps(verify(args.capture,args.source),indent=2))
