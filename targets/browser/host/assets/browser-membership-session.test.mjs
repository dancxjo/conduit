import test from 'node:test';
import assert from 'node:assert/strict';
import { createBrowserMembershipSession } from './browser-membership-session.mjs';
import { joinBrowserBody } from './browser-membership.js';
const turn=()=>new Promise(resolve=>setImmediate(resolve));
function carrier(){let next,closed=0;const sent=[];return {sent,get closed(){return closed;},sendSessionFrame:async bytes=>sent.push(bytes.slice()),receiveSessionFrame:()=>new Promise(resolve=>next=resolve),deliver:bytes=>next(bytes),close:()=>closed++};}
class Socket {
 static instances=[];
 constructor(){this.handlers={};this.sent=[];Socket.instances.push(this);}
 addEventListener(name,fn){this.handlers[name]=fn;}
 send(bytes){this.sent.push(bytes.slice());}
 close(code,reason){this.closeCode=code;this.closeReason=reason;}
 emit(name,event={}){this.handlers[name]?.(event);}
}
test('admitted carrier serializes frames and closes exactly once on cancellation',async()=>{
 const c=carrier(),received=[];let closes=0;
 const s=createBrowserMembershipSession({carrier:c});s.start({open:()=>s.send(new Uint8Array([1])),message:async e=>received.push([...e.data]),close:()=>closes++});
 await turn();assert.deepEqual([...c.sent[0]],[1]);c.deliver(new Uint8Array([2]));await turn();assert.deepEqual(received,[[2]]);
 s.close();s.close();assert.equal(c.closed,1);assert.equal(closes,1);assert.equal(s.isOpen(),false);assert.throws(()=>s.send(new Uint8Array([3])));
 c.deliver(new Uint8Array([4]));await turn();assert.deepEqual(received,[[2]]);
});
test('finite sends refuse pressure and clear queued buffers on close',async()=>{
 const c=carrier();let release;const held=[];c.sendSessionFrame=bytes=>{held.push(bytes);return new Promise(resolve=>release=resolve);};
 const s=createBrowserMembershipSession({carrier:c});s.start({open(){},message(){},close(){}});await turn();
 for(let i=0;i<4;i++)s.send(new Uint8Array([i+1]));assert.throws(()=>s.send(new Uint8Array([5])),/pressure/);
 await turn();s.close();assert.equal(held[0][0],0);release();await turn();assert.equal(held.length,1);
});
test('bad carrier frame or handler failure closes without delivering later authority',async()=>{
 for(const bad of [new Uint8Array(0),new Uint8Array(65*1024+1),'']){
 const c=carrier();let received=0;const s=createBrowserMembershipSession({carrier:c});s.start({open(){},message(){received++;},close(){}});await turn();c.deliver(bad);await turn();assert.equal(c.closed,1);assert.equal(received,0);
 }
 const c=carrier(),s=createBrowserMembershipSession({carrier:c});s.start({open(){throw Error('refuse');},message(){},close(){}});await turn();assert.equal(c.closed,1);
});
test('original WebSocket wrapper preserves binary frames and close code',async()=>{
 const s=createBrowserMembershipSession({url:'wss://owner.invalid',WebSocketType:Socket});let opened=0;const received=[];
 s.start({open(){opened++;},message:e=>received.push([...e.data]),close(){}});const ws=Socket.instances.at(-1);ws.emit('open');await turn();s.send(new Uint8Array([1]));ws.emit('message',{data:new Uint8Array([2]).buffer});await turn();assert.equal(opened,1);assert.deepEqual(received,[[2]]);assert.deepEqual([...ws.sent[0]],[1]);s.close(1008,'refused');assert.equal(ws.closeCode,1008);
});
function admittedHost(){const api={memory:{buffer:new ArrayBuffer(8192)}};for(const name of ['input_ptr','output_ptr','output_len','initialize','prove','prove_return','advertisement'])api['conduit_browser_membership_'+name]=()=>0;api.conduit_browser_membership_input_capacity=()=>4096;api.conduit_browser_membership_output_capacity=()=>4096;return {api,hostId:'host/test',bootId:'boot/test',membership:{schema:'conduit.browser/body-membership-client@1',hostId:'host/test',bootId:'boot/test',verifyingKey:Array(32).fill(1),advertisement:()=>({host_id:'host/test',boot_id:'boot/test'})}};}
test('actual membership advertise and refusal state are identical over carrier and WS',async()=>{
 const saved={document:globalThis.document,window:globalThis.window,WebSocket:globalThis.WebSocket};globalThis.document={visibilityState:'visible',addEventListener(){}};globalThis.window={addEventListener(){}};globalThis.WebSocket=Socket;
 try{for(const mode of ['carrier','websocket']){
 const c=carrier(),states=[];const membership=await joinBrowserBody({bodyUrl:'wss://owner.invalid',sessionCarrier:mode==='carrier'?c:null,wasmBytes:new Uint8Array([1]),admittedHost:admittedHost(),onState:s=>states.push(s)});
 const ws=Socket.instances.at(-1);if(mode==='websocket')ws.emit('open');await turn();const bytes=mode==='carrier'?c.sent[0]:ws.sent[0];assert.equal(JSON.parse(new TextDecoder().decode(bytes)).kind,'advertise');
 const refusal=new TextEncoder().encode(JSON.stringify({kind:'refused',protocol:1,code:'exact-window-refused'}));if(mode==='carrier')c.deliver(refusal);else ws.emit('message',{data:refusal});await turn();assert.equal(membership.state(),'refused:exact-window-refused');assert.equal(membership.membershipCredential()?.credential_id,undefined);membership.close();await turn();assert.equal(membership.presenceState(),'unavailable');
 }}finally{Object.assign(globalThis,saved);}
});

