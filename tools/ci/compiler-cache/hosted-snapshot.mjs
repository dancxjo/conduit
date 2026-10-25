import { promises as fs, createReadStream } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { CacheRefusal } from './refusal.mjs';
import { copyCompilerClosure, copyRegular, snapshotInventory, hashFile } from './cargo-snapshot.mjs';
import { SCCACHE_ENGINE } from './engine.mjs';

export const HOSTED_SNAPSHOT_LIMITS = Object.freeze({ maxBytes: 6 * 1024 ** 3, maxFiles: 150000 });
const fail = code => { throw new CacheRefusal(code); };
const PINNED_TONGUES=JSON.parse(await fs.readFile(new URL('./pinned-tongues.json',import.meta.url),'utf8'));
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
function run(program, args, options = {}) {
  const result = spawnSync(program, args, { maxBuffer: 16 * 1024 * 1024, timeout: 120000, ...options });
  if (result.error || result.status !== 0 || result.signal) throw Object.assign(new CacheRefusal('hosted-snapshot-command-failed'), { diagnostic: result.stderr?.toString().slice(-4096) });
  return result.stdout;
}

// These acquisition steps happen before the immutable offline execution root
// exists. Cached .crate archives are authenticated against the exact Cargo.lock,
// then extracted afresh (never trust mutable ~/.cargo/registry/src contents).
const VENDOR = String.raw`
import sys,json,tomllib,hashlib,tarfile,pathlib,io
lock=tomllib.load(open(sys.argv[1],'rb'))
checks={(p['name'],p['version']):p['checksum'] for p in lock['package'] if p.get('source','').startswith('registry+')}
root=pathlib.Path(sys.argv[2]); cache=pathlib.Path(sys.argv[3]); total=0; count=0
for p in lock['package']:
 if not p.get('source','').startswith('registry+'): continue
 if p['source']!='registry+https://github.com/rust-lang/crates.io-index': raise RuntimeError('unsupported registry or git dependency')
 name=p['name']+'-'+p['version']; expected=checks[(p['name'],p['version'])]
 candidates=list(cache.glob('*/'+name+'.crate'))
 valid=[f for f in candidates if f.stat().st_size<=256*1024*1024 and hashlib.sha256(f.read_bytes()).hexdigest()==expected]
 if not valid: raise RuntimeError('missing authenticated crate archive: '+name)
 data=valid[0].read_bytes(); destination=root/name; destination.mkdir(); hashes={}
 with tarfile.open(fileobj=io.BytesIO(data),mode='r:gz') as archive:
  members=archive.getmembers()
  for item in members:
   rel=pathlib.PurePosixPath(item.name)
   if not rel.parts or rel.parts[0]!=name or '..' in rel.parts or rel.is_absolute(): raise RuntimeError('unsafe crate path')
   if item.isdir(): continue
   if not item.isfile() or item.size>128*1024*1024: raise RuntimeError('unsupported crate member')
   rel=pathlib.Path(*rel.parts[1:]); total+=item.size; count+=1
   if total>3*1024**3 or count>100000 or not rel.parts: raise RuntimeError('vendor bound')
   target=destination/rel; target.parent.mkdir(parents=True,exist_ok=True)
   body=archive.extractfile(item).read()
   with target.open('xb') as output: output.write(body)
   target.chmod(0o555 if item.mode&0o111 else 0o444); hashes[rel.as_posix()]=hashlib.sha256(body).hexdigest()
 (destination/'.cargo-checksum.json').write_text(json.dumps({'package':expected,'files':hashes},sort_keys=True))
print(json.dumps({'packages':len(checks),'files':count,'bytes':total}))
`;

// Only lock-authenticated archives enter the immutable registry source. Mutable
// registry/src trees and their advisory checksum metadata are never trusted.
export async function extractAuthenticatedRegistry({lockFile,vendorRoot,archiveCache}) {
  if(![lockFile,vendorRoot,archiveCache].every(value=>typeof value==='string'&&path.isAbsolute(value)))fail('invalid-vendor-acquisition-policy');
  return JSON.parse(run('/usr/bin/python3',['-c',VENDOR,lockFile,vendorRoot,archiveCache],{maxBuffer:1024*1024}));
}

