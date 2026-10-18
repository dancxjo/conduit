#!/usr/bin/env python3
"""Compare the unchanged terminal Rust entrance with the fixed-storage profile."""
import argparse,hashlib,importlib.util,json,statistics
from pathlib import Path

def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def compare(before,after,reader):
 old,left,lh=reader.read(before);new,right,rh=reader.read(after)
 assert new['measurement_schema']=='conduit.terminal-prefix/reserved-count@1'
 assert new['measurement_entrance']=='scheduler::scan_characterization::terminal_completion::capture_completion_prefix'
 assert old['harness_sha256']==digest(before.parent/'capture-terminal.py')
 assert new['harness_sha256']==digest(after.parent/'capture-current-terminal.py')
 assert old['source']!=new['source'] and old['executable_sha256']!=new['executable_sha256']
 for key in ['profile','rustc','cpu_model','affinity','system']:assert old[key]==new[key],key
 assert old['profile']['opt_level']=='3'
 word=new['native_word_bytes'];assert word in [4,8]
 binary=after.parent/'current-selector-refresh/5231-selector-alternating/bin/packed'
 assert digest(binary)==new['executable_sha256'],'terminal worker identity'
 header=binary.read_bytes()[:5];assert header[:4]==b'\x7fELF' and word=={1:4,2:8}[header[4]],'native word width'

 entrance=new['measurement_entrance']
 for folder in [before,after]:
  for process in range(3):assert 'test '+entrance+' ...' in (folder/f'process-{process}.stdout.log').read_text()
 excluded={'elapsed_ns','terminal_node_probes','kernel_bytes'}
 for key,prior in left.items():
  current=right[key]
  assert {k:v for k,v in prior.items() if k not in excluded}=={k:v for k,v in current.items() if k not in excluded},'semantic/work mismatch'
  assert prior['allocations']==prior['reallocations']==current['allocations']==current['reallocations']==0
  assert prior['terminal_node_probes']==17*prior['nodes']
  assert current['terminal_node_probes']==(0 if current['nodes']>=2*word else 17*current['nodes'])
  assert current['kernel_bytes']==prior['kernel_bytes'],'fixed reservation grew'
 summaries=[]
 for nodes in [1,8,32,256]:
  row={'nodes':nodes}
  for label,rows in [('before',left),('after',right)]:
   selected=[r for r in rows.values() if r['nodes']==nodes]
   means=[statistics.mean(r['elapsed_ns'] for r in selected if r['process']==p) for p in range(3)]
   row[label]={'process_mean_ns':means,'mean_ns':statistics.mean(means),'process_sd_ns':statistics.stdev(means),'kernel_bytes':selected[0]['kernel_bytes'],'terminal_node_probes':selected[0]['terminal_node_probes']}
  summaries.append(row)
 return {'schema':'conduit.terminal-prefix/reserved-comparison@1','before_source':old['source'],'after_source':new['source'],'exact_sign_replay_pairs':len(left),'artifact_hashes_verified':lh+rh,'native_word_bytes':word,'summaries':summaries,'limits':new['limits'],'wrapper_difference':'Same compiled Rust timing entrance and span; wrapper changes only source/output/binary locations and expected probe-budget validation. Both wrapper hashes are checked independently; historical source and its raw observations are preserved.'}

def main():
 if not __debug__:raise RuntimeError('Python -O disables evidence checks')
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--before',type=Path,required=True);p.add_argument('--after',type=Path,required=True);p.add_argument('--reader',type=Path,default=Path(__file__).with_name('verify-terminal-completion.py'));p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 spec=importlib.util.spec_from_file_location('legacy',a.reader);reader=importlib.util.module_from_spec(spec);spec.loader.exec_module(reader)
 result=compare(a.before,a.after,reader)
 with a.output.open('x') as output:output.write(json.dumps(result,indent=2)+'\n')
if __name__=='__main__':main()
