// CPU WebCrypto/protocol proof; no browser, producer, or presence proof.
const {readFileSync}=require('node:fs');
const vm=require('node:vm');
const assert=require('node:assert/strict');
const {webcrypto}=require('node:crypto');
const html=readFileSync(require('node:path').join(__dirname,'../src/body_join_browser.html'),'utf8');
const source=html.match(/<script>\n([\s\S]*?)<\/script>/)[1];
const elements=new Map();
const bootstrap={invitation:{schema:'conduit.body/spawn-invitation@1',secret:Array(32).fill(7),claim:{body_id:'body/test',invitation_id:'invitation/test',nonce:Array(32).fill(9),expires_at_millis:99999}},root_key:[],producer_host:'esp32/test',producer_boot:'boot/root-old'};
const saved=new Map();
const context={crypto:webcrypto,TextEncoder,Uint8Array,URL,Blob,console,setTimeout,AbortSignal,location:{protocol:'file:'},localStorage:{getItem:k=>saved.get(k)||null,setItem:(k,v)=>saved.set(k,v)},document:{querySelector:k=>{if(!elements.has(k))elements.set(k,{textContent:k==='#bootstrap'?JSON.stringify(bootstrap):'',append(){},value:''});return elements.get(k)},createElement:()=>({style:{}})}};
vm.createContext(context);
vm.runInContext(source+`;globalThis.api={join,verifyReceipt,validatePending,recover,proofTranscript,recoveryTranscript,currentTranscript,receiptTranscript,sign,setPending:p=>pending=p,getPending:()=>pending,setRoot:r=>bootstrap.root_key=r};`,context);
(async()=>{
 const api=context.api;
 const root=await webcrypto.subtle.generateKey('Ed25519',true,['sign','verify']);
 const rootBytes=Array.from(new Uint8Array(await webcrypto.subtle.exportKey('raw',root.publicKey)));api.setRoot(rootBytes);
 const host='browser/test',boot='boot/browser-original';
 const signature=Array.from(await api.sign(await api.proofTranscript(host,boot))).map(v=>v.toString(16).padStart(2,'0')).join('');
 const request={schema:'conduit.environment/observer-admission@1',invitation_id:'invitation/test',body_id:'body/test',nonce:'09'.repeat(32),signature,host_advertisement:{protocol_version:1,host_id:host,boot_id:boot,offer_generation:1,profile:'conduit/environment/browser-observer@1',resources:[],capabilities:[],planner_capabilities:[]},membership_admitted:false,plan_created:false,play_created:false};
 const p={endpoint:'http://127.0.0.1:8787',host,boot,request,ticket:41};api.setPending(p);await api.validatePending();
 const credential={credential_id:'credential/1',body_id:'body/test',part_id:'part/1',host_id:host,boot_id:boot,issued_at_millis:50};
 async function signed(r,recovery){r.signature=Array.from(new Uint8Array(await webcrypto.subtle.sign('Ed25519',root.privateKey,await (recovery?api.currentTranscript(r):api.receiptTranscript(r)))));return r}
 p.receipt=await signed({schema:'conduit.environment/joined-host@1',credential,producer_host:'esp32/test',producer_boot:'boot/root-old',invitation_id:'invitation/test',membership_revision:2,offer_generation:1,producer_key:rootBytes},false);
 await api.validatePending();
 for(const mutate of [r=>r.credential.boot_id='boot/new',r=>r.producer_key[0]^=1,r=>r.signature[0]^=1,r=>r.producer_boot='boot/root-new']){const original=p.receipt;p.receipt=structuredClone(original);mutate(p.receipt);await assert.rejects(api.validatePending());p.receipt=original}
 let posts=0,query;
 context.fetch=async(url,options)=>{if(options?.method==='POST'){posts++;query=JSON.parse(options.body);return {status:202,json:async()=>({schema:'conduit.environment/body-join-queued@1',ticket:99})}}return {status:200,json:async()=>signed({schema:'conduit.environment/current-observer@1',credential,producer_host:'esp32/test',producer_boot:'boot/root-new',membership_revision:3,producer_key:rootBytes,request_nonce:query.nonce},true)}};
 await api.recover(p.endpoint);assert.equal(posts,1);assert.equal(query.boot_id,boot);await api.validatePending();
 const original=structuredClone(p.recovery.receipt);
 for(const mutate of [r=>r.request_nonce[0]^=1,r=>r.credential.body_id='body/other',r=>r.credential.host_id='browser/other',r=>r.signature[0]^=1,r=>r.membership_revision=0]){const r=structuredClone(original);mutate(r);await assert.rejects(api.verifyReceipt(r,true))}
 const retained=JSON.stringify(p.recovery.request);context.fetch=async()=>{throw Error('ambiguous network failure')};await assert.rejects(api.recover(p.endpoint));assert.equal(JSON.stringify(p.recovery.request),retained);assert.equal(p.recovery.ticket,99);
 context.fetch=async()=>({status:404});await assert.rejects(api.recover(p.endpoint));assert.equal(p.recovery.ticket,null);assert.equal(JSON.stringify(p.recovery.request),retained);
 // A lost original volatile ticket must never replay its invitation proof.
 delete p.receipt;delete p.recovery;p.ticket=41;let lost=true;posts=0;
 context.fetch=async(url,options)=>{if(options?.method==='POST'){posts++;query=JSON.parse(options.body);assert.equal(query.schema,'conduit.environment/observer-recovery-request@1');return {status:202,json:async()=>({schema:'conduit.environment/body-join-queued@1',ticket:100})}}if(lost){lost=false;return {status:404}}return {status:200,json:async()=>signed({schema:'conduit.environment/current-observer@1',credential,producer_host:'esp32/test',producer_boot:'boot/root-new',membership_revision:3,producer_key:rootBytes,request_nonce:query.nonce},true)}};
 await api.join();assert.equal(posts,1);assert.equal(p.ticket,41);assert.equal(p.recovery.ticket,100);
 p.recovery.request.boot_id='boot/forged';await assert.rejects(api.validatePending());
 console.log('browser recovery: cached scope/signatures, original Boot, changed Root Boot, nonce/body/host refusals, ambiguous retry and lost mailbox preservation GREEN');
})().catch(e=>{console.error(e);process.exitCode=1});