test('cancellation during durable credential custody cannot restore live admission',async()=>{
 const saved={document:globalThis.document,window:globalThis.window};globalThis.document={visibilityState:'visible',addEventListener(){}};globalThis.window={addEventListener(){}};
 try{
 let committed;const c=carrier(),states=[];
 const membership=await joinBrowserBody({sessionCarrier:c,wasmBytes:new Uint8Array([1]),admittedHost:admittedHost(),onState:s=>states.push(s),onCredential:()=>new Promise(resolve=>committed=resolve)});
 await turn();c.deliver(new TextEncoder().encode(JSON.stringify({kind:'admitted',protocol:1,credential:{credential_id:'credential/test',body_id:'body/test',part_id:'part/test',host_id:'host/test',boot_id:'boot/test',issued_at_millis:1}})));await turn();
 membership.close();committed();await turn();assert.equal(membership.state(),'offline');assert.equal(membership.presenceState(),'unavailable');assert.equal(states.includes('admitted'),false);assert.equal(c.closed,1);
 }finally{Object.assign(globalThis,saved);}
});

test('WS preserves canonical large advertisement envelope while protected carrier keeps exact bound',async()=>{
 const s=createBrowserMembershipSession({url:'wss://owner.invalid',WebSocketType:Socket});
 s.start({open(){},message(){},close(){}});const ws=Socket.instances.at(-1);ws.emit('open');await turn();
 s.send(new Uint8Array(701632));assert.equal(ws.sent[0].length,701632);
 assert.throws(()=>s.send(new Uint8Array(2*1024*1024+1025)),/bound/);s.close();
 const c=carrier(),protectedSession=createBrowserMembershipSession({carrier:c});protectedSession.start({open(){},message(){},close(){}});await turn();
 assert.throws(()=>protectedSession.send(new Uint8Array(65520)),/bound/);protectedSession.close();
 const limited=carrier();limited.maximumFrameBytes=32;const small=createBrowserMembershipSession({carrier:limited});small.start({open(){},message(){},close(){}});await turn();assert.throws(()=>small.send(new Uint8Array(33)),/bound/);small.close();
});

