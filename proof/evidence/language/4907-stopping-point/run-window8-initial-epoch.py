import pathlib,json,subprocess,os,hashlib,time,re
w=pathlib.Path(__file__).resolve().parent
m=json.loads((w/'canonical-window8-scoped-coherent-sdk-closed.json').read_text())
paths=[pathlib.Path('/home/dancxjo/conduit-4907-production-custody/semantics/language/src')/n for n in ['parser_session_window8_dependency_candidate.rs','parser_session_window8_stage.rs']]
def sha(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''):h.update(block)
    return h.hexdigest()
if subprocess.check_output(['rustc','+stable','-vV'],text=True)!=m['compiler']:
    raise SystemExit('compiler version differs from coherent SDK')
for name, library in m['transitive_libraries'].items():
    library_path=pathlib.Path(library['path'])
    if sha(library_path)!=library['sha256']:
        raise SystemExit('transitive SDK digest mismatch: '+name)
    paths.append(library_path)
harness=w/'window8-initial-epoch-runtime.rs' 
paths += [harness, pathlib.Path(__file__).resolve(), w/'canonical-window8-scoped-coherent-sdk-closed.json']
paths += [pathlib.Path(p) for p in re.findall(r'#\[path="([^"]+)"\]',harness.read_text())]
for name, library in m['libraries'].items():
    library_path=pathlib.Path(library['path'])
    expected=library.get('sha256')
    if not expected or sha(library_path)!=expected:
        raise SystemExit('SDK digest mismatch: '+name)
    paths.append(library_path)
output=pathlib.Path(m['out_dir'])
paths += sorted(p for p in output.iterdir() if p.is_file())
paths += sorted((w/'dependency-runtime-fixture').iterdir())
repo=pathlib.Path('/home/dancxjo/conduit-4907-production-custody/semantics/language')
paths += list((repo/'tests/common').glob('*.rs'))
paths += list(repo.glob('*.conduit'))
paths += [repo/'training/ewt_proposal_v1/canonical-dictionary-overlay-v2'/f'shard-{n:03}.native.bin' for n in range(128)]
paths += [p for p in (repo/'training/ewt_proposal_v2/origin-corrected-v2').rglob('*') if p.is_file()]
before={str(p):sha(p) for p in dict.fromkeys(paths)}
c=['rustc','+stable','--edition=2024','--test','-C','debuginfo=0','-C','link-arg=-Wl,--strip-debug',str(w/'window8-initial-epoch-runtime.rs'),'-o',str(w/'window8-initial-epoch-test')]
for d in m['dependency_directories']:c+=['-L','dependency='+d]
for k,v in m['libraries'].items():c+=['--extern',k+'='+v['path']]
for k,v in m['transitive_libraries'].items():
    if k.startswith('sha2-'):c+=['--extern','sha2='+v['path']]
e=os.environ.copy();e['OUT_DIR']=m['out_dir'];e['CARGO_MANIFEST_DIR']=str(repo)
t=time.monotonic()
with (w/'window8-initial-epoch-compile.log').open('w') as f:r=subprocess.run(c,env=e,stdout=f,stderr=subprocess.STDOUT)
run=None
if r.returncode==0:
    binary=w/'window8-initial-epoch-test'
    before[str(binary)]=sha(binary)
    with (w/'window8-initial-epoch.log').open('w') as f:run=subprocess.run([str(binary),'--exact','actual_prepared_window8_initial_epoch','--nocapture','--test-threads=1'],stdout=f,stderr=subprocess.STDOUT)
x={'runtime_exit':None if run is None else run.returncode,'compile_exit':r.returncode,'seconds':time.monotonic()-t,'inputs':before,'unchanged':all(sha(pathlib.Path(p))==h for p,h in before.items()),'scope':'actual prepared factory and initial Partial stable14 single-epoch Session diagnostic; no successor/commit/whole goal acceptance','hashed_input_count':len(before),'sdk_manifest_sha256':sha(w/'canonical-window8-scoped-coherent-sdk-closed.json')}
(w/'window8-initial-epoch-result.json').write_text(json.dumps(x,indent=2));print(json.dumps({k:v for k,v in x.items() if k!='inputs'}))
raise SystemExit(r.returncode if r.returncode else (0 if x['unchanged'] and run is not None and run.returncode==0 else 2))
