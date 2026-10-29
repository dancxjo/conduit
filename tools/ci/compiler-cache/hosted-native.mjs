import {promises as fs} from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {CacheRefusal} from './refusal.mjs';
const fail=code=>{throw new CacheRefusal(code);};
const options={encoding:'utf8',timeout:30000,maxBuffer:1024*1024,env:{PATH:'/usr/bin:/bin',LANG:'C'}};
function run(program,args){const result=spawnSync(program,args,options);if(result.error||result.signal||result.status!==0)fail('hosted-native-discovery-failed');return result.stdout.trim();}
export function nativeRuntimePath(file){
 file=path.normalize(file);
 if(file.startsWith('/usr/'))return file.slice(1);
 if(file.startsWith('/lib/'))return 'usr'+file;
 if(file.startsWith('/lib64/'))return 'usr'+file;
 fail('unsupported-hosted-native-path');
}
export function parseNativeDependencies(output){
 const files=[];
 for(const raw of output.trim().split('\n')){
  const line=raw.trim();if(!line||/^linux-vdso\S* \(0x[\da-f]+\)$/.test(line))continue;
  if(line==='statically linked'||line==='not a dynamic executable')continue;
  const match=/^(?:\S+ => )?(\/\S+) \(0x[\da-f]+\)$/.exec(line);
  if(!match)fail('unsupported-hosted-native-dependency');files.push(path.normalize(match[1]));
 }
 return [...new Set(files)].sort();
}
export function nativeGccSelection({target,major,libgcc,cc1,collect2}){
 if(target!=='x86_64-linux-gnu'||!/^1[2-5]$/.test(major)||![libgcc,cc1,collect2].every(file=>path.isAbsolute(file)&&file.startsWith('/usr/')))fail('unsupported-hosted-native-toolchain');
 const directory=path.dirname(path.normalize(libgcc));
 if(directory!==`/usr/lib/gcc/${target}/${major}`)fail('unsupported-hosted-native-toolchain');
 const trees=[directory];
 for(const file of [cc1,collect2]){const parent=path.dirname(path.normalize(file));if(!trees.includes(parent))trees.push(parent);}
 if(trees.some(directory=>!new RegExp(`^/usr/(?:lib|libexec)/gcc/${target}/${major}$`).test(directory)))fail('unsupported-hosted-native-toolchain');
 return {target,major,trees};
}

// Capture named GCC/linker/header inputs plus actual ELF dependency resolution.
// This admits concrete Debian/Ubuntu x86_64 GCC12-15 layouts; no host directory
// is mounted, and neither a missing payload nor unknown loader output falls back.
export async function captureHostedNativeClosure({destination,toolchainRoot,engine,copy}){
 const gcc='/usr/bin/gcc';
 const selection=nativeGccSelection({target:run(gcc,['-dumpmachine']),major:run(gcc,['-dumpversion']),libgcc:run(gcc,['-print-libgcc-file-name']),cc1:run(gcc,['-print-prog-name=cc1']),collect2:run(gcc,['-print-prog-name=collect2'])});
 const runtime=path.join(destination,'runtime'),seeds=new Set(),copied=new Set();
 const capture=async(input,relative=nativeRuntimePath(input))=>{
  const key=path.normalize(relative);if(copied.has(key))return;copied.add(key);
  if(copied.size>50000)fail('hosted-native-file-bound');
  const stat=await fs.stat(input);if(!stat.isFile())fail('unsupported-hosted-native-file');
  await copy(input,path.join(runtime,key),Boolean(stat.mode&0o111));seeds.add(input);
 };
 async function tree(directory){
  for(const name of(await fs.readdir(directory)).sort()){
   const file=path.join(directory,name),stat=await fs.stat(file);
   if(stat.isDirectory())await tree(file);else if(stat.isFile())await capture(file);else fail('unsupported-hosted-native-file');
  }
 }
 for(const name of ['cc','gcc',`gcc-${selection.major}`])await capture(gcc,'usr/bin/'+name);
 for(const name of ['as','ld','ar','ranlib'])await capture('/usr/bin/'+name);
 for(const directory of selection.trees)await tree(directory);
 for(const name of ['crt1.o','Scrt1.o','crti.o','crtn.o','libc.so','libc_nonshared.a','libm.so','libmvec.so.1','libpthread.a','libdl.a','librt.a','libutil.a','libutil.so.1','libresolv.so.2']){
  const found=run(gcc,['-print-file-name='+name]);if(!path.isAbsolute(found))fail('missing-hosted-native-library');await capture(path.normalize(found));
 }
 for(const name of(await fs.readdir('/usr/include')).sort()){
  const file=path.join('/usr/include',name),stat=await fs.stat(file);
  if(stat.isFile())await capture(file);else if(['x86_64-linux-gnu','linux','asm-generic'].includes(name))await tree(file);
 }
 for(const name of ['cargo','rustc'])seeds.add(path.join(toolchainRoot,'bin',name));
 for(const name of await fs.readdir(path.join(toolchainRoot,'lib')))if(name.startsWith('librustc_driver-')||name.startsWith('libLLVM'))seeds.add(path.join(toolchainRoot,'lib',name));
 for(const name of ['rust-lld','gcc-ld/ld.lld'])seeds.add(path.join(toolchainRoot,'lib/rustlib/x86_64-unknown-linux-gnu/bin',name));
 for(const seed of [engine,'/usr/bin/dash','/usr/bin/sleep'])seeds.add(seed);
 const examined=new Set();
 while(seeds.size){
  const file=seeds.values().next().value;seeds.delete(file);if(examined.has(file))continue;examined.add(file);
  const handle=await fs.open(file,'r');let magic;try{const buffer=Buffer.alloc(4);await handle.read(buffer,0,4,0);magic=buffer.toString('hex');}finally{await handle.close();}
  if(magic!=='7f454c46')continue;
  const result=spawnSync('/usr/bin/ldd',[file],options);const output=(result.stdout??'')+'\n'+(result.stderr??'');
  if(result.error||result.signal||(result.status!==0&&!/^\s*(?:not a dynamic executable|statically linked)\s*$/.test(output)))fail('hosted-native-loader-discovery-failed');
  for(const dependency of parseNativeDependencies(output)){
   if(dependency.startsWith(toolchainRoot+'/')){seeds.add(dependency);continue;}
   await capture(dependency);
  }
 }
 const policy={schema:1,target:selection.target,gccMajor:Number(selection.major),gccTrees:selection.trees.map(nativeRuntimePath),discovery:'Exact named GCC/linker/header files plus actual trusted-host ELF loader closure; no ambient library directory mounts',fileCount:copied.size};
 await fs.writeFile(path.join(destination,'native-policy.json'),JSON.stringify(policy),{flag:'wx',mode:0o444});return policy;
}
