"""Bounded raw postcard structure audit; Rust producer alone supplies semantic reload."""
import pathlib,json,hashlib,struct,re,subprocess
ROOT=pathlib.Path(__file__).resolve().parent.parent;OLD=ROOT/'outputs/6398-prior-shared-carrier-1e1a9';NEW=ROOT/'work/6398-carrier-refresh/shared-carrier';RECEIPT=ROOT/'outputs/6398-carrier-producer-resume-local-proof.json';SOURCE='002b2b44a833b5670e38415ecce3f652687cb372'
def sha(b):return hashlib.sha256(b).hexdigest()
def digest(tag,b):return hashlib.sha256(b'conduit.info.semantic.v1'+struct.pack('<H',len(tag))+tag.encode()+b).hexdigest()
def decode(b):
 assert 0<len(b)<=64*1024**2
 pos=0
 def integer():
  nonlocal pos
  value=0
  for i in range(10):
   assert pos<len(b);octet=b[pos];pos+=1
   if i==9:assert octet<=1
   value|=(octet&127)<<(7*i)
   if not octet&128:
    assert i==0 or octet!=0 # canonical unsigned LEB128 length
    return value
  raise AssertionError('oversizedvarint')
 count=integer();assert count==26;fields=[]
 for _ in range(count):
  n=integer();assert n<=32*1024**2 and n<=len(b)-pos;fields.append(b[pos:pos+n]);pos+=n
 assert pos==len(b);return fields
names=['source','intent','phonemes','phones','definitions','notation_profile','alignment','syllables','positioned_boundaries','word_phonemes','formant','boundaries','commit_request','commit_result','admission','model_correspondence','projection_bytes','lexical_profile','lexical_tape','commitment_source','sample_basis','model_vocabulary','model_checkpoint','model_terminal_identity','complete_carrier','source_identity']
def inspect(directory):
 files={name:(directory/name).read_bytes()for name in ['you-would.native','you-would.wav','you-would.json']};report=json.loads(files['you-would.json']);fields=decode(files['you-would.native']);assert len(fields[25])==32 and fields[25].hex()==report['source_identity']==report['model_terminal']['source_identity'];assert len(fields[23])==32 and fields[23].hex()==report['model_terminal']['terminal_evidence_identity']
 assert digest('proof/carried-formant-artifact@1',files['you-would.native'])==report['original_artifact_identity'];assert digest('proof/carried-formant-wav@1',files['you-would.wav'])==report['wav_identity']
 wav=files['you-would.wav'];assert wav[:4]==b'RIFF'and wav[8:16]==b'WAVEfmt 'and wav[36:40]==b'data';assert struct.unpack_from('<I',wav,4)[0]==len(wav)-8 and struct.unpack_from('<I',wav,16)[0]==16;assert struct.unpack_from('<HHIIHH',wav,20)==(1,1,8000,16000,2,16);assert struct.unpack_from('<I',wav,40)[0]==len(wav)-44==report['pcm_frames']*2 and report['sample_rate_hz']==8000
 assert report['human_listening']==report['device_playback']=='not performed';assert report['model_terminal']['selected_backend']is None and report['model_terminal']['refusal']=='NoAdmittedAcousticBack'and report['model_terminal']['trained_weights']is False
 bindings=[]
 def walk(v):
  if isinstance(v,dict):
   for e in v.get('configuration',[]):
    if e.get('key')in('source','projection'):
     expected=report[e['key']+'_identity'];assert (e.get('value') or e.get('default_value'))['Text']==expected;bindings.append(e['key'])
     if 'TextOneOf'in e.get('rule',{}):assert e['rule']['TextOneOf']['values']==[expected]
   for x in v.values():walk(x)
  elif isinstance(v,list):
   for x in v:walk(x)
 walk(report['plan']);assert len(bindings)>=4
 return files,report,fields,bindings
