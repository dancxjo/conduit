#!/usr/bin/env python3
"""Standalone proposed matched averaged perceptron. PREPARED, NEVER EXECUTED."""
import argparse,collections,hashlib,json,pathlib,random,struct
from gate import load,sha,require
FEATURES,CLASSES,LOOKUPS,SEED,EPOCHS,SCALE=413,76,25,4907,12,256
GROUPS=[range(0,18),range(18,36),range(36,360),range(360,366),range(366,371),range(371,373),range(373,374),range(374,379)]+[range(i,i+2) for i in range(379,413,2)]
def train(rows,orders,arm):
 weights=[[0]*FEATURES for _ in range(CLASSES)]
 totals=[[0]*FEATURES for _ in range(CLASSES)]
 times=[[0]*FEATURES for _ in range(CLASSES)]
 step=updates=0
 for order in orders:
  for index in order:
   row=rows[index];f=row[arm]['features'];gold=f['gold']['indices'];target=row['target']
   alternatives=sorted(tuple(a['indices']) for a in f['alternatives'])
   step+=1
   best_features,best=max(((ix,cls) for ix in alternatives for cls in range(CLASSES)),key=lambda x:(sum(weights[x[1]][i] for i in x[0]),-x[1],tuple(-i for i in x[0])))
   if list(best_features)==gold and best==target:continue
   updates+=1;changes=collections.Counter()
   for i in gold:changes[target,i]+=1
   for i in best_features:changes[best,i]-=1
   for (cls,i),delta in changes.items():
    totals[cls][i]+=(step-times[cls][i])*weights[cls][i]
    times[cls][i]=step;weights[cls][i]+=delta
    require(abs(weights[cls][i])<2**31,'finite preparation weight budget exceeded')
 for cls in range(CLASSES):
  for i in range(FEATURES):totals[cls][i]+=(step-times[cls][i])*weights[cls][i]
 quantized=[[round(v/step*SCALE) for v in row] for row in totals]
 require(all(-32768<=v<=32767 for row in quantized for v in row),'I16 overflow; no clipping')
 # One index from each canonical Source feature group; duplicates within a
 # record are intentionally counted in training score/update operations above.
 bounds=[max(abs(sum(max(row[i] for i in g) for g in GROUPS)),abs(sum(min(row[i] for i in g) for g in GROUPS))) for row in quantized]
 require(max(bounds)<=128525,'existing Source score bound exceeded; no widening')
 artifact=struct.pack('<8sIII',b'CI16SUM1',FEATURES,CLASSES,LOOKUPS)+b''.join(struct.pack('<h',v) for row in quantized for v in row)
 require(len(artifact)==62796,'artifact size mismatch')
 return artifact,{'samples':len(rows),'steps':step,'updates':updates,'maximum_group_score_bound':max(bounds)}
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--cohort',type=pathlib.Path,required=True);ap.add_argument('--reviewed-cohort-sha256',required=True);ap.add_argument('--evidence',type=pathlib.Path,required=True);ap.add_argument('--output',type=pathlib.Path,required=True);a=ap.parse_args()
 require(not a.output.exists(),'fresh output directory required')
 rows,cohort=load(a.cohort,a.reviewed_cohort_sha256,a.evidence)
 order=list(range(len(rows)));rng=random.Random(SEED);orders=[]
 for _ in range(EPOCHS):rng.shuffle(order);orders.append(order.copy())
 # Train both in memory before writing either artifact: no half-result success.
 results={arm:train(rows,orders,arm) for arm in ['causal','full']}
 a.output.mkdir(parents=False,exist_ok=False)
 receipts={}
 for arm,(artifact,metrics) in results.items():
  p=a.output/(arm+'.i16');p.write_bytes(artifact);receipts[arm]={'sha256':sha(p),**metrics}
 receipt={'status':'FITTED_NONBLIND_TRAIN_ABLATION_NOT_DEFAULT_NOT_CALIBRATED','seed':SEED,'epochs':EPOCHS,'scale':SCALE,'features':FEATURES,'classes':CLASSES,'lookups':LOOKUPS,'cohort_sha256':a.reviewed_cohort_sha256,'trainer_sha256':sha(pathlib.Path(__file__)),'gate_sha256':sha(pathlib.Path(__file__).with_name('gate.py')),'pair_order_sha256':hashlib.sha256(json.dumps([r['pair_id'] for r in rows],separators=(',',':')).encode()).hexdigest(),'epoch_orders_sha256':hashlib.sha256(json.dumps(orders,separators=(',',':')).encode()).hexdigest(),'source_evidence_files':cohort['files'],'artifacts':receipts,'scope':'Previously fitted TRAIN reused; frozen profile prior exposure. No heldout evaluation, probability, survivor calibration, default or quality acceptance.'}
 (a.output/'receipt.json').write_text(json.dumps(receipt,indent=2,sort_keys=True)+'\n')
if __name__=='__main__':main()
