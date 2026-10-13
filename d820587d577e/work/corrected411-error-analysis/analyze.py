import json,pathlib,struct,collections,hashlib,sys
base=pathlib.Path('semantics/language/training/ewt_proposal_v2/origin-corrected-v2')
b=(base/'samples.bin').read_bytes(); at=8; counts=collections.defaultdict(collections.Counter)
while at<len(b):
 values=struct.unpack_from('<29H',b,at);at+=58
 gold,target,n=values[:27],values[27],values[28]; counts[gold][target]+=1;at+=54*n
conflict=[{'features':k,'targets':dict(v)}for k,v in counts.items()if len(v)>1]
rows=json.load(open('work/corrected411-runtime/diagnostic-rows.json'));actual=json.load(open('work/corrected411-runtime/diagnostic-results.json'))
errors=[]
for row,r in zip(rows,actual['receipts']):
 errors.append({'id':row['id'],'token_errors':[{'ordinal':i,'form':f,'expected_head':h,'actual_head':p['head'],'expected_relation':rel,'actual_relation':p['base'],'expected_pos':pos,'actual_pos':ap}for i,(f,h,rel,pos,p,ap)in enumerate(zip(row['forms'],row['heads'],row['relations'],row['pos'],r['predicted'],r['pos']))if (h,rel,pos)!=(p['head'],p['base'],ap)]})
out={'scope':'Offline TRAIN/teaching diagnostic only; no runtime or heldout evidence generated','samples_sha256':hashlib.sha256(b).hexdigest(),'distinct_gold_feature_tuples':len(counts),'conflicting_exact_gold_feature_tuples':len(conflict),'conflicting_sample_occurrences':sum(sum(x['targets'].values())for x in conflict),'errors':errors,'conflicts':conflict}
pathlib.Path('work/corrected411-error-analysis/report.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({k:v for k,v in out.items()if k!='conflicts'},indent=2))
