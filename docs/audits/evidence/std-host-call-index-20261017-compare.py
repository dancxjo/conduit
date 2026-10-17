import json, pathlib, statistics, sys
before, after, output = map(pathlib.Path, sys.argv[1:])
def load(p):
    rows=[json.loads(line) for line in (p/'raw.jsonl').read_text().splitlines()]
    status=json.loads((p/'status.json').read_text()); assert status['worker_success'] and status['source_custody_verified'] and status['conformance_success']
    config={k:v for k,v in status['config'].items() if k!='output'}
    data={(r['phase'],r['case'],r['repetition']):r for r in rows if r.get('phase') in ['finite-ordinary-hosted-pipeline','finite-ordinary-fusion-pipeline']}
    assert len(data)==27
    return status,config,data
a,ac,ar=load(before);b,bc,br=load(after);assert ac==bc and ar.keys()==br.keys()
exact=['source_sha256','values','gears','ordered_output_sha256','kernel_decisions','kernel_logical_invocations','value_capacity_before','value_capacity_after','signs']
for key in ar:
    for field in exact: assert ar[key][field]==br[key][field],(key,field)
def stats(xs):
    return {'samples_ns':xs,'mean_ns':statistics.mean(xs),'sample_sd_ns':statistics.stdev(xs)}
groups=[]
for phase,case in sorted({(k[0],k[1]) for k in ar}):
    rows_a=[ar[(phase,case,i)] for i in range(ac['repetitions'])];rows_b=[br[(phase,case,i)] for i in range(ac['repetitions'])]
    metrics={}
    for field in ['full_host_call.wall_ns','kernel_phase_wall.back_preparation_ns','kernel_phase_wall.play_ns']:
        outer,inner=field.split('.');aa=stats([r[outer][inner] for r in rows_a]);bb=stats([r[outer][inner] for r in rows_b]);metrics[field]={'before':aa,'after':bb,'ratio_of_means_after_over_before':bb['mean_ns']/aa['mean_ns']}
    groups.append({'phase':phase,'case':case,'metrics':metrics})
result={'schema':'std-host-call-index-comparison@1','before_source':a['git_sha'],'after_source':b['git_sha'],'config':ac,'matched_rows':len(ar),'exact_fields':exact,'groups':groups,'scope':'one independently verified process per variant, three batches; uncontrolled host, descriptive only; no causal speedup or calibrated regression claim'}
output.write_text(json.dumps(result,indent=2)+'\n')
for g in groups:
    m=g['metrics']['kernel_phase_wall.play_ns'];print(g['case'],round(m['before']['mean_ns']/1e6,3),round(m['after']['mean_ns']/1e6,3),round(m['ratio_of_means_after_over_before'],3))
