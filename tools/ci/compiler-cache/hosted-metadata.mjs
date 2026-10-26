import path from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import { CacheRefusal } from './refusal.mjs';
const fail=()=>{throw new CacheRefusal('unsupported-hosted-metadata-path');};
// Cargo acquisition occurs before freezing and therefore names temporary and
// user-specific roots. These are provenance, not compiler inputs. Keep all graph
// information while mapping only actual Cargo path fields/IDs to sandbox paths.
export function canonicalHostedMetadata(metadata,{sourceRoot,gitCheckout}) {
  if(metadata?.version!==1||!Array.isArray(metadata.packages)||metadata.packages.length>2000||![sourceRoot,gitCheckout].every(value=>typeof value==='string'&&path.isAbsolute(value)&&path.normalize(value)===value))fail();
  const rules=new Map([[sourceRoot,'/source'],[path.join(sourceRoot,'target'),'/task/target'],[gitCheckout,'/git-vendor/tongues']]);
  for(const pkg of metadata.packages){
    if(typeof pkg.name!=='string'||typeof pkg.version!=='string'||!path.isAbsolute(pkg.manifest_path??''))fail();
    const root=path.dirname(pkg.manifest_path);
    if(pkg.source==='registry+https://github.com/rust-lang/crates.io-index'){
      const name=`${pkg.name}-${pkg.version}`;
      if(!/^[A-Za-z0-9_.+-]+$/.test(name)||path.basename(root)!==name)fail();
      const prior=rules.get(root),target=`/vendor/${name}`;
      if(prior&&prior!==target)fail();rules.set(root,target);
    }else if(pkg.source===null){if(root!==sourceRoot&&!root.startsWith(sourceRoot+'/'))fail();}
    else if(typeof pkg.source==='string'&&pkg.source.startsWith('git+https://github.com/dancxjo/tongues.git?rev=')){
      if(!root.startsWith(gitCheckout+'/'))fail();
    }else fail();
  }
  const ordered=[...rules].sort((a,b)=>b[0].length-a[0].length);
  const absolute=value=>{
    if(!path.isAbsolute(value))fail();value=path.normalize(value);
    for(const [input,output]of ordered)if(value===input||value.startsWith(input+'/'))return output+value.slice(input.length);
    fail();
  };
  const pathKeys=new Set(['manifest_path','src_path','path','workspace_root','target_directory','build_directory','license_file','readme']);
  function walk(value,key){
    if(typeof value==='string'){
      if(value.startsWith('path+file://')){
        const start='path+file://'.length,end=value.indexOf('#',start);
        if(end===-1)fail();
        let file;try{file=fileURLToPath(value.slice('path+'.length,end));}catch{fail();}
        return 'path+'+pathToFileURL(absolute(file)).href+value.slice(end);
      }
      if(pathKeys.has(key)&&path.isAbsolute(value))return absolute(value);
      return value;
    }
    if(Array.isArray(value))return value.map(item=>walk(item,key));
    if(value&&typeof value==='object')return Object.fromEntries(Object.keys(value).sort().map(name=>[name,walk(value[name],name)]));
    return value;
  }
  return walk(metadata);
}