// Protocol composition uses an explicit stub verifier; it does not claim an
// actual WASM boot or cryptographic proof over the live network.
function rootAwareFixture() {
 const host=admittedHost(),events=[];
 const credential={credential_id:'credential/test',body_id:'body/test',part_id:'part/test',host_id:host.hostId,boot_id:host.bootId,issued_at_millis:1};
 const claim={invitation_id:'invitation/test',body_id:'body/test',nonce:Array(32).fill(3),expires_at_millis:Date.now()+60000};
 let stored=null;
 Object.assign(host.membership,{
 advertisement:()=>({host_id:host.hostId,boot_id:host.bootId,offer_generation:1}),
 prepareMembershipScope:input=>{events.push('prepare');return {...claim,host_id:host.hostId,boot_id:host.bootId,offer_generation:1};},
 proveSpawn:()=>({hostId:host.hostId,bootId:host.bootId,signature:Array(64).fill(4)}),
 adoptMembershipScope:receipt=>{events.push('verify');if(receipt?.signature!=='valid')throw Error('invalid Root receipt');return credential;},
 exportPartMembership:()=>{events.push('export');return new Uint8Array([7]);},
 importPartMembership:()=>({...credential,boot_id:'boot/prior'}),
 retireMembershipScope:()=>events.push('retire'),
 });
 const storage={readBytes:async()=>stored?.slice()??null,writeBytes:async(key,bytes)=>{events.push('persist');stored=bytes.slice();},deleteJson:async()=>{stored=null;}};
 const options={wasmBytes:new Uint8Array([1]),admittedHost:host,expectedBodyId:'body/test',invitedProvision:{claim,secret:Array(32).fill(2)},
 sporeRoot:{rootAuthority:{schema:'conduit.body/root-authority@1',body_id:'body/test',owner_host_id:'owner/test',public_key:Array(32).fill(11)},rootProducer:{host_id:'owner/test',boot_id:'owner-boot/test'}},selectedStorage:storage,renewPresence:false,reconnectPresence:false};
 return {options,events,credential,storage,seedStored(){stored=new Uint8Array([7]);}};
}
const browserGlobals=async run=>{
 const saved={document:globalThis.document,window:globalThis.window};globalThis.document={visibilityState:'visible',addEventListener(){}};globalThis.window={addEventListener(){}};
 try{await run();}finally{Object.assign(globalThis,saved);}
};
const deliverJson=(c,frame)=>c.deliver(new TextEncoder().encode(JSON.stringify(frame)));
test('actual membership signed mode rejects unsigned or invalid receipt before callback or admission',()=>browserGlobals(async()=>{
 for(const signed_membership_receipt of [undefined,{signature:'invalid'}]){
 const f=rootAwareFixture(),c=carrier(),callbacks=[];
 const joined=await joinBrowserBody({...f.options,sessionCarrier:c,onCredential:credential=>callbacks.push(credential)});await turn();
 assert.equal(JSON.parse(new TextDecoder().decode(c.sent[0])).kind,'invited-advertise');
 deliverJson(c,{kind:'admitted',protocol:1,credential:f.credential,signed_membership_receipt});await turn();
 assert.deepEqual(callbacks,[]);assert.equal(joined.state(),'offline');assert.equal(joined.presenceState(),'unavailable');assert.equal(f.events.includes('persist'),false);assert.equal(c.closed,1);
 }
}));
test('verified membership is retained before callback and cancellation during custody cannot restore Active',()=>browserGlobals(async()=>{
 const f=rootAwareFixture(),c=carrier(),callbacks=[];let release;
 f.storage.writeBytes=async()=>{f.events.push('persist-start');await new Promise(resolve=>release=resolve);f.events.push('persist-end');};
 const joined=await joinBrowserBody({...f.options,sessionCarrier:c,onCredential:credential=>{f.events.push('callback');callbacks.push(credential);}});await turn();
 deliverJson(c,{kind:'admitted',protocol:1,credential:f.credential,signed_membership_receipt:{signature:'valid'}});await turn();
 joined.close();release();await turn();
 assert.deepEqual(f.events,['prepare','verify','export','persist-start','persist-end','callback']);
 assert.deepEqual(callbacks,[f.credential]);assert.equal(joined.state(),'offline');assert.equal(joined.presenceState(),'unavailable');assert.equal(f.events.includes('retire'),false);
}));
test('custody-only receipt retains verified Part but creates no credential/presence/Face permission',()=>browserGlobals(async()=>{
 const f=rootAwareFixture(),c=carrier(),callbacks=[];
 const joined=await joinBrowserBody({...f.options,sessionCarrier:c,onCredential:credential=>callbacks.push(credential)});await turn();
 deliverJson(c,{kind:'membership-receipt',protocol:1,receipt:{signature:'valid'}});await turn();
 assert.deepEqual(callbacks,[f.credential]);assert.equal(joined.state(),'custody-retained-awaiting-fresh-boot');assert.equal(joined.presenceState(),'unavailable');
 assert.equal(joined.membershipCredential(),null);assert.equal(c.closed,1);
 await assert.rejects(joined.requestFaceSnapshot({}),/membership|presence/i);
 assert.equal(f.events.includes('retire'),false);
}));
test('Root-aware fresh Boot uses retained Part for existing Return path without a new admission preparation',()=>browserGlobals(async()=>{
 const f=rootAwareFixture();f.seedStored();const c=carrier();
 const joined=await joinBrowserBody({...f.options,sessionCarrier:c});await turn();
 const sent=JSON.parse(new TextDecoder().decode(c.sent[0]));assert.equal(sent.kind,'return-advertise');assert.equal(sent.credential.boot_id,'boot/prior');
 assert.equal(joined.state(),'returning-fresh-boot');assert.equal(joined.presenceState(),'unavailable');assert.equal(f.events.includes('prepare'),false);joined.close();
}));
test('Root-bearing provision cannot silently enter legacy unsigned mode or reuse the current Boot',()=>browserGlobals(async()=>{
 const f=rootAwareFixture();f.options.invitedProvision.rootAuthority=f.options.sporeRoot.rootAuthority;
 await assert.rejects(joinBrowserBody({...f.options,sporeRoot:null,sessionCarrier:carrier()}),/explicit selected Root/);
 const g=rootAwareFixture();g.seedStored();g.options.admittedHost.membership.importPartMembership=()=>g.credential;
 await assert.rejects(joinBrowserBody({...g.options,sessionCarrier:carrier()}),/current Boot/);
 assert.equal(g.events.includes('retire'),false);assert.equal(g.events.includes('prepare'),false);
}));
