import os,sys,re,json,time,hashlib,shutil,subprocess,datetime
from pathlib import Path
arm=sys.argv[1]
if arm not in ('baseline','optimized'):raise SystemExit('unknown arm')
w=Path(__file__).resolve().parents[1];packet=Path(__file__).resolve().parent;cache=w/'domain-6183-build-target';repo=w/('6183-protection-'+arm);out=packet/arm;out.mkdir(exist_ok=True)
if (out/'first.log').exists():raise SystemExit('first attempt already exists; refusing repeat')
def git(*a):return subprocess.check_output(['git',*a],cwd=repo,text=True).strip()
source=git('rev-parse','HEAD');expected=json.loads((packet/'pair-preflight.json').read_text())[arm if arm=='baseline' else 'candidate'];assert source==expected;assert git('status','--porcelain')==''
for pid in os.listdir('/proc'):
 if not pid.isdigit():continue
 try:
  args=Path('/proc',pid,'cmdline').read_bytes().decode().split('\0')[:-1]
  if args and os.path.basename(args[0]) in ('cargo','rustc','xtask','conduit-xtask-dispatch','qemu-system-x86_64') and any(str(cache) in a for a in args):raise SystemExit('owned compiler cache unexpectedly active:'+pid)
 except (OSError,UnicodeError):pass
# Retain prior producers before invalidating only fingerprint metadata. No
# executable, guest ELF, ISO, raw evidence or build-output directory is deleted.
saved=[]
for relative in ['debug/xtask','debug/conduit-xtask-dispatch','x86_64-unknown-none/release/conduitos']:
 p=cache/relative
 if p.is_file():
  q=out/'prior-cache-producers'/relative;q.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,q);saved.append({'path':str(q),'sha256':hashlib.sha256(q.read_bytes()).hexdigest()})
invalidated=[]
for f in list(cache.rglob('.fingerprint')):
 for p in list(f.iterdir()):
  if p.is_dir() and re.fullmatch(r'(conduitos|xtask|conduit-xtask-dispatch)-[0-9a-f]+',p.name):
   q=out/'producer-fingerprints'/p.relative_to(cache);q.parent.mkdir(parents=True,exist_ok=True);p.rename(q);invalidated.append(str(p.relative_to(cache)))
env=os.environ.copy();env.update(CARGO_TARGET_DIR=str(cache),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CONDUIT_CONDUITOS_TARGET_ROOT=str(out/'native'))
cmd=['cargo','+stable','xtask','make','conduitos','ordinary-domain-proof','--locked'];stat=os.statvfs(cache)
meta={'arm':arm,'source':source,'command':cmd,'cwd':str(repo),'artifact_root':str(out/'native'),'cache':str(cache),'jobs':2,'incremental':False,'started_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'producer_fingerprints_archived':invalidated,'prior_producers_retained':saved,'filesystem_free_bytes':stat.f_bavail*stat.f_frsize,'status':'running','scope':'native protection only; no timings, normal product or full6183 claim'}
(out/'run.json').write_text(json.dumps(meta,indent=2)+'\n');(packet/'cache-owner.json').write_text(json.dumps({'owner':'budget_audit','arm':arm,'pid':os.getpid(),'status':'running'},indent=2)+'\n')
with (out/'first.log').open('wb') as log:
 p=subprocess.Popen(cmd,cwd=repo,env=env,stdout=log,stderr=subprocess.STDOUT);rc=p.wait()
meta.update(exit_code=rc,status='success' if rc==0 else 'failed',finished_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),source_after=git('rev-parse','HEAD'),status_after=git('status','--porcelain'),raw_stdout_sha256=hashlib.sha256((out/'first.log').read_bytes()).hexdigest());(out/'run.json').write_text(json.dumps(meta,indent=2)+'\n');(packet/'cache-owner.json').write_text(json.dumps({'owner':'budget_audit','arm':arm,'status':'terminal','exit_code':rc},indent=2)+'\n');print(json.dumps(meta));raise SystemExit(rc)