export async function prepareHostedSnapshot({ repositoryRoot, toolchainRoot, engine, destination, cargoHome = path.join(process.env.HOME, '.cargo') }) {
  if (process.platform !== 'linux' || process.arch !== 'x64' || ![repositoryRoot,toolchainRoot,engine,destination,cargoHome].every(value => typeof value === 'string' && path.isAbsolute(value))) fail('unsupported-hosted-snapshot-policy');
  repositoryRoot=await fs.realpath(repositoryRoot);toolchainRoot=await fs.realpath(toolchainRoot);engine=await fs.realpath(engine);
  destination=path.join(await fs.realpath(path.dirname(destination)),path.basename(destination));
  if ([repositoryRoot,toolchainRoot,cargoHome].some(root=>destination===root||destination.startsWith(root+'/'))) fail('overlapping-hosted-snapshot');
  if (await hashFile(engine)!==SCCACHE_ENGINE.binarySha256) fail('engine-binary-mismatch');
  const free=await fs.statfs(path.dirname(destination),{bigint:true});if(free.bavail*free.bsize<19n*1024n**3n)fail('hosted-snapshot-reserve');
  const git=args=>run('git',['--no-replace-objects','-C',repositoryRoot,...args]).toString().trim();
  const sourceCommit=git(['rev-parse','HEAD']);const sourceTree=git(['rev-parse','HEAD^{tree}']);
  const entries=git(['ls-tree','-rl','HEAD']).split('\n');
  if(entries.length>20000||entries.some(line=>!/^100(644|755) blob [a-f0-9]{40}\s+\d+\t/.test(line)))fail('unsupported-hosted-source-tree');
  await fs.mkdir(destination,{mode:0o700});
  const source=path.join(destination,'source');await fs.mkdir(source);
  // Archive the named commit itself: dirty controller files never get relabelled
  // as captured product source. No untracked source or ambient Cargo home enters.
  const archive=run('git',['--no-replace-objects','-C',repositoryRoot,'archive','--format=tar',sourceCommit],{maxBuffer:512*1024**2});
  run('/usr/bin/tar',['-x','--no-same-owner','--no-same-permissions','-C',source],{input:archive});
  const acquisitionEnv={PATH:'/usr/bin:/bin',HOME:process.env.HOME,CARGO_HOME:cargoHome,RUSTC:path.join(toolchainRoot,'bin/rustc'),RUSTC_WRAPPER:'',RUSTC_WORKSPACE_WRAPPER:''};
  const acquisitionCargoSha256=await hashFile(path.join(toolchainRoot,'bin/cargo'));
  const acquired=JSON.parse(run(path.join(toolchainRoot,'bin/cargo'),['metadata','--locked','--offline','--format-version','1'],{cwd:source,env:acquisitionEnv,maxBuffer:32*1024**2}));
  const metadata=path.join(destination,'metadata.json');await fs.writeFile(metadata,JSON.stringify(acquired));
  const bypassNames=proceduralMacroBypassNames(acquired);
  await fs.writeFile(path.join(destination,'proc-macro-bypass.txt'),bypassNames.join('\n')+'\n',{mode:0o444,flag:'wx'});
  const vendor=path.join(destination,'vendor');await fs.mkdir(vendor);
  const vendorReceipt=await extractAuthenticatedRegistry({lockFile:path.join(source,'Cargo.lock'),vendorRoot:vendor,archiveCache:path.join(cargoHome,'registry/cache')});
  const gitPackages=acquired.packages.filter(pkg=>pkg.source?.startsWith('git+'));
  const gitSource=PINNED_TONGUES.source;
  if(gitPackages.length!==2||gitPackages.some(pkg=>pkg.source!==gitSource||!['speaking','tongues-pipeline'].includes(pkg.name)))fail('unsupported-hosted-git-input');
  // Preserve the pinned Git workspace layout: speaking includes shared
  // ../../../fixtures which a flat Cargo vendor directory would omit.
  const revision=gitSource.split('#')[1];
  const checkout=run('git',['-C',path.dirname(gitPackages[0].manifest_path),'rev-parse','--show-toplevel']).toString().trim();
  if(run('git',['--no-replace-objects','-C',checkout,'rev-parse','HEAD']).toString().trim()!==revision)fail('hosted-git-revision-mismatch');
  const gitTree=run('git',['--no-replace-objects','-C',checkout,'rev-parse',`${revision}^{tree}`]).toString().trim();
  const gitRoot=path.join(destination,'git-vendor/tongues');await fs.mkdir(gitRoot,{recursive:true});
  const gitArchive=run('git',['--no-replace-objects','-C',checkout,'archive','--format=tar',revision],{maxBuffer:128*1024**2});
  run('/usr/bin/tar',['-x','--no-same-owner','--no-same-permissions','-C',gitRoot],{input:gitArchive});
  // Public Tongues has six deliberately external private-data links. They are
  // never followed, copied or mounted; their absence is part of this contract.
  const excludedSymlinks=['archive','data','datasets','listenbury','models','runs'];
  for(const name of excludedSymlinks){
    const link=path.join(gitRoot,name);if(!(await fs.lstat(link)).isSymbolicLink())fail('hosted-git-link-contract');
    const expected=run('git',['--no-replace-objects','-C',checkout,'show',`${revision}:${name}`]);
    if(!expected.equals(Buffer.from(await fs.readlink(link))))fail('hosted-git-link-contract');
    await fs.unlink(link);
  }
  const gitCustody=await verifyHostedSource({repositoryRoot:checkout,sourceRoot:gitRoot,sourceCommit:revision,sourceTree:gitTree,excludedSymlinks});
  const normalizations=[];
  for(const pkg of gitPackages){
    const prefix=path.relative(checkout,path.dirname(pkg.manifest_path));
    const relative=`${prefix}/Cargo.toml`;
    if(!['crates/speaking/Cargo.toml','crates/tongues-pipeline/Cargo.toml'].includes(relative))fail('unsupported-hosted-git-manifest');
    const normalized=await fs.readFile(new URL(`./pinned-tongues/${pkg.name}.toml`,import.meta.url),'utf8');
    const original=await fs.readFile(path.join(gitRoot,relative));
    await fs.writeFile(path.join(gitRoot,relative),normalized);
    normalizations.push({path:relative,originalSha256:digest(original),sha256:await hashFile(path.join(gitRoot,relative)),policy:'Pinned Cargo-normalized manifest for this exact Git source'});
  }
  // A Cargo directory source scans every sibling manifest, including unused
  // Tongues workspace packages. Give these unchanged public sources explicit
  // generated checksums; only the two selected manifests are normalized.
  for(const name of await fs.readdir(path.join(gitRoot,'crates'))){
    const directory=path.join(gitRoot,'crates',name);
    try{await fs.stat(path.join(directory,'Cargo.toml'));}catch(error){if(error.code==='ENOENT')continue;throw error;}
    try{await fs.stat(path.join(directory,'.cargo-checksum.json'));continue;}catch(error){if(error.code!=='ENOENT')throw error;}
    const inventory=await snapshotInventory(directory,HOSTED_SNAPSHOT_LIMITS);
    const checksum=path.join(directory,'.cargo-checksum.json');
    await fs.writeFile(checksum,JSON.stringify({package:null,files:Object.fromEntries(inventory.files.map(file=>[file.path,file.sha256]))}),{flag:'wx',mode:0o444});
    normalizations.push({path:`crates/${name}/.cargo-checksum.json`,sha256:await hashFile(checksum)});
  }
  await fs.writeFile(path.join(destination,'git-source-custody.json'),JSON.stringify({source:gitSource,...gitCustody,normalizations},null,2),{flag:'wx',mode:0o444});
  const vendorConfig='[source.crates-io]\nreplace-with = "frozen-vendor"\n\n[source."'+gitSource.split('#')[0]+'"]\ngit = "https://github.com/dancxjo/tongues.git"\nrev = "'+revision+'"\nreplace-with = "frozen-tongues"\n\n[source.frozen-vendor]\ndirectory = "/vendor"\n\n[source.frozen-tongues]\ndirectory = "/git-vendor/tongues/crates"\n';
  let copiedBytes=0,copiedFiles=0;
  const copy=async(input,output,executable=false)=>{
    const stat=await fs.stat(input);copiedBytes+=stat.size;copiedFiles++;
    if(copiedBytes>3*1024**3||copiedFiles>50000)fail('hosted-runtime-bound');
    await copyRegular(input,output,executable);
  };
  await copyCompilerClosure({toolchainRoot,engine,destination,copy});
  for(const name of ['rust-lld','gcc-ld/ld.lld'])await copy(path.join(toolchainRoot,'lib/rustlib/x86_64-unknown-linux-gnu/bin',name),path.join(destination,'toolchain/lib/rustlib/x86_64-unknown-linux-gnu/bin',name),true);
  // A concrete Debian x86_64 GCC 15 C/link closure, not a host /usr mount.
  // Resolved symlink payloads are copied into their named lookup locations.
  const runtime=path.join(destination,'runtime');
  async function tree(input,output) {
    for(const name of(await fs.readdir(input)).sort()) {
      const src=path.join(input,name), dst=path.join(output,name);const stat=await fs.stat(src);
      if(stat.isDirectory())await tree(src,dst);else if(stat.isFile())await copy(src,dst,Boolean(stat.mode&0o111));else fail('unsupported-hosted-runtime-file');
    }
  }
  for(const [input,output] of [['/usr/bin/cc','usr/bin/cc'],['/usr/bin/gcc-15','usr/bin/gcc'],['/usr/bin/gcc-15','usr/bin/gcc-15'],['/usr/bin/as','usr/bin/as'],['/usr/bin/ld','usr/bin/ld'],['/usr/bin/ar','usr/bin/ar'],['/usr/bin/ranlib','usr/bin/ranlib']])await copy(input,path.join(runtime,output),true);
  for(const directory of ['lib/gcc/x86_64-linux-gnu/15','libexec/gcc/x86_64-linux-gnu/15'])await tree('/usr/'+directory,path.join(runtime,'usr',directory));
  for(const name of ['libbfd-2.46-system.so','libctf.so.0','libjansson.so.4','libzstd.so.1','libsframe.so.3','libisl.so.23','libmpc.so.3','libmpfr.so.6','libgmp.so.10','libstdc++.so.6','libmvec.so.1','crt1.o','Scrt1.o','crti.o','crtn.o','libc.so','libc_nonshared.a','libm.so','libpthread.a','libdl.a','librt.a','libutil.a','libutil.so.1','libresolv.so.2'])await copy('/usr/lib/x86_64-linux-gnu/'+name,path.join(runtime,'usr/lib/x86_64-linux-gnu',name));
  for(const name of await fs.readdir('/usr/include')) {
    const input=path.join('/usr/include',name),output=path.join(runtime,'usr/include',name);const stat=await fs.stat(input);
    if(stat.isFile())await copy(input,output);else if(['x86_64-linux-gnu','linux','asm-generic'].includes(name))await tree(input,output);
  }
  await fs.writeFile(path.join(destination,'cargo-config.toml'),vendorConfig,{mode:0o444,flag:'wx'});
  if(await hashFile(path.join(destination,'toolchain/bin/cargo'))!==acquisitionCargoSha256)fail('hosted-acquisition-cargo-mutated');
  await verifyHostedSource({repositoryRoot,sourceRoot:source,sourceCommit,sourceTree});
  const snapshot=await snapshotInventory(destination,HOSTED_SNAPSHOT_LIMITS);
  return {schema:'hosted-linux-cargo-snapshot-v1',root:destination,host:'x86_64-unknown-linux-gnu',sourceCommit,sourceTree,...snapshot,vendor:vendorReceipt,procMacroBypass:bypassNames,package:{name:'conduit',command:['build','--locked','--release','-p','conduit','--bin','conduit'],proceduralMacroPolicy:'Bypass all transitive procedural-macro dependencies, including reexports; response files refuse'},acquisitionPolicySha256:digest(VENDOR)};
}

