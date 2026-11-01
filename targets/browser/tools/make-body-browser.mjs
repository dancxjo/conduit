// Internal adapter invoked by the ordinary checked Body build.
import {readFile,writeFile,mkdir,lstat} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {acquireHostRelease} from '../workspace/creche-release-bundle.mjs';
import {buildBrowserBundleImage} from '../deployment/browser/browser-bundle.mjs';
import {publishBrowserBodySpore} from './package-body-spore.mjs';
const [action,inputPath,source,destination]=process.argv.slice(2);
const input=JSON.parse(await readFile(inputPath,'utf8'));
if(action==='configure') {
  const releaseRoot=path.resolve(source);
  const release=await acquireHostRelease({target_id:'browser/wasm32/page',package_id:'browser-wasm@1',output:'browser-bundle',builder_adapter:'conduit-host-browser/build-wasm@1',deployment_adapter:'conduit-host-browser/load@1',manifest_path:pathToFileURL(path.join(releaseRoot,'browser-page.json')).href},undefined,{fetcher:async url=>{
    const file=fileURLToPath(url),stat=await lstat(file);
    if(path.dirname(file)!==releaseRoot||!stat.isFile()||stat.size>32*1024*1024)throw Error('Reviewed release input exceeds its bound');
    return new Response(await readFile(file));
  }});
  const configured=await buildBrowserBundleImage({checked:input,distribution:release});
  await mkdir(destination);
  for(const file of configured.payloads)await writeFile(path.join(destination,file.path),file.bytes);
  await writeFile(path.join(destination,'browser-bundle-release.json'),JSON.stringify(configured.manifest));
}else if(action==='package') {
  const receipt=await publishBrowserBodySpore(input);
  process.stdout.write(JSON.stringify(receipt));
}else throw Error('Unknown internal browser Body build stage');
