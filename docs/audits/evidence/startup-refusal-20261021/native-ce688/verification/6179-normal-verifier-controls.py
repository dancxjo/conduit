from pathlib import Path
import tempfile,shutil,hashlib,json,subprocess
root=Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit');original=root/'work/6158-74-canonical-1280x800-preserved';source='74f64e12ff062cb991c891d2a162e125c7ed3944';verifier=root/'work/6179-verify-normal-startup.py'
original_seal=hashlib.sha256((original/'seal.json').read_bytes()).hexdigest();results=[]
for case in ['resealed-foreign-source','resealed-missing-play-acceptance','resealed-empty-serial-prefix']:
 with tempfile.TemporaryDirectory(prefix='6179-verifier-',dir=root/'work') as tmp:
  p=Path(tmp)/'packet';shutil.copytree(original,p,copy_function=lambda a,b:__import__('os').link(a,b));modified=[]
  def put(name,b):
   q=p/name;q.unlink();q.write_bytes(b);modified.append(name)
  if case=='resealed-foreign-source':
   name='journey-proof.json';d=json.loads((p/name).read_text());d['source_commit']='foreign-source';put(name,json.dumps(d).encode())
  elif case=='resealed-missing-play-acceptance':
   name='journey-serial.log';b=(p/name).read_bytes();assert b.count(b'CONDUIT_STARTUP_STAGE play accepted')==1;put(name,b.replace(b'CONDUIT_STARTUP_STAGE play accepted',b'CONDUIT_STARTUP_STAGE play declined'))
  else:
   name='journey-frames/manifest.json';d=json.loads((p/name).read_text());d['checkpoints'][0]['serial_byte_end']=0;put(name,json.dumps(d).encode())
  seal=json.loads((p/'seal.json').read_text())
  for name in modified:seal['hashes'][name]=hashlib.sha256((p/name).read_bytes()).hexdigest()
  put('seal.json',json.dumps(seal).encode());out=Path(tmp)/'output.json';r=subprocess.run(['/usr/bin/python3',str(verifier),str(p),source,str(out)],capture_output=True,text=True);assert r.returncode!=0 and not out.exists(),case
  results.append({'case':case,'exit_code':r.returncode,'output_absent':not out.exists(),'stderr':r.stderr})
assert hashlib.sha256((original/'seal.json').read_bytes()).hexdigest()==original_seal
(root/'outputs/6179-normal-verifier-negative-controls.json').write_text(json.dumps({'status':'verified','fixture_source':source,'original_seal_unchanged':True,'controls':results,'scope':'Three resealed documentary counterexamples; hashes are custody, not signatures or hostile authenticity.'},indent=2)+'\n');print('Three resealed documentary controls refused with no output; original seal unchanged')
