"""Fail-closed proposed post-review gate. Hash custody is not Source re-execution."""
import hashlib,json,pathlib,re
HEX=re.compile(r'^[0-9a-f]{64}$')
MAX_BYTES=1024*1024*1024
def sha(p):
 require(p.is_file() and p.stat().st_size<=MAX_BYTES,'file exceeds bounded evidence profile')
 return hashlib.sha256(p.read_bytes()).hexdigest()
def require(ok,why):
 if not ok:raise ValueError(why)
def load(cohort_path,expected_hash,evidence):
 require(HEX.fullmatch(expected_hash),'explicit externally reviewed cohort hash required')
 require(cohort_path.is_file() and cohort_path.stat().st_size<=16*1024*1024,'cohort manifest too large')
 require(sha(cohort_path)==expected_hash,'cohort changed since separate review')
 c=json.loads(cohort_path.read_text())
 require(c.get('schema')=='reviewed-aligned-causal-train@1','not reviewed aligned cohort')
 require(c.get('status')=='REVIEWED_FROZEN_ALIGNED_SOURCE_VALIDATED','quarantined/original/partial cohort refused')
 require(c.get('original_sentences')==394 and c.get('original_pairs')==1620,'original denominator lost')
 require(c.get('reviewed_exclusion_basis')=='whole_sentence_actual_occurrence_segmentation_only','unauthorized filtering')
 require(c.get('all_other_validation_failures')==0,'nonsegmentation failure invalidates all preparation')
 require(c.get('no_augmentation') is True and c.get('previously_fitted_train') is True,'scope exposure required')
 require(c.get('profile_sha256')=='045a51b00224785f77983a7c8127f690a1ca8e3c471eb40f6339ceec60c5251b','profile drift')
 names=['observed-pairs.jsonl','observed-stops.jsonl','source-observations.jsonl','source-identities.json','input-identities.json','quarantine.json','validation-manifest.json','aligned-validation.json']
 for name in names:
  require(HEX.fullmatch(c['files'][name]) and sha(evidence/name)==c['files'][name],name+' hash drift')
 v=json.loads((evidence/'aligned-validation.json').read_text())
 require(v.get('status')=='ALIGNED_SOURCE_PARITY_COMPLETE' and v.get('fit_inputs_eligible') is True,'partial/quarantined Source report refused')
 require(v.get('unexpected_failures')==0 and v.get('unprocessed_requests')==0,'incomplete Source coverage')
 ids=c['pair_ids'];sentences=c['sentence_ids'];excluded=c['excluded_sentence_ids']
 require(ids==sorted(set(ids)) and sentences==sorted(set(sentences)),'noncanonical membership')
 require(set(sentences).isdisjoint(excluded) and len(sentences)+len(set(excluded))==394,'sentence partition incomplete')
 require(len(ids)==c['pair_count']==v['pair_count'] and len(sentences)==v['sentence_count'],'count mismatch')
 require(0<len(ids)<=1620 and 0<len(sentences)<=394,'finite cohort bounds')
 require(c['excluded_pair_count']+len(ids)==1620,'pair denominator lost')
 original=json.loads((evidence/'validation-manifest.json').read_text())
 quarantine=json.loads((evidence/'quarantine.json').read_text())
 require(original.get('status')=='QUARANTINED_SOURCE_SEGMENTATION_NOT_FIT_ELIGIBLE' and original.get('fit_eligible') is False,'original complete quarantined report required')
 require(original.get('original_sentences')==394 and original.get('original_pairs')==1620,'original complete coverage lost')
 require(original.get('pairs')==len(ids) and original.get('source_validated_sentences')==len(sentences),'aligned set differs from completed compatible Source coverage')
 require(quarantine.get('both_arms_excluded_together') is True,'asymmetric exclusion')
 require({x['sentence_id'] for x in quarantine['actual_lexical_occurrence_mismatches']}==set(excluded),'quarantine sentence set differs from reviewed partition')
 require(quarantine.get('sentences')==len(excluded) and quarantine.get('paired_targets')==c['excluded_pair_count'],'quarantine count mismatch')
 require(all(x['reason']=='actual_source_occurrence_segmentation_differs_from_offline_labels' for x in quarantine['actual_lexical_occurrence_mismatches']),'unexpected failure cannot be excluded')
 rows=[json.loads(l) for l in (evidence/'observed-pairs.jsonl').read_text().splitlines()]
 require(len(rows)==len(ids) and {r['pair_id'] for r in rows}==set(ids),'pair coverage mismatch')
 require({r['sentence_id'] for r in rows}==set(sentences),'sentence coverage mismatch')
 refs=set()
 def visit(x):
  if isinstance(x,dict):
   for k,y in x.items():
    if k in {'input','output','prior','lexical'} and isinstance(y,str):
     require(HEX.fullmatch(y),'invalid canonical reference');refs.add(y)
    else:visit(y)
  elif isinstance(x,list):
   for y in x:visit(y)
 for row in rows:visit(row)
 for line in (evidence/'source-observations.jsonl').read_text().splitlines():visit(json.loads(line))
 blobs=c['canonical_blobs']
 require(isinstance(blobs,dict) and len(blobs)<=50000 and refs<=set(blobs),'canonical evidence coverage missing')
 total=0
 for identity,ordinary_sha256 in blobs.items():
  require(HEX.fullmatch(identity) and HEX.fullmatch(ordinary_sha256),'invalid semantic filename/ordinary custody digest')
  path=evidence/'canonical'/identity
  require(path.is_file() and path.stat().st_size<=65536,'canonical blob size exceeds fixture')
  total+=path.stat().st_size
  require(total<=MAX_BYTES,'canonical evidence exceeds1GiB fixture bound')
  require(sha(path)==ordinary_sha256,'canonical ordinary custody digest mismatch')
 require(c.get('original_request_sha256')=='d566230d31d0b4496cd8c7d9598f359293646cece7b0beafa3627e7a4230e555','offline request lineage drift')
 supervision=c['supervision_digests']
 require(set(supervision)==set(ids),'supervision pair coverage missing')
 for r in rows:
  require(r['sentence_id'] in sentences and type(r['target']) is int and 0<=r['target']<76,'invalid TRAIN target')
  require(isinstance(r.get('supervision'),str) and HEX.fullmatch(r['supervision']) and r['supervision']==supervision[r['pair_id']],'separate offline supervision provenance mismatch')
  require(r['status']=='OBSERVED_NOT_GLOBALLY_VALIDATED','unexpected observed-record schema')
  for arm in ['causal','full']:
   f=r[arm]['features'];seen=set()
   require(0<len(f['alternatives'])<=256,'alternative support outside frozen finite profile')
   for alt in f['alternatives']+[f['gold']]:
    ix=alt['indices'];require(len(ix)==25 and all(type(i) is int and 0<=i<413 for i in ix),'feature ABI')
    groups=[range(0,18),range(18,36),range(36,360),range(360,366),range(366,371),range(371,373),range(373,374),range(374,379)]+[range(i,i+2) for i in range(379,413,2)]
    require(all(i in g for i,g in zip(ix,groups)),'Source feature group ABI drift')
    require(alt['evidence']['entry']=='language-parser-scorer-model-features','wrong Source entry')
    require(len(alt['choice_vector'])==4,'choice ABI')
   for alt in f['alternatives']:
    key=tuple(alt['indices']);require(key not in seen,'duplicate alternative tuple');seen.add(key)
   require(tuple(f['gold']['indices']) in seen,'gold feature absent from candidate support')
 require(c.get('multiplicity')=='one_feature_tuple_per_unique_ordered25','multiplicity not frozen')
 return sorted(rows,key=lambda r:r['pair_id']),c
