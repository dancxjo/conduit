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
const context={crypto:webcrypto,TextEncoder,Uint8Array,URL,Blob,console,setTimeout,location:{protocol:'file:'},localStorage:{getItem:k=>saved.get(k)||null,setItem:(k,v)=>saved.set(k,v)},document:{querySelector:k=>{if(!elements.has(k))elements.set(k,{textContent:k==='#bootstrap'?JSON.stringify(bootstrap):'',append(){},value:''});return elements.get(k)},createElement:()=>({style:{}})}};
vm.createContext(context);
vm.runInContext(source+`;globalThis.api={join,verifyReceipt,validatePending,recover,proofTranscript,recoveryTranscript,currentTranscript,receiptTranscript,sign,setPending:p=>pending=p,getPending:()=>pending,returnObserver,verifyReturnChallenge,verifyReturnedReceipt,returnChallengeTranscript,returnProofTranscript,validateReturn,setRoot:r=>bootstrap.root_key=r};`,context);
(async()=>{
 const api=context.api,root=await webcrypto.subtle.generateKey('Ed25519',true,['sign','verify']);
 const rootBytes=Array.from(new Uint8Array(await webcrypto.subtle.exportKey('raw',root.publicKey)));api.setRoot(rootBytes);
 const host='browser/test',boot='boot/browser-original';
 const signature=Array.from(await api.sign(await api.proofTranscript(host,boot))).map(v=>v.toString(16).padStart(2,'0')).join('');
 const request={schema:'conduit.environment/observer-admission@1',invitation_id:'invitation/test',body_id:'body/test',nonce:'09'.repeat(32),signature,host_advertisement:{protocol_version:1,host_id:host,boot_id:boot,offer_generation:1,profile:'conduit/environment/browser-observer@1',resources:[],capabilities:[],planner_capabilities:[]},membership_admitted:false,plan_created:false,play_created:false};
 const p={endpoint:'http://127.0.0.1:8787',host,boot,request,ticket:41};api.setPending(p);
 async function signed(r,transcript){r.signature=Array.from(new Uint8Array(await webcrypto.subtle.sign('Ed25519',root.privateKey,await transcript(r))));return r}
 const credential={credential_id:'credential/1',body_id:'body/test',part_id:'part/1',host_id:host,boot_id:boot,issued_at_millis:50};
 p.receipt=await signed({schema:'conduit.environment/joined-host@1',credential,producer_host:'esp32/test',producer_boot:'boot/root-old',invitation_id:'invitation/test',membership_revision:2,offer_generation:1,producer_key:rootBytes},api.receiptTranscript);
 let active,ticket=50,posts=[],challenge,returned;
 context.fetch=async(url,options)=>{
  if(options?.method==='POST'){active=JSON.parse(options.body);posts.push(active.schema);return {status:202,json:async()=>({schema:'conduit.environment/body-join-queued@1',ticket:++ticket})}}
  if(active.schema==='conduit.environment/observer-return-challenge-request@1'){
   const a=active.host_advertisement;
   challenge=await signed({schema:'conduit.environment/observer-return-challenge@1',challenge:{admission_id:'admission/return',body_id:'body/test',part_id:'part/1',host_id:host,boot_id:a.boot_id,offer_generation:1,nonce:Array(32).fill(19),issued_at_millis:100,expires_at_millis:1000},producer_host:'esp32/test',producer_boot:'boot/root-new',producer_key:rootBytes},api.returnChallengeTranscript);
   return {status:200,json:async()=>challenge};
  }
  assert.equal(active.schema,'conduit.environment/observer-return-request@1');
  assert.equal(active.signature,Buffer.from(await api.sign(await api.returnProofTranscript(challenge.challenge))).toString('hex'));
  returned=await signed({schema:'conduit.environment/current-observer@1',credential:{...credential,credential_id:'credential/returned',boot_id:active.host_advertisement.boot_id,issued_at_millis:200},producer_host:'esp32/test',producer_boot:'boot/root-new',producer_key:rootBytes,membership_revision:4,request_nonce:challenge.challenge.nonce},api.currentTranscript);
  return {status:200,json:async()=>returned};
 };
 const admission=JSON.stringify(p.receipt),original=JSON.stringify(p.request);
 await api.returnObserver(p.endpoint);assert.deepEqual(posts,['conduit.environment/observer-return-challenge-request@1','conduit.environment/observer-return-request@1']);
 assert.equal(JSON.stringify(p.receipt),admission);assert.equal(JSON.stringify(p.request),original);assert.equal(p.boot,boot);assert.notEqual(p.returning.boot,boot);await api.validateReturn(p.returning);
 for(const mutate of [r=>r.challenge.nonce[0]^=1,r=>r.challenge.boot_id=boot,r=>r.challenge.part_id='part/foreign',r=>r.producer_key[0]^=1,r=>r.signature[0]^=1]){const r=structuredClone(challenge);mutate(r);await assert.rejects(api.verifyReturnChallenge(r,p.returning))}
 for(const mutate of [r=>r.credential.boot_id=boot,r=>r.credential.part_id='part/foreign',r=>r.request_nonce[0]^=1,r=>r.signature[0]^=1,r=>r.producer_boot='boot/foreign']){const r=structuredClone(returned);mutate(r);await assert.rejects(api.verifyReturnedReceipt(r,p.returning))}
 // A lost completion result uses read-only recovery for the new Boot, never another invitation.
 delete p.returning.receipt;p.returning.finish.ticket=75;posts=[];
 context.fetch=async(url,options)=>{if(options?.method==='POST'){active=JSON.parse(options.body);posts.push(active.schema);assert.equal(active.boot_id,p.returning.boot);return {status:202,json:async()=>({schema:'conduit.environment/body-join-queued@1',ticket:76})}}if(url.endsWith('=75'))return {status:404};return {status:200,json:async()=>returned}};
 await api.returnObserver(p.endpoint);assert.deepEqual(posts,['conduit.environment/observer-recovery-request@1']);assert.equal(p.returning.receipt_from_recovery,true);
 const validFinish=p.returning.finish.request.signature;p.returning.finish.request.signature='00'.repeat(64);let calls=0;context.fetch=async()=>{calls++;throw Error('must not send')};await assert.rejects(api.returnObserver(p.endpoint));assert.equal(calls,0);p.returning.finish.request.signature=validFinish;
 // Ambiguous challenge delivery retains the exact new Boot/request for retry.
 delete p.returning;context.fetch=async()=>{throw Error('network interrupted')};await assert.rejects(api.returnObserver(p.endpoint));const retained=JSON.stringify(p.returning);await assert.rejects(api.returnObserver(p.endpoint));assert.equal(JSON.stringify(p.returning),retained);
 console.log('browser return: pinned challenge, canonical proof, signed fresh-Boot receipt, refusal checks, lost completion recovery and ambiguous retry GREEN');
})().catch(e=>{console.error(e);process.exitCode=1});
