"""Compare retained actual warm outputs to the development-only pinned C oracle."""
import hashlib,json,math,pathlib,struct,subprocess,sys
if len(sys.argv)!=3:raise SystemExit('usage: retained_warm_comparison.py SCRATCH_DIRECTORY PINNED_DEVELOPMENT_ROOT')
WORK=pathlib.Path(sys.argv[1]);MODEL=pathlib.Path(sys.argv[2])
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def floats(path):
 b=path.read_bytes();assert len(b)%4==0
 v=struct.unpack('<'+'f'*(len(b)//4),b);assert all(math.isfinite(x) for x in v);return v
def diff(actual,reference):
 assert len(actual)==len(reference)
 d=[x-y for x,y in zip(actual,reference)]
 return {'values':len(d),'max_absolute_error':max(map(abs,d)),'rms_error':math.sqrt(sum(x*x for x in d)/len(d)),'maximum_error_index':max(range(len(d)),key=lambda i:abs(d[i]))}
blob=MODEL/'resources/f32.bin';assert digest(blob)=='d35f0510da476716183a33caafe45cc095e48d496f06de2db7655e780a385e47'
manifest=json.loads((MODEL/'resources/manifest.json').read_text())
assert manifest['upstream_sha']=='503d81b138d76621aae4b12786e90de48aa8db3a'
expected_source_digests={'fargan.c': 'fca4e51f0407f9791900a972119a69f724485f034e48a2e77a608d24d950cc2c', 'fargan.h': 'e1761b61669f70ed6ae918772b8fae3ed9fe5c65f67d2ce3d68749ac4bd9a4ad', 'fargan_data.h': '402e691f4975d6d156903f0fe12da3385e20ec6dd6155c3387e9dad536f654cd', 'nnet.c': '1b376a8499167e143f8a4ec8d0c622d259b4ae99d8c7495a80d6318912de81e7', 'nnet_default.c': '1402eb611b7fcc1fe7fa18639f0395e0af47a783ef87450318411483516e9011', 'parse_lpcnet_weights.c': '78ab42e0fc04a72f89bc1a1af9363c630263608cb1065751c2c4d89c5f93e89a'}
for name,expected in expected_source_digests.items():assert digest(MODEL/'oracle/dnn'/name)==expected
data=MODEL/'oracle/dnn/fargan_data.c';assert digest(data)==manifest['source_sha256']
reference=(WORK/'pinned-warm-oracle.bin').read_bytes();assert len(reference)==3864
v=struct.unpack('<965fi',reference);h=v[:128];r=v[128:965];period=v[-1]
a=floats(WORK/'actual-warm-state.f32le');history=floats(WORK/'actual-warm-history.f32le');feature=floats(WORK/'actual-first-feature.f32le')
inspection=json.loads((WORK/'retained-inspection.json').read_text())
assert period==inspection['warm_period']
components={name:diff(a[start:end],r[start:end]) for name,start,end in [('convolution_history',0,164),('gru1',164,324),('gru2',324,452),('gru3',452,580),('pitch_history',580,836),('deemphasis',836,837)]}
report={
 'scope':'retained actual first-row warm startup only; no utterance-coverage or acoustic-quality claim',
 'upstream_revision':'503d81b138d76621aae4b12786e90de48aa8db3a',
 'precision':'same full float32 pretrained matrices; Source libm activations versus pinned upstream scalar activation approximations',
 'model_blob_sha256':digest(blob),'generated_model_source_sha256':digest(data),
 'session_basis_sha256':inspection['basis_sha256'],'raw_epochs_sha256':inspection['raw_epochs_sha256'],
 'oracle_build':{'compiler':subprocess.check_output(['gcc','-dumpfullversion'],text=True).strip(),'optimization':'-O2','floating_point_contract':'-ffp-contract=off','simd_disabled':True,'vectorization_disabled':True,'DISABLE_DEBUG_FLOAT':False},
 'measured':{'actual_feature_rows':1,'conditioning_calls':5,'continuation_subframes':4,'continuation_pcm_seed_samples':320,'continuation_pcm_seed':'all zero, Source-owned startup policy','period':period,'first_feature':feature,'warm_signal_state':diff(a,r),'warm_conditioning_history':diff(history,h),'state_components':components},
 'source_origins':{'first_feature':'first FarganFeatureProposalEpoch retained in session startup material','warm_outputs':'three exact canonical Source outputs retained in the same session startup material','source_native_admission':'previous authoritative export; development extraction does not grant new Native authority','oracle':'fargan_init then fargan_cont from pinned upstream C; no product runtime linkage'},
 'full_utterance_feature_rows_retained':False,'full_utterance_pcm_difference_measured':False,'warm_pcm_difference_measured':False,'bit_parity':False,'quality_cause_identified':False,'human_quality_acceptance':False,
 'limitations':['Subsequent actual Source feature20 rows were not retained. The64 actual PCM/state epochs alone do not determine those inputs.','Warm state error does not establish end-to-end PCM error or explain perceived acoustic quality.','Any different first-feature fixture cannot be substituted for the extracted actual retained row.'],
 'upstream_convolution_storage':{'allocated_floats':328,'active_history_floats':164,'basis':'compute_generic_conv1d uses nb_inputs328 minus input_size164'},
 'pinned_source_digests':{name:digest(MODEL/'oracle/dnn'/name) for name in ['fargan.c','fargan.h','fargan_data.h','nnet.c','nnet_default.c','parse_lpcnet_weights.c']},
 'artifacts':{}
}
for name in ['actual-first-proposal.canonical','actual-first-feature.f32le','actual-warm-five.f32le','FarganSubframeState.canonical','NumericHistory2x64.canonical','FarganPeriod.canonical','actual-warm-state.f32le','actual-warm-history.f32le','pinned-warm-oracle.bin']:
 source=WORK/name;report['artifacts'][name]={'bytes':source.stat().st_size,'sha256':digest(source)}
(WORK/'warm-comparison.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'report':str(WORK/'warm-comparison.json'),'state':report['measured']['warm_signal_state'],'history':report['measured']['warm_conditioning_history']}))
