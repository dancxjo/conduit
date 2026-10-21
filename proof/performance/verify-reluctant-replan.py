#!/usr/bin/env python3
"""Independent exact arithmetic and causal check of the hosted H2 family."""
import gzip, hashlib, json, sys
from fractions import Fraction
from pathlib import Path

def require(ok, message):
    if not ok: raise ValueError(message)
def data(path):
    if path.exists(): return path.read_bytes()
    return gzip.decompress(path.with_name(path.name+'.gz').read_bytes())
def ratio(v): return Fraction(int(v['numerator']), int(v['denominator']))
def digest(v): return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()).hexdigest()
def load(directory):
    root=Path(directory);status=json.loads(data(root/'status.json'));worker=json.loads(data(root/'worker-status.json'))
    for name, expected in status['artifact_sha256'].items():
        require(hashlib.sha256(data(root/name)).hexdigest()==expected, 'artifact hash differs: '+name)
    return [json.loads(line) for line in data(root/'raw.jsonl').decode().splitlines()],status,worker

def verify(rows,status,worker,positive=True):
    require(status['source_custody_verified'] and status['worker_success'], 'outer source/worker refusal')
    require(worker['source_custody'] and worker['success'], 'worker source/measurement refusal')
    require(status['git_sha']==status['final_git_sha']==worker['source']==worker['final_source'], 'source substitution')
    def one(kind):
        values=[r for r in rows if r['case']==kind];require(len(values)==1,'missing/duplicated '+kind);return values[0]
    reg,definition,initial,summary=map(one,['registration','definition','initial','summary'])
    require(reg['horizon']==1024 and reg['minimum_observations']==3 and reg['maximum_explicit_backlog']==4, 'profile differs')
    clock=reg['clock'];require(clock['scale']=='Nanoseconds' and clock['basis_id']=='linux/CLOCK_MONOTONIC', 'clock basis differs')
    error=2*(clock['resolution_ticks']+clock['uncertainty_ticks']);require(error>0,'unknown clock resolution')
    def measurement(m):
        a,b=int(m['start']),int(m['end']);require(b>=a and m['clock']==clock,'clock/order differs')
        lo,hi=Fraction(max(0,b-a-error),10**9),Fraction(b-a+error,10**9)
        require(ratio(m['range']['lower'])==lo and ratio(m['range']['upper'])==hi and m['range']['unit']=='s','duration range differs')
        return lo,hi
    old,new=definition['previous_plan'],definition['candidate_plan'];host=definition['host']
    require(old['plan_id']!=new['plan_id'],'replacement reused old Plan')
    for key in ['source_document_id','checked_plot_id','expanded_plot_id']:
        require(old[key]==new[key], 'changed authored identity')
    require(len(old['fragments'])==len(new['fragments'])==1,'nonlocal plan')
    oldgear,newgear=old['fragments'][0]['placements'][0],new['fragments'][0]['placements'][0]
    require(len(old['fragments'][0]['placements'])==len(new['fragments'][0]['placements'])==1,'cost is not for one Gear')
    for key in ['gear_id','kind_id','kind_contract_revision','inputs','outputs','configuration','semantic_contract','source_span']:
        require(oldgear[key]==newgear[key], 'changed logical work '+key)
    require(oldgear['implementation_id']=='std/kernel-pure-expression@1' and newgear['implementation_id']=='std/kernel-local-pure-expression@1','wrong realized profiles')
    require(not oldgear['resources'] and not newgear['resources'],'unsupported residence profile')
    require(clock['host_id']==host['host_id'] and clock['boot_id']==host['boot_id'],'clock custodian differs')
    require(definition['input_bytes']==list((7).to_bytes(8,'little')),'cost demand differs')
    stages=definition['stages'];require([r['stage'] for r in stages]==['Preparation','Transfer','Residency','Migration','Continuity'],'missing/reordered cost stage')
    require(len({r['sign_id'] for r in stages})==5,'duplicate replacement cost Sign')
    intervals=[measurement(r['measurement']) for r in stages]
    overhead=sum((hi for _,hi in intervals),Fraction())
    require(ratio(definition['overhead']['upper'])==overhead and ratio(definition['overhead']['lower'])==sum((lo for lo,_ in intervals),Fraction()),'replacement cost total differs')
    for i,stage in enumerate(stages):
        plan=old if i>=3 else new;gear=oldgear if i>=3 else newgear;source=stage['source']
        require(stage['plan_id']==plan['plan_id'],'stage rebound to other Plan')
        require(source['sign_id']==stage['sign_id'] and source['host_id']==host['host_id'] and source['boot_id']==host['boot_id'] and source['capability_id']==gear['capability_id'] and source['implementation_id']==gear['implementation_id'] and source['artifact']==gear['artifact_id'],'stage source differs')
        require(source['observed']==stage['measurement']['end'],'stage lost completed measurement')
        require(source['clock']==clock and source['calibration_identity']==reg['calibration_identity'] and source['calibration_revision']==reg['calibration_revision'],'stage calibration differs')
    def receipt(r,plan):
        p=r['active_play'];require(r['plan_id']==plan['plan_id']==p['plan_id'] and r['source_document_id']==plan['source_document_id'],'execution Plan/Source differs')
        require(p['host_id']==host['host_id'] and p['boot_id']==host['boot_id'],'execution Host/Boot differs')
        require(r['value_capacity_before']==r['value_capacity_after'],'Play storage grew')
        require(r['output']['bytes']==list((231).to_bytes(8,'little')) and r['output']['port']=='result' and r['output']['track']=='payload','output custody differs')
        signs=r['kernel_sign'];require([s['sequence'] for s in signs]==list(range(len(signs))),'kernel Sign gap')
        require(sum(s['kind']=='BackCompleted' for s in signs)==1 and sum(s['kind']=='RemoteValueDelivered' for s in signs)==1,'missing logical completion/delivery')
        terminal=r['terminal'];require(terminal in r['observations'],'terminal not retained')
        require(terminal['kind']=={'PlanTerminal':{'disposition':'Completed'}} and terminal['plan_id']==p['plan_id'] and terminal['active_play_id']==p['active_play_id'],'terminal rebound')
        require(terminal['host_id']==p['host_id'] and terminal['boot_id']==p['boot_id'],'terminal custodian rebound')
        return terminal['sign_id']
    receipt(initial['receipt'],old)
    require(digest(initial['receipt'])==definition['old_receipt_sha256']==summary['old_receipt_before']==summary['old_receipt_after'],'old receipt changed or vanished')
    executions={};plays=[initial['receipt']['active_play']]
    for row in rows:
        if row['case']=='execution':
            name=row['request'];require(name not in executions,'duplicate request')
            plan=new if name.endswith('/candidate') or name.startswith('horizon/') else old
            receipt(row['receipt'],plan);measurement(row['queue']);measurement(row['service']);executions[name]=row;plays.append(row['receipt']['active_play'])
        elif row['case']=='accepted-replacement':
            receipt(row['receipt'],new);require(row['request']=='horizon/0','replacement request differs');plays.append(row['receipt']['active_play'])
    accepted=[r for r in rows if r['case']=='accepted-replacement']
    decisions={r['label']:r for r in rows if r['case']=='decision'}
    require(len(decisions)==len([r for r in rows if r['case']=='decision']),'duplicated decision')
    expected_labels={'noisy','persistent','already-recommended'} if summary['persistent_replacement_accepted'] else {'noisy','persistent'}
    require(set(decisions)==expected_labels,'missing decision')
    threshold=Fraction(int(reg['minimum_net_ns']),10**9)
    for label,row in decisions.items():
        account=row['account'];family='persistent' if label=='already-recommended' else label
        require(account['previous_plan_id']==old['plan_id'] and account['candidate_plan_id']==new['plan_id'],'decision identity differs')
        require(ratio(account['overhead_upper'])==overhead and len(account['observations'])==3,'decision cost differs')
        require(account['replacement_sources']==[s['source'] for s in stages],'decision lost replacement source custody')
        observed=[];nets=[]
        for i,observation in enumerate(account['observations']):
            a,b=executions[f'{family}/{i}/previous'],executions[f'{family}/{i}/candidate']
            alo,ahi=measurement(a['queue']);slo,shi=measurement(a['service']);oldlower=alo+slo
            blo,bhi=measurement(b['queue']);tlo,thi=measurement(b['service']);newupper=bhi+thi
            net=(oldlower-newupper)*reg['horizon']-overhead
            require(ratio(observation['net_seconds'])==net,'fabricated conservative net benefit');nets.append(net)
            left,right=observation['previous'],observation['candidate']
            for source,record,gear in [(left,a,oldgear),(right,b,newgear)]:
                require(source['sign_id']==record['receipt']['terminal']['sign_id'],'forecast lost actual terminal Sign')
                require(source['clock']==clock and source['calibration_identity']==reg['calibration_identity'] and source['calibration_revision']==reg['calibration_revision'],'forecast calibration differs')
                require(source['host_id']==host['host_id'] and source['boot_id']==host['boot_id'] and source['capability_id']==gear['capability_id'] and source['implementation_id']==gear['implementation_id'] and source['artifact']==gear['artifact_id'],'forecast subject substitution')
                require(int(source['observed'])>=int(record['service']['end']) and int(source['expires'])>int(account['at']['ticks']),'stale/future forecast')
            require(left['observed']==right['observed'],'unpaired epoch');observed.append(int(left['observed']))
        require(observed==sorted(set(observed)) and observed[-1]-observed[0]-error>=int(reg['minimum_span_ns']),'persistence/order differs')
        require(int(account['at']['ticks'])-observed[0]+error<=int(reg['maximum_window_ns']),'outside window')
        require(max([observed[i+1]-observed[i]+error for i in range(2)]+[int(account['at']['ticks'])-observed[-1]+error])<=int(reg['maximum_gap_ns']),'observation gap')
        if label=='noisy': require(account['keep_current']=='NetBenefitNotPersistent' and any(n<=threshold for n in nets) and row['output_count']==0,'noise retriggered/reason lost')
        elif label=='already-recommended': require(account['keep_current']=='AlreadyRecommended' and row['output_count']==0,'repeated old recommendation')
        else: require((account['keep_current'] is None)==all(n>threshold for n in nets),'profitability decision differs')
    for i in range(4): require(f'noisy/1/queued-explicit-request/{i}' in executions,'missing admitted backlog')
    queued=executions['noisy/1/candidate']['queue'];require(int(queued['end'])>int(queued['start']),'fake queued load')
    for i in range(4):
        b=executions[f'noisy/1/queued-explicit-request/{i}']['service'];require(int(queued['start'])<=int(b['start'])<=int(b['end'])<=int(queued['end']),'backlog outside measured queue')
    require(len({p['active_play_id'] for p in plays})==len(plays),'reused Play')
    require([p['play_sequence'] for p in plays]==list(range(len(plays))),'unrecorded/replayed execution')
    if positive:
        require(summary['persistent_replacement_accepted'] and summary['executed_horizon_requests']==1024 and len(accepted)==1,'no accepted persistent replacement')
        require(all(f'horizon/{i}' in executions for i in range(1,1024)) and len(plays)==1041,'incomplete finite workload')
    return {'source':worker['source'],'executions':len(plays),'accepted':summary['persistent_replacement_accepted'],'old_custody_sha256':summary['old_receipt_after']}
if __name__=='__main__':
    try: print(json.dumps(verify(*load(sys.argv[1])),sort_keys=True))
    except (ValueError,KeyError,IndexError,TypeError,AssertionError) as e: sys.exit('FAIL: '+str(e))
