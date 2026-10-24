import pathlib,subprocess,json,os,time,hashlib,re,shutil
root=pathlib.Path.cwd();source=root/'work/6411-payload-ceiling-source';target=root/'work/6411-payload-ceiling-target';tmp=root/'work/omnibus-ready-proof-tmp';tmp.mkdir(exist_ok=True);git=lambda *a:subprocess.check_output(['git','-C',str(source),*a],text=True).strip();head='51c2dee0c0607585c6f20dc4991394909f613bc9';assert git('rev-parse','HEAD')==head and not git('status','--porcelain');archive=json.loads((root/'outputs/6411-before-omnibus-target-custody.json').read_bytes());assert pathlib.Path(archive['archive']).is_file()
env=dict(os.environ,CARGO_TARGET_DIR=str(target),TMPDIR=str(tmp),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_TERM_COLOR='never',CONDUIT_NATIVE_REFERENCE_PROOF_RETAIN='1');tests=['prepared_structured_tape','prepared_literal_children','prepared_structured_payload','prepared_expression_scratch_bounds','bme280_lifecycle','device_frames'];args=[v for t in tests for v in ['--test',t]]
commands=[('supported-resource-equality',['cargo','+stable','xtask','check','resource-reference-equality']),('core-reference',['cargo','+stable','test','--locked','-p','conduit-core','--test','resource_reference','--','--nocapture','--test-threads=1']),('constructor-regressions',['cargo','+stable','test','--locked','-p','conduit-plot',*args,'--','--nocapture','--test-threads=1']),('supported-display-output',['cargo','+stable','xtask','prove','environment','display-output']),('strict-owning-libraries',['cargo','+stable','clippy','--locked','-p','conduit-core','-p','conduit-plot','-p','conduit-std-host','--lib','--','-D','warnings']),('strict-constructor-targets',['cargo','+stable','clippy','--locked','-p','conduit-plot',*args,'--','-D','warnings']),('strict-generated-bindings',['cargo','+stable','clippy','--locked','-p','conduit-data','--test','native_reference_bindings','--','-D','warnings']),('strict-core-reference',['cargo','+stable','clippy','--locked','-p','conduit-core','--test','resource_reference','--','-D','warnings'])]
r={'source':head,'tree':git('rev-parse','HEAD^{tree}'),'branch':git('branch','--show-current'),'started':time.time(),'steps':[],'environment':{k:env[k]for k in ['CARGO_TARGET_DIR','TMPDIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG','CARGO_TERM_COLOR','CONDUIT_NATIVE_REFERENCE_PROOF_RETAIN']},'prior_exact_cf_target_preserved':{'archive':archive['archive'],'sha256':archive['archive_sha256'],'custody':'outputs/6411-before-omnibus-target-custody.json'},'root_inputs':{p:git('rev-parse',head+':'+p)for p in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo/config.toml']}};receipt=root/'outputs/omnibus-ready-combined-local-proof.json';r['status']='running';receipt.write_text(json.dumps(r,indent=2)+'\n')
for name,cmd in commands:
 free=shutil.disk_usage(root).free
 if free<7*1024**3:r.update(status='stopped-before-next-command-for-headroom',available_bytes=free);receipt.write_text(json.dumps(r,indent=2)+'\n');raise SystemExit('Headroom guard')
 log=root/f'work/omnibus-ready-{name}.log';print('START',name,free,flush=True)
 with log.open('wb')as f:p=subprocess.run(cmd,cwd=source,env=env,stdout=f,stderr=subprocess.STDOUT)
 b=log.read_bytes();text=b.decode(errors='replace');row={'name':name,'command':cmd,'exit_code':p.returncode,'log':str(log),'sha256':hashlib.sha256(b).hexdigest(),'before_available_bytes':free,'actual_passed_counts':[int(x)for x in re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',text)],'actual_test_Running':re.findall(r'^\s*Running .*?\(([^)]+)\)\s*$',text,re.M),'actual_command_Running':re.findall(r'^\s*Running `([^`]+)`',text,re.M)};r['steps'].append(row);r.update(status='running'if p.returncode==0 else'first-negative-preserved',clean_after=not git('status','--porcelain'));receipt.write_text(json.dumps(r,indent=2)+'\n');print(name,p.returncode,row['actual_passed_counts'],flush=True)
 if p.returncode:raise SystemExit('First negative preserved')
assert r['clean_after'];r.update(status='passed',finished=time.time(),executables=[]);paths=set()
for folder in [target,tmp,source/'target/xtask-host-release']:
 if not folder.exists():continue
 for p in folder.rglob('*'):
  if p.is_file()and os.access(p,os.X_OK):
   with p.open('rb')as f:magic=f.read(4)
   if magic==b'\x7fELF':paths.add(p.resolve())
for step in r['steps']:
 for p in [*step['actual_test_Running'],*[c.split(' ',1)[0]for c in step['actual_command_Running']]]:
  p=pathlib.Path(p);p=p if p.is_absolute()else source/p;paths.add(p.resolve())
for p in sorted(paths):
 b=p.read_bytes();assert b[:4]==b'\x7fELF';r['executables'].append({'path':str(p),'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
receipt.write_text(json.dumps(r,indent=2)+'\n');print('ALL PASS',len(r['executables']),flush=True)
