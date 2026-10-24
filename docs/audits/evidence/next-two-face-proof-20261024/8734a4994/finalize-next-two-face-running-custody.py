"""Complete source-bound Cargo run backtick custody after runner exits successfully."""
import pathlib,json,re,shlex,hashlib
ROOT=pathlib.Path.cwd();p=ROOT/'outputs/next-two-face-proof-local-proof.json';r=json.loads(p.read_bytes());assert r['status']=='passed'and len(r['steps'])==6
rows={pathlib.Path(e['path']).resolve():e for e in r['executables']}
for step in r['steps']:
 raw=(ROOT/step['log']).read_bytes();assert hashlib.sha256(raw).hexdigest()==step['sha256'];runs=[]
 for text in re.findall(r'^\s*Running `([^`]+)`\s*$',raw.decode(),re.M):
  argv=shlex.split(text);path=pathlib.Path(argv[0]);path=path if path.is_absolute()else pathlib.Path('/tmp/conduit-next-two-face-proof-source')/path;path=path.resolve();e=rows[path];assert hashlib.sha256(path.read_bytes()).hexdigest()==e['sha256'];runs.append({'argv':argv,'path':str(path),'sha256':e['sha256'],'bytes':e['bytes']})
 step['cargo_run_backtick_executables']=runs
assert len(r['steps'][0]['cargo_run_backtick_executables'])==1
r['Cargo_Running_parser_qualification']='Original runner captured test-parenthesis forms; post-exit finalizer additionally captures every raw Cargo run backtick argv and joins actual retained ELF SHA.'
p.write_text(json.dumps(r,indent=2)+'\n')
print('Actual backtick Running custody',sum(len(s['cargo_run_backtick_executables'])for s in r['steps']))
