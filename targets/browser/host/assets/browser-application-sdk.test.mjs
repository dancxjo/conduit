import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
const chromium=process.env.CONDUIT_TEST_CHROMIUM;
// Real browser materialization with SDK/importer doubles: composition proof,
// not runtime admission, membership, or Face execution.
test('application import selects admitted policy and preserves persistent identity configuration',{skip:!chromium},async()=>{
 const source=await readFile(new URL('./browser-application-sdk.mjs',import.meta.url),'utf8');
 const script=`
import {admitApplicationSdk} from '/composition.mjs';
const encode=v=>new TextEncoder().encode(typeof v==='string'?v:JSON.stringify(v));
const resources=[],bytes=new Map();
function add(role,path,value){resources.push({role,path,kind:'content',url:new URL('/'+path,location.href),dependencies:[]});bytes.set(role,encode(value));}
add('browser-sdk','sdk/browser-sdk.mjs','export const Conduit={browser:async o=>{if(o.application.identity!=="app/test"||o.durable!==true||o.reviewedRelease.package.id!=="selected")throw Error("continuity/policy");if(o.root==="fail")throw Error("Boot refusal");return {hostId:"persistent"};}};');
add('continuity','sdk/browser-sdk-continuity.mjs','export const applicationContinuity=async a=>({identity:a.identity});');
add('importer','sdk/workspace/creche-browser-spore-import.mjs','export async function importBrowserSpore(b,o){if(o.expectedBodyId!=="body/test"||o.nowMillis!==1000||o.reviewedRelease.package.id!=="selected")throw Error("policy/scope");const secret=[2];globalThis.detachedSecret=secret;return {provision:{secret},browserOptions:{assetMap:new Map(),reviewedRelease:o.reviewedRelease}};}');
add('pkg','sdk/bundle/browser-sdk-package.json',{id:'selected'});
add('distribution','sdk/bundle/browser-page.json',{});
add('release','sdk/bundle/browser-bundle-release.json',{});
try{
const sdk=await admitApplicationSdk({resources,applicationId:'app/test',stateCompatibility:{identity:'state/test',version:1},packageDigest:'sha256:test'},bytes);
const options={nowMillis:1000,expectedBodyId:'body/test',reviewedRelease:{package:{id:'attacker'}}};
const result=await sdk.importBrowserSpore(new Uint8Array([1]),options);
if(result.host.hostId!=='persistent'||result.provision.secret[0]!==2||'membership' in result)throw Error('inert result');
bytes.delete('pkg');let refused=false;try{await sdk.importBrowserSpore(new Uint8Array([1]),options);}catch{refused=true;}if(!refused)throw Error('missing policy');
bytes.set('pkg',encode({id:'selected'}));refused=false;try{await sdk.importBrowserSpore(new Uint8Array([1]),{...options,expectedBodyId:'foreign'});}catch{refused=true;}if(!refused)throw Error('foreign Body');
refused=false;try{await sdk.importBrowserSpore(new Uint8Array([1]),{nowMillis:1000});}catch{refused=true;}if(!refused)throw Error('missing explicit Body');
refused=false;try{await sdk.importBrowserSpore(new Uint8Array([1]),{...options,root:'fail'});}catch{refused=true;}if(!refused||globalThis.detachedSecret[0]!==0)throw Error('failed Boot secret retirement');
document.body.textContent='PASS';
}catch(e){document.body.textContent='FAIL: '+e.stack;}
`;
 const server=createServer((req,res)=>{res.setHeader('Content-Type',req.url==='/composition.mjs'?'text/javascript':'text/html');res.end(req.url==='/composition.mjs'?source:`<!doctype html><body>RUNNING<script type="module">${script}</script>`);});
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 try{
 const child=spawn(chromium,['--no-sandbox','--disable-gpu','--dump-dom','--timeout=10000','--virtual-time-budget=3000',`http://127.0.0.1:${server.address().port}/`]);let output='',errors='';child.stdout.on('data',d=>output+=d);child.stderr.on('data',d=>errors+=d);
 const status=await new Promise((resolve,reject)=>{child.on('error',reject);child.on('exit',resolve);});assert.equal(status,0,errors);assert.match(output,/<body>PASS<\/body>/,output);
 }finally{await new Promise(resolve=>server.close(resolve));}
});