if not RECEIPT.exists():raise SystemExit('Producer receipt not yet available; audit not executed')
p=json.loads(RECEIPT.read_bytes());assert p['protected1033_verified_before'] and p['protected1033_verified_after'] and p['prior127_verified_before'] and p['prior127_verified_after'];assert p['source']==SOURCE and p['status']=='passed'and p['exit_code']==0 and p['actual_passed']==2 and p['test_counts']==[1,1]and p['clean_after'];log=pathlib.Path(p['log']).read_bytes();assert sha(log)==p['log_sha256'];assert b'original_multiword_intent_executes_through_installed_factory_plan_and_play ... ok'in log and b'independently_reload_exported_carrier ... ok'in log
for row in p['generated_files']:
 b=pathlib.Path(row['path']).read_bytes();assert sha(b)==row['sha256']and len(b)==row['bytes']
a,ar,af,ab=inspect(OLD);b,br,bf,bb=inspect(NEW);fields=[{'index':i,'name':n,'unchanged':x==y,'old_bytes':len(x),'new_bytes':len(y),'old_sha256':sha(x),'new_sha256':sha(y)}for i,(n,x,y)in enumerate(zip(names,af,bf))]
for e in p['executables']:
 path=pathlib.Path(e['path']);data=path.read_bytes();assert data[:4]==b'\x7fELF'and len(data)==e['bytes']and sha(data)==e['sha256']
# Join both cargo-run router commands and cargo-test actual executable paths.
running=re.findall(r'^\s*Running `([^`]+)`',log.decode(),re.M)
running_paths=[line.split(' ',1)[0] for line in running]
running_paths+=re.findall(r'^\s*Running .*?\(([^)]+)\)\s*$',log.decode(),re.M)
inventory={str(pathlib.Path(e['path']).resolve()) for e in p['executables']};actual_bindings=[];extras=[]
for name in running_paths:
 path=pathlib.Path(name);path=path if path.is_absolute() else pathlib.Path('/tmp/conduit-6398-carrier-refresh-source')/path
 path=path.resolve();data=path.read_bytes();assert data[:4]==b'\x7fELF'
 row={'actual_path':str(path),'bytes':len(data),'sha256':sha(data),'in_private_receipt_inventory':str(path)in inventory};actual_bindings.append(row)
 if str(path)not in inventory:extras.append(row)
assert len(actual_bindings)>=4
out={'status':'passed','source':SOURCE,'producer_receipt_sha256':sha(RECEIPT.read_bytes()),'producer_command':p['command'],'raw_log_sha256':sha(log),'actual_Rust_producer_and_fresh_reload_tests':2,'all26_structural_fields':fields,'changed_field_indexes':[r['index']for r in fields if not r['unchanged']],'file_custody':{n:{'old_sha256':sha(a[n]),'old_bytes':len(a[n]),'new_sha256':sha(b[n]),'new_bytes':len(b[n]),'unchanged':a[n]==b[n]}for n in a},'report_correspondence':{k:{'old':ar[k],'new':br[k],'unchanged':ar[k]==br[k]}for k in ['source_identity','projection_identity','original_artifact_identity','wav_identity','compiled_dsp_source','speech_authoring_source','native_schema_digests','model_terminal','plan']},'wav':'BothactualRIFFheaders/PCMgeometryvalidated,8000Hzmono16bit; byteequalityreportedabove','raw_identity_joins':'Bothpacket25==reportSource==modelSource;packet23==modelterminalidentity;actualdomainSHAartifact/WAVdigestsandreportPlanSource/projectionconfigurationjoinsverified.','semantic_boundary':'BoundedstandardlibpostcardVec<Vec<u8>>structuraldecodeandhashjoinsONLY. Nativeadmission/actualPlanPlay/PCMsemanticreplay providedbyretainedsource-boundRustproducer+freshprocessreload, notindependentlyreimplementedhere.','actual_ELFs_rehashed':len(p['executables']),'all_actual_Running_bindings':actual_bindings,'additional_router_executables_outside_private_inventory':extras,'limits':['No sitefixture/manifest/source/ref/issuemutation;generatedcandidateartifactsonly.','No parent4907/6068closure,learnedvoice/deviceplayback/humanlistening/physical/CIacceptance.','Old1e1a9packetpreservedandnotrelabelednewexecution; exactnewproducer002b remainsdistinctfromfuturefixturecommit/stagedpublication.']};(ROOT/'outputs/6398-generated-carrier-independent-review.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({'status':'passed','changed_fields':out['changed_field_indexes'],'WAV_unchanged':a['you-would.wav']==b['you-would.wav'],'ELFs':len(p['executables'])}))