// Resolver features intentionally overapproximate the fixed package build.
// Reexports and indirect macro dependencies must never escape the bypass set.
export function proceduralMacroBypassNames(metadata) {
  if(!Array.isArray(metadata?.packages)||!Array.isArray(metadata?.resolve?.nodes)||metadata.packages.length>2000||metadata.resolve.nodes.length>2000)fail('unsupported-hosted-dependency-graph');
  const macroIds=new Set(metadata.packages.filter(pkg=>pkg.targets.some(target=>target.kind.includes('proc-macro'))).map(pkg=>pkg.id));
  let changed=true;
  while(changed){changed=false;for(const node of metadata.resolve.nodes)if(!macroIds.has(node.id)&&node.deps.some(dep=>macroIds.has(dep.pkg))){macroIds.add(node.id);changed=true;}}
  return [...new Set(metadata.packages.filter(pkg=>macroIds.has(pkg.id)).map(pkg=>pkg.name))].sort();
}

// Validate copied archive bytes, including export-ignore/export-subst refusal,
// against the actual raw commit/tree Merkle closure rather than a file count.
export async function verifyHostedSource({repositoryRoot,sourceRoot,sourceCommit,sourceTree,excludedSymlinks=[]}) {
  if(!/^[a-f0-9]{40}$/.test(sourceCommit)||!/^[a-f0-9]{40}$/.test(sourceTree))fail('invalid-hosted-git-custody');
  const git=args=>run('git',['--no-replace-objects','-C',repositoryRoot,...args]);
  const objectId=(kind,bytes)=>createHash('sha1').update(`${kind} ${bytes.length}\0`).update(bytes).digest('hex');
  const commit=git(['cat-file','commit',sourceCommit]);
  if(objectId('commit',commit)!==sourceCommit||commit.toString().split('\n')[0]!==`tree ${sourceTree}`)fail('hosted-source-commit-mismatch');
  const records=git(['ls-tree','-r','-t','-z',sourceTree]).toString().split('\0').filter(Boolean);
  const trees=new Set([sourceTree]),expected=new Map(),excluded=[];
  for(const record of records){
    const match=/^(040000|100644|100755|120000) (tree|blob) ([a-f0-9]{40})\t([\s\S]+)$/.exec(record);
    if(!match)fail('unsupported-hosted-source-tree');
    if(match[1]==='120000'){
      if(!excludedSymlinks.includes(match[4]))fail('unsupported-hosted-source-tree');
      const target=git(['cat-file','blob',match[3]]);
      if(objectId('blob',target)!==match[3])fail('hosted-source-link-mismatch');
      try{await fs.lstat(path.join(sourceRoot,match[4]));fail('hosted-source-link-present');}catch(error){if(error.code!=='ENOENT')throw error;}
      excluded.push({path:match[4],blobOid:match[3],target:target.toString(),policy:'External private input remains absent; reads must refuse'});continue;
    }
    if(match[2]==='tree')trees.add(match[3]);else expected.set(match[4],{oid:match[3],executable:match[1]==='100755'});
  }
  if(excluded.length!==excludedSymlinks.length)fail('hosted-source-link-contract');
  const ids=[...trees];const rawTrees=run('git',['--no-replace-objects','-C',repositoryRoot,'cat-file','--batch'],{input:ids.join('\n')+'\n'});
  let cursor=0;
  for(const oid of ids){
    const end=rawTrees.indexOf('\n',cursor);const header=rawTrees.subarray(cursor,end).toString().split(' ');const size=Number(header[2]);
    if(header[0]!==oid||header[1]!=='tree'||!Number.isSafeInteger(size)||size<0)fail('hosted-source-tree-mismatch');
    const raw=rawTrees.subarray(end+1,end+1+size);
    if(objectId('tree',raw)!==oid||rawTrees[end+1+size]!==10)fail('hosted-source-tree-mismatch');
    cursor=end+size+2;
  }
  if(cursor!==rawTrees.length)fail('hosted-source-tree-mismatch');
  const actual=await snapshotInventory(sourceRoot,HOSTED_SNAPSHOT_LIMITS);
  if(actual.files.length!==expected.size)fail('hosted-source-inventory-mismatch');
  for(const file of actual.files){
    const entry=expected.get(file.path);
    if(!entry||entry.executable!==file.executable)fail('hosted-source-inventory-mismatch');
    const hash=createHash('sha1').update(`blob ${file.bytes}\0`);
    for await(const bytes of createReadStream(path.join(sourceRoot,file.path)))hash.update(bytes);
    if(hash.digest('hex')!==entry.oid)fail('hosted-source-blob-mismatch');
  }
  return {sourceCommit,sourceTree,files:actual.files.length,sourceFilesSha256:actual.sha256,...(excluded.length?{excludedSymlinks:excluded}:{})};
}
